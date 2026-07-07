//! 沙盒任务收尾与结果记录。

use crate::db::find_release_by_id;
use crate::server::{
    OperationLogAction, OperationLogBiz, OperationLogParams, OperationLogStatus,
    write_operation_log,
};

use super::{SandboxRun, TaskStatus};

/// 查询发布版本号，用于补全沙盒操作日志中的目标名称。
pub(super) async fn fetch_release_version(release_id: i32) -> Option<String> {
    match find_release_by_id(release_id).await {
        Ok(Some(release)) => Some(release.version),
        Ok(None) => {
            warn!(
                "未找到 release 记录，无法补全沙盒日志: release_id={}",
                release_id
            );
            None
        }
        Err(err) => {
            warn!(
                "查询 release 失败，无法补全沙盒日志: release_id={}, error={}",
                release_id, err
            );
            None
        }
    }
}

/// 将沙盒执行结果写入操作日志，供发布审计和问题回溯使用。
pub(super) async fn log_sandbox_execution_result(run: &SandboxRun) {
    let mut params = OperationLogParams::new()
        .with_target_id(run.release_id.to_string())
        .with_field("task_id", run.task_id.clone())
        .with_field("final_status", run.status.as_str().to_string());

    if let Some(version) = fetch_release_version(run.release_id).await {
        params = params.with_target_name(version);
    }

    if let Some(conclusion) = run.conclusion.as_ref() {
        params = params
            .with_field("passed", conclusion.passed.to_string())
            .with_field(
                "runtime_error_count",
                conclusion.runtime_error_count.to_string(),
            )
            .with_field(
                "runtime_output_count",
                conclusion.runtime_output_count.to_string(),
            )
            .with_field(
                "runtime_miss_count",
                conclusion.runtime_miss_count.to_string(),
            )
            .with_field("input_count", conclusion.input_count.to_string());

        if let Some(stage) = conclusion.failed_stage {
            params = params.with_field("failed_stage", stage.as_str().to_string());
        }
    }

    let log_status = if run
        .conclusion
        .as_ref()
        .map(|c| c.passed)
        .unwrap_or(run.status == TaskStatus::Success)
    {
        OperationLogStatus::Success
    } else {
        OperationLogStatus::Error
    };

    write_operation_log(
        OperationLogBiz::Release,
        OperationLogAction::Validate,
        params,
        log_status,
    )
    .await;
}
