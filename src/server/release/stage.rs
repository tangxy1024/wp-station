//! 发布阶段与状态摘要工具。
//!
//! 负责发布详情、发布列表、发布目标轨迹的序列化与反序列化，
//! 以及根据发布状态生成前端展示所需的阶段快照。

use crate::constants::release::{group_title, publish_label};
use crate::db::{Release, ReleaseStatus, ReleaseTarget};
use crate::error::AppError;
use crate::utils::format_beijing_time;

use super::{ReleaseDeviceDetail, StageSnapshot};

/// 根据发布状态和发布范围构造列表页/详情页的阶段摘要。
pub fn stage_summary_for_release(
    release_status: &ReleaseStatus,
    release_group: &str,
) -> Vec<StageSnapshot> {
    let publish_status = match release_status {
        ReleaseStatus::PASS => "pass",
        ReleaseStatus::FAIL => "fail",
        ReleaseStatus::PARTIAL_FAIL => "fail",
        ReleaseStatus::RUNNING => "running",
        _ => "pending",
    };

    vec![
        StageSnapshot {
            label: "沙盒".to_string(),
            status: "pass".to_string(),
            detail: Some("最近一次沙盒验证已通过".to_string()),
        },
        StageSnapshot {
            label: publish_label(release_group).to_string(),
            status: publish_status.to_string(),
            detail: Some(group_title(release_group).to_string()),
        },
    ]
}

/// 序列化发布摘要阶段信息。
pub fn serialize_stage_summary(stages: &[StageSnapshot]) -> String {
    serde_json::to_string(stages).unwrap_or_else(|_| "[]".to_string())
}

/// 序列化发布目标的阶段轨迹。
pub fn serialize_stage_trace(stages: &[StageSnapshot]) -> String {
    serde_json::to_string(stages).unwrap_or_else(|_| "[]".to_string())
}

/// 解析发布目标的阶段轨迹；解析失败时回退到默认轨迹。
pub fn parse_stage_trace(raw: Option<&str>) -> Vec<StageSnapshot> {
    raw.and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(default_target_stage_trace)
}

/// 默认的发布目标阶段轨迹。
pub fn default_target_stage_trace() -> Vec<StageSnapshot> {
    vec![
        StageSnapshot {
            label: "准备".to_string(),
            status: "pass".to_string(),
            detail: None,
        },
        StageSnapshot {
            label: "调用客户端".to_string(),
            status: "pending".to_string(),
            detail: None,
        },
        StageSnapshot {
            label: "运行状态".to_string(),
            status: "pending".to_string(),
            detail: None,
        },
    ]
}

/// 回滚场景下的发布目标阶段轨迹。
pub fn rollback_target_stage_trace() -> Vec<StageSnapshot> {
    vec![
        StageSnapshot {
            label: "准备".to_string(),
            status: "pass".to_string(),
            detail: Some("开启回滚流程".to_string()),
        },
        StageSnapshot {
            label: "调用客户端".to_string(),
            status: "pending".to_string(),
            detail: Some("重新推送旧版本配置".to_string()),
        },
        StageSnapshot {
            label: "运行状态".to_string(),
            status: "pending".to_string(),
            detail: None,
        },
    ]
}

/// 根据发布状态生成最简阶段摘要。
pub fn stage_summary_for_status(status: &ReleaseStatus) -> Vec<StageSnapshot> {
    let summary_status = match status {
        ReleaseStatus::PASS => "pass",
        ReleaseStatus::FAIL => "fail",
        ReleaseStatus::PARTIAL_FAIL => "fail",
        ReleaseStatus::RUNNING => "running",
        _ => "pending",
    };
    vec![StageSnapshot {
        label: "发布".to_string(),
        status: summary_status.to_string(),
        detail: None,
    }]
}

/// 构造发布列表和详情页使用的概要阶段信息。
pub(super) fn build_release_summary_stages(
    sandbox_ready: bool,
    release_status: &ReleaseStatus,
    release_group: &str,
) -> Vec<StageSnapshot> {
    let sandbox_stage = StageSnapshot {
        label: "沙盒".to_string(),
        status: if sandbox_ready { "pass" } else { "pending" }.to_string(),
        detail: None,
    };

    let publish_status = match release_status {
        ReleaseStatus::PASS => "pass",
        ReleaseStatus::FAIL | ReleaseStatus::PARTIAL_FAIL => "fail",
        ReleaseStatus::RUNNING => "running",
        ReleaseStatus::WAIT if sandbox_ready => "running",
        _ => "pending",
    };

    vec![
        sandbox_stage,
        StageSnapshot {
            label: publish_label(release_group).to_string(),
            status: publish_status.to_string(),
            detail: None,
        },
    ]
}

/// 从数据库中的字符串状态解析为发布状态枚举。
pub(super) fn parse_release_status(release: &Release) -> Result<ReleaseStatus, AppError> {
    release
        .status
        .parse::<ReleaseStatus>()
        .map_err(|_| AppError::Validation("无效的发布状态".to_string()))
}

/// 组装单台设备的发布详情。
pub(super) fn build_device_detail(
    target: &ReleaseTarget,
    device: Option<&crate::db::Device>,
) -> Result<ReleaseDeviceDetail, AppError> {
    let (ip, port, name, client_version, _config_version, last_seen_at) =
        if let Some(device) = device {
            (
                device.ip.clone(),
                device.port,
                device.name.clone(),
                device.client_version.clone(),
                device.config_version.clone(),
                device.last_seen_at.map(format_beijing_time),
            )
        } else {
            (
                "-".to_string(),
                0,
                None,
                None,
                None,
                Some(format_beijing_time(target.updated_at)),
            )
        };

    Ok(ReleaseDeviceDetail {
        id: target.id,
        device_id: target.device_id,
        device_name: name,
        ip,
        port,
        status: target.status.clone(),
        client_version,
        config_version: target.current_config_version.clone(),
        target_config_version: target.target_config_version.clone(),
        stage_trace: parse_stage_trace(target.stage_trace.as_deref()),
        error_message: target.error_message.clone(),
        last_seen_at,
    })
}
