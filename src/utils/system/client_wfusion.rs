//! `wfusion` 系统设备侧服务适配器。
//!
//! 目前接入两类能力：
//! - 健康检查：`GET /admin/v1/runtime/status`
//! - 发布：`POST /admin/v1/reloads/model`
//!
//! 接口路径与 `wparse` 基本一致，但状态字段存在差异：
//! - 在线状态字段使用 `accepting`
//! - 发布接口可能直接返回 `result=applied`，此时可视为立即完成

use super::client_wparse::{DeployCheckResult, DeployResult, ServiceError};
use crate::constants::warparse::{DEPLOY_PATH, STATUS_PATH};
use crate::db::Device;
use crate::error::AppError;
use crate::server::setting::AdminApiConf;
use reqwest::{Certificate, Client};
use serde::Deserialize;
use serde_json::Value;
use std::error::Error;
use std::fs;
use std::sync::OnceLock;
use std::time::Duration;

/// 返回 `wfusion` 当前未实现能力的统一校验错误。
pub fn not_implemented(feature: &str) -> AppError {
    AppError::validation(format!("wfusion {} 暂未实现", feature))
}

/// `wfusion` 设备健康检查快照。
#[derive(Debug, Clone)]
pub struct DeviceHealthSnapshot {
    pub is_online: bool,
    pub client_version: Option<String>,
    pub config_version: Option<String>,
}

/// `wfusion` 发布请求最小载荷。
#[derive(Debug, Clone)]
pub struct PublishPayload {
    pub version: String,
    pub release_group: String,
}

#[derive(Deserialize, Debug)]
struct StatusResponse {
    version: Option<String>,
    accepting: Option<bool>,
    accepting_commands: Option<bool>,
    reloading: Option<bool>,
    current_request_id: Option<String>,
    last_reload_request_id: Option<String>,
    last_reload_result: Option<String>,
    project_version: Option<Value>,
    config_version: Option<String>,
    applied_version: Option<String>,
    active_version: Option<String>,
}

#[derive(Deserialize, Debug)]
struct PublishResponse {
    accepted: Option<bool>,
    request_id: Option<String>,
    message: Option<String>,
    result: Option<String>,
    hot_reload: Option<BoolLike>,
    requires_restart: Option<BoolLike>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum BoolLike {
    Bool(bool),
    Int(i64),
    String(String),
}

impl BoolLike {
    fn is_truthy(&self) -> bool {
        match self {
            BoolLike::Bool(value) => *value,
            BoolLike::Int(value) => *value != 0,
            BoolLike::String(value) => {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            }
        }
    }
}

/// `wfusion` 外部服务适配器。
pub struct WfusionService {
    client: Client,
    scheme: &'static str,
}

static WFUSION_CA_PEM: OnceLock<Option<Vec<u8>>> = OnceLock::new();

impl Default for WfusionService {
    fn default() -> Self {
        Self::new().expect("创建 WfusionService 失败")
    }
}

impl WfusionService {
    /// 创建 `wfusion` 服务适配器。
    pub fn new() -> Result<Self, ServiceError> {
        let setting = crate::server::Setting::load();
        Self::from_admin_api_conf(&setting.admin_api, None)
    }

    /// 使用自定义超时时间创建服务实例，供设备连通性校验场景复用。
    pub fn with_timeout(timeout: Duration) -> Result<Self, ServiceError> {
        let setting = crate::server::Setting::load();
        Self::from_admin_api_conf(&setting.admin_api, Some(timeout))
    }

    /// 启动时预加载 TLS CA 文件，尽早暴露证书问题。
    pub fn preload_tls(conf: &AdminApiConf) -> Result<(), ServiceError> {
        if WFUSION_CA_PEM.get().is_some() {
            return Ok(());
        }

        if !conf.enabled {
            let _ = WFUSION_CA_PEM.set(None);
            return Ok(());
        }

        if conf.ca_file.trim().is_empty() {
            return Err(ServiceError::InvalidState(
                "wfusion TLS 证书未配置".to_string(),
            ));
        }

        let ca_pem = Some(fs::read(&conf.ca_file).map_err(|e| {
            ServiceError::Network(format!(
                "读取 wfusion 证书失败: path={}, error={}",
                conf.ca_file, e
            ))
        })?);

        if let Some(ca_pem) = ca_pem.as_ref() {
            Certificate::from_pem(ca_pem).map_err(|e| {
                ServiceError::Tls(format!(
                    "解析 wfusion 证书失败: path={}, error={}",
                    conf.ca_file, e
                ))
            })?;
        }

        let _ = WFUSION_CA_PEM.set(ca_pem);
        Ok(())
    }

