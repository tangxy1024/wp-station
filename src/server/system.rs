// 系统信息业务逻辑层

use serde::Serialize;

use crate::server::Setting;
use crate::server::setting::default_data_collect_url;
use crate::utils::{
    load_integration_rule_overview_from_layout, load_integration_runtime_overview_from_layout,
};

#[derive(Serialize)]
pub struct VersionResponse {
    pub wp_station: &'static str,
    pub wp_parse: &'static str,
}

#[derive(Serialize)]
pub struct FeaturesConfigResponse {
    pub data_collect_url: String,
    pub default_data_collect_url: String,
}

#[derive(Serialize)]
pub struct IntegrationRuntimeItemResponse {
    pub key: String,
    pub title: String,
    pub connect: String,
    pub type_key: String,
    pub type_label: String,
    pub detail: String,
}

#[derive(Serialize)]
pub struct IntegrationRuntimeOverviewResponse {
    pub sources: Vec<IntegrationRuntimeItemResponse>,
    pub sinks: Vec<IntegrationRuntimeItemResponse>,
    pub supported_source_type_count: usize,
    pub supported_sink_type_count: usize,
}

#[derive(Serialize)]
pub struct IntegrationRuleLogTypeResponse {
    pub key: String,
    pub log_type_name: String,
    pub rule_keys: Vec<String>,
}

#[derive(Serialize)]
pub struct IntegrationRuleItemResponse {
    pub key: String,
    pub device_type: String,
    pub log_types: Vec<IntegrationRuleLogTypeResponse>,
}

#[derive(Serialize)]
pub struct IntegrationRuleOverviewResponse {
    pub items: Vec<IntegrationRuleItemResponse>,
}

/// 返回服务存活探针信息。
pub fn hello_logic() -> &'static str {
    "Hello from Actix-web!"
}

/// 返回当前服务与依赖组件版本。
pub fn get_version_logic() -> VersionResponse {
    VersionResponse {
        wp_station: env!("WP_STATION_VERSION"),
        wp_parse: env!("WP_PARSE_VERSION"),
    }
}

/// 返回前端展示所需的配置项
pub fn get_features_config_logic() -> FeaturesConfigResponse {
    let setting = Setting::load();
    FeaturesConfigResponse {
        data_collect_url: setting.features.data_collect_url,
        default_data_collect_url: default_data_collect_url(),
    }
}

/// 返回接入概览页面所需的输入源与输出源运行时摘要。
pub fn get_integration_runtime_overview_logic()
-> Result<IntegrationRuntimeOverviewResponse, crate::error::AppError> {
    let layout = Setting::load().project_layout();
    let overview = load_integration_runtime_overview_from_layout(&layout)?;

    Ok(IntegrationRuntimeOverviewResponse {
        sources: overview
            .sources
            .into_iter()
            .map(|item| IntegrationRuntimeItemResponse {
                key: item.key,
                title: item.title,
                connect: item.connect,
                type_key: item.type_key,
                type_label: item.type_label,
                detail: item.detail,
            })
            .collect(),
        sinks: overview
            .sinks
            .into_iter()
            .map(|item| IntegrationRuntimeItemResponse {
                key: item.key,
                title: item.title,
                connect: item.connect,
                type_key: item.type_key,
                type_label: item.type_label,
                detail: item.detail,
            })
            .collect(),
        supported_source_type_count: overview.supported_source_type_count,
        supported_sink_type_count: overview.supported_sink_type_count,
    })
}

/// 返回接入概览页面所需的规则侧设备类型与日志类型摘要。
pub fn get_integration_rule_overview_logic()
-> Result<IntegrationRuleOverviewResponse, crate::error::AppError> {
    let layout = Setting::load().project_layout();
    let overview = load_integration_rule_overview_from_layout(&layout)?;

    Ok(IntegrationRuleOverviewResponse {
        items: overview
            .items
            .into_iter()
            .map(|item| IntegrationRuleItemResponse {
                key: item.key,
                device_type: item.device_type,
                log_types: item
                    .log_types
                    .into_iter()
                    .map(|log_type| IntegrationRuleLogTypeResponse {
                        key: log_type.key,
                        log_type_name: log_type.log_type_name,
                        rule_keys: log_type.rule_keys,
                    })
                    .collect(),
            })
            .collect(),
    })
}