    /// 根据统一管理接口配置构建 HTTP 客户端。
    pub fn from_admin_api_conf(
        conf: &AdminApiConf,
        timeout: Option<Duration>,
    ) -> Result<Self, ServiceError> {
        let mut builder = Client::builder();
        let scheme = if conf.enabled { "https" } else { "http" };

        if let Some(timeout) = timeout {
            builder = builder.timeout(timeout);
        }

        if let Some(ca_bytes) = Self::load_ca_pem(conf)? {
            let ca = Certificate::from_pem(&ca_bytes)
                .map_err(|e| ServiceError::Tls(format!("解析 wfusion 证书失败: error={}", e)))?;
            builder = builder.add_root_certificate(ca);
        }

        let client = builder
            .build()
            .map_err(|e| ServiceError::Network(e.to_string()))?;

        Ok(Self { client, scheme })
    }

    /// 查询设备健康状态。
    pub async fn check_health(&self, device: &Device) -> Result<DeviceHealthSnapshot, AppError> {
        let status = self
            .fetch_status(device)
            .await
            .map_err(|err| AppError::validation(err.to_string()))?;
        let config_version = status.config_version();
        Ok(DeviceHealthSnapshot {
            is_online: status.is_accepting(),
            client_version: status.version,
            config_version,
        })
    }

    /// 发起 `wfusion` 发布请求。
    pub async fn publish(
        &self,
        device: &Device,
        payload: PublishPayload,
    ) -> Result<DeployResult, ServiceError> {
        let url = self.build_url(device, DEPLOY_PATH)?;
        let body = serde_json::json!({
            "wait": true,
            "update": true,
            "version": payload.version,
            "group": payload.release_group,
            "timeout_ms": 15000,
            "reason": "wp-station deployment",
        });

        info!("调用 wfusion 发布 API: url={}", url);
        debug!("wfusion 发布请求参数: {}", body);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", device.token))
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                let error = Self::classify_reqwest_error(&err);
                warn!("wfusion 发布请求失败: error={}", error);
                error
            })?;

        let status = resp.status();
        if !status.is_success() {
            let error_body = resp
                .text()
                .await
                .unwrap_or_else(|_| "无法读取响应体".to_string());
            return Err(ServiceError::Response(format!(
                "HTTP {} - {}",
                status, error_body
            )));
        }

        let parsed: PublishResponse = resp
            .json()
            .await
            .map_err(|err| ServiceError::Response(err.to_string()))?;

        let result_text = parsed.result.clone();
        let completed = parsed.accepted.unwrap_or(false)
            && Self::publish_result_succeeded(result_text.as_deref());
        let requires_restart = parsed
            .requires_restart
            .as_ref()
            .map(BoolLike::is_truthy)
            .unwrap_or(false);
        let hot_reload = parsed
            .hot_reload
            .as_ref()
            .map(BoolLike::is_truthy)
            .unwrap_or(false);

        let message =
            Self::publish_message(parsed.message, result_text, hot_reload, requires_restart);

        info!(
            "wfusion 发布响应: accepted={:?}, request_id={:?}, completed={}, requires_restart={}, hot_reload={}",
            parsed.accepted, parsed.request_id, completed, requires_restart, hot_reload
        );

        Ok(DeployResult {
            accepted: parsed.accepted.unwrap_or(false),
            request_id: parsed.request_id,
            message,
            completed: completed && !requires_restart,
        })
    }

    /// 轮询发布结果。
    ///
    /// `wfusion` 若未在发布接口直接返回 `applied`，则尝试从状态接口中读取：
    /// - `reloading`
    /// - `last_reload_request_id`
    /// - `last_reload_result`
    /// - `config_version/project_version`
    pub async fn check_deploy_success(
        &self,
        device: &Device,
        target_version: &str,
        expected_request_id: Option<&str>,
    ) -> Result<DeployCheckResult, ServiceError> {
        let status = self.fetch_status(device).await?;
        let config_version = status.config_version();
        let current_version = config_version.clone();
        let is_reloading = status.reloading.unwrap_or(false);

        if is_reloading {
            return Ok(DeployCheckResult {
                is_success: false,
                current_version,
                config_version,
                is_reloading: true,
            });
        }

        let request_id_matched = expected_request_id
            .map(|expected| status.last_reload_request_id.as_deref() == Some(expected))
            .unwrap_or(true);
        let version_matched = current_version
            .as_deref()
            .map(|current| {
                Self::normalize_version(current) == Self::normalize_version(target_version)
            })
            .unwrap_or(false);
        let reload_done = status
            .last_reload_result
            .as_deref()
            .map(|value| Self::publish_result_succeeded(Some(value)))
            .unwrap_or(false);

        let is_success = if version_matched {
            expected_request_id.is_none() || request_id_matched || reload_done
        } else {
            request_id_matched && reload_done
        };

        if !is_success {
            debug!(
                "wfusion 发布状态未完成: expected_request_id={:?}, actual_request_id={:?}, target_version={}, current_version={:?}, reload_result={:?}, current_request_id={:?}",
                expected_request_id,
                status.last_reload_request_id,
                target_version,
                current_version,
                status.last_reload_result,
                status.current_request_id
            );
        }

        Ok(DeployCheckResult {
            is_success,
            current_version,
            config_version,
            is_reloading: false,
        })
    }

    async fn fetch_status(&self, device: &Device) -> Result<StatusResponse, ServiceError> {
        let url = self.build_url(device, STATUS_PATH)?;
        debug!("调用 wfusion 状态 API: url={}", url);

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", device.token))
            .send()
            .await
            .map_err(|err| {
                let error = Self::classify_reqwest_error(&err);
                warn!("wfusion 状态请求失败: error={}", error);
                error
            })?;

        let status = resp.status();
        if !status.is_success() {
            return Err(ServiceError::Response(format!("HTTP {}", status)));
        }

        let payload = resp
            .json::<StatusResponse>()
            .await
            .map_err(|err| ServiceError::Response(err.to_string()))?;

        debug!(
            "wfusion 状态响应: accepting={:?}, version={:?}, config_version={:?}, reloading={:?}, last_reload_result={:?}",
            payload.accepting.or(payload.accepting_commands),
            payload.version,
            payload.config_version(),
            payload.reloading,
            payload.last_reload_result
        );

        Ok(payload)
    }

    fn build_url(&self, device: &Device, path: &str) -> Result<String, ServiceError> {
        let base = self.device_endpoint(device)?;
        Ok(format!("{}{}", base, path))
    }

    /// `0.0.0.0` / `::` 是监听地址，不适合作为客户端目标地址，统一归一化到本机回环。
    fn device_endpoint(&self, device: &Device) -> Result<String, ServiceError> {
        if device.ip.trim().is_empty() {
            return Err(ServiceError::InvalidState(
                "设备未配置 IP，无法连接".to_string(),
            ));
        }
        if device.port <= 0 {
            return Err(ServiceError::InvalidState(
                "设备未配置有效端口，无法连接".to_string(),
            ));
        }

        let normalized_ip = match device.ip.trim() {
            "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
            value => value,
        };
        Ok(format!(
            "{}://{}:{}",
            self.scheme, normalized_ip, device.port
        ))
    }

    fn classify_reqwest_error(err: &reqwest::Error) -> ServiceError {
        let message = Self::error_chain(err);
        let lower = message.to_ascii_lowercase();
        if err.is_timeout() {
            ServiceError::Timeout(message)
        } else if err.is_connect() {
            ServiceError::Connect(message)
        } else if lower.contains("tls")
            || lower.contains("certificate")
            || lower.contains("handshake")
            || lower.contains("invalid http version parsed")
            || lower.contains("invalidcontenttype")
        {
            ServiceError::Tls(message)
        } else {
            ServiceError::Network(message)
        }
    }

    fn load_ca_pem(conf: &AdminApiConf) -> Result<Option<Vec<u8>>, ServiceError> {
        if let Some(cached) = WFUSION_CA_PEM.get() {
            return Ok(cached.clone());
        }

        if !conf.enabled {
            return Ok(None);
        }

        if conf.ca_file.trim().is_empty() {
            return Err(ServiceError::InvalidState(
                "wfusion TLS 证书未配置".to_string(),
            ));
        }

        let ca_pem = fs::read(&conf.ca_file).map_err(|e| {
            ServiceError::Network(format!(
                "读取 wfusion 证书失败: path={}, error={}",
                conf.ca_file, e
            ))
        })?;
        Ok(Some(ca_pem))
    }

    fn error_chain(err: &(dyn Error + 'static)) -> String {
        let mut parts = vec![err.to_string()];
        let mut current = err.source();
        while let Some(source) = current {
            parts.push(source.to_string());
            current = source.source();
        }
        parts.join(" -> ")
    }

    fn publish_result_succeeded(result: Option<&str>) -> bool {
        match result.map(|value| value.trim().to_ascii_lowercase()) {
            Some(value) => matches!(value.as_str(), "applied" | "success" | "reload_done"),
            None => false,
        }
    }

    fn publish_message(
        message: Option<String>,
        result: Option<String>,
        hot_reload: bool,
        requires_restart: bool,
    ) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(message) = message.filter(|value| !value.trim().is_empty()) {
            parts.push(message);
        }
        if let Some(result) = result.filter(|value| !value.trim().is_empty()) {
            parts.push(format!("result={}", result));
        }
        if hot_reload {
            parts.push("hot_reload=true".to_string());
        }
        if requires_restart {
            parts.push("requires_restart=true".to_string());
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    }

    fn normalize_version(version: &str) -> &str {
        version
            .strip_prefix('v')
            .or_else(|| version.strip_prefix('V'))
            .unwrap_or(version)
    }
}

impl StatusResponse {
    fn is_accepting(&self) -> bool {
        self.accepting.or(self.accepting_commands).unwrap_or(false)
    }

    fn config_version(&self) -> Option<String> {
        self.config_version
            .clone()
            .or_else(|| self.applied_version.clone())
            .or_else(|| self.active_version.clone())
            .or_else(|| Self::version_from_value(self.project_version.as_ref()))
    }

    fn version_from_value(value: Option<&Value>) -> Option<String> {
        let value = value?;
        match value {
            Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
            Value::Object(map) => ["version", "tag", "current", "active"]
                .iter()
                .find_map(|key| map.get(*key).and_then(|value| value.as_str()))
                .map(str::to_string),
            _ => None,
        }
    }
}
