//! 沙盒任务创建与停止逻辑。

use std::sync::Arc;

use chrono::Utc;

use crate::db::{
    delete_sandbox_run_record, find_release_by_id, find_sandbox_run_by_task_id,
    insert_sandbox_run_record, update_sandbox_run_record,
};
use crate::error::AppError;
use crate::server::{
    OperationLogAction, OperationLogBiz, OperationLogParams, write_operation_log_for_result,
};
use crate::utils::SystemKind;

use super::result::fetch_release_version;
use super::runner as sandbox_runner;
use super::{
    Conclusion, CreateSandboxRunRequest, CreateSandboxRunResponse, QueuePlacement, SandboxRun,
    SandboxState, SandboxTaskHandle, StageStatus, TaskStatus,
};

/// 创建沙盒运行任务并将其排入队列。
pub async fn create_sandbox_run_logic(
    state: SandboxState,
    request: CreateSandboxRunRequest,
) -> Result<CreateSandboxRunResponse, AppError> {
    let release = find_release_by_id(request.release_id)
        .await?
        .ok_or_else(|| AppError::not_found("发布记录不存在"))?;
    let release_system = release
        .system
        .parse::<SystemKind>()
        .map_err(|_| AppError::validation(format!("未知系统类型: {}", release.system)))?;
    let release_version = release.version.clone();
    let release_id = release.id;

    let sanitized_options = request.options.clone().sanitized();
    let options_for_log = sanitized_options.clone();
    let overrides = request.overrides.clone();
    let overrides_len = overrides.len();

    let result = async {
        let run = SandboxRun::new(release_id, overrides, sanitized_options);
        insert_sandbox_run_record(&run)
            .await
            .map_err(AppError::from)?;
        let task_id = run.task_id.clone();
        let task_handle = Arc::new(SandboxTaskHandle::new(run, release_system));
        let placement = match state.enqueue_task(task_handle.clone()).await {
            Ok(p) => p,
            Err(err) => {
                let _ = delete_sandbox_run_record(&task_id).await;
                return Err(err);
            }
        };

        if matches!(placement, QueuePlacement::Immediate) {
            sandbox_runner::spawn_sandbox_execution(state.clone(), task_handle.clone());
        }

        let snapshot = task_handle.snapshot().await;
        info!(
            "创建沙盒任务成功: task_id={}, release_id={}, placement={:?}",
            snapshot.task_id, snapshot.release_id, placement
        );

        Ok(CreateSandboxRunResponse {
            task_id: snapshot.task_id,
            status: snapshot.status,
            queue_position: placement.position(),
        })
    }
    .await;

    let mut log_params = OperationLogParams::new()
        .with_target_id(release_id.to_string())
        .with_target_name(release_version)
        .with_field("sample_count", options_for_log.sample_count.to_string())
        .with_field("overrides", overrides_len.to_string())
        .with_field("keep_workspace", options_for_log.keep_workspace.to_string());

    if let Ok(resp) = &result {
        log_params = log_params
            .with_field("task_id", resp.task_id.clone())
            .with_field("queue_position", resp.queue_position.to_string());
    }

    write_operation_log_for_result(
        OperationLogBiz::Release,
        OperationLogAction::Validate,
        log_params,
        &result,
    )
    .await;

    result
}

/// 停止正在执行或等待中的沙盒任务。
pub async fn stop_sandbox_run_logic(
    state: SandboxState,
    task_id: &str,
) -> Result<SandboxRun, AppError> {
    let result = if let Some(task) = state.find_current_task(task_id).await {
        task.cancel_token().cancel();
        task.with_run_mut(|run| {
            run.status = TaskStatus::Stopped;
            let now = Utc::now();
            run.ended_at.get_or_insert(now);
            for stage in &mut run.stages {
                if matches!(stage.status, StageStatus::Pending | StageStatus::Running) {
                    stage.status = StageStatus::Stopped;
                    stage
                        .summary
                        .get_or_insert_with(|| "任务已被用户终止".to_string());
                    stage.ended_at.get_or_insert(now);
                }
            }
            if run.conclusion.is_none() {
                run.conclusion = Some(Conclusion::stopped());
            }
        })
        .await;
        let snapshot = task.snapshot().await;
        if let Err(err) = update_sandbox_run_record(&snapshot).await {
            warn!("更新沙盒记录失败: {}", err);
        } else {
            info!(
                "已停止运行中的沙盒任务: task_id={}, release_id={}",
                snapshot.task_id, snapshot.release_id
            );
        }
        Ok(snapshot)
    } else if let Some(task) = state.stop_queued_task(task_id).await {
        task.with_run_mut(|run| {
            run.status = TaskStatus::Stopped;
            let now = Utc::now();
            run.ended_at = Some(now);
            for stage in &mut run.stages {
                if matches!(stage.status, StageStatus::Pending | StageStatus::Running) {
                    stage.status = StageStatus::Stopped;
                    stage.summary = Some("任务在排队阶段被终止".to_string());
                    let created_at = run.created_at;
                    stage.started_at.get_or_insert(created_at);
                    stage.ended_at = Some(now);
                    stage.duration_ms = Some(0);
                }
            }
            run.conclusion = Some(Conclusion::stopped());
        })
        .await;

        let snapshot = task.snapshot().await;
        if let Err(err) = update_sandbox_run_record(&snapshot).await {
            warn!("更新沙盒记录失败: {}", err);
        } else {
            info!(
                "已停止排队中的沙盒任务: task_id={}, release_id={}",
                snapshot.task_id, snapshot.release_id
            );
        }
        Ok(snapshot)
    } else if find_sandbox_run_by_task_id(task_id)
        .await
        .map_err(AppError::from)?
        .is_some()
    {
        Err(AppError::conflict_with_code(
            "TASK_NOT_RUNNING",
            "当前没有正在执行或等待的沙盒任务",
        ))
    } else {
        Err(AppError::not_found("沙盒任务不存在或已完成"))
    };

    let mut log_params = OperationLogParams::new().with_field("task_id", task_id.to_string());
    if let Ok(run) = &result {
        log_params = log_params
            .with_target_id(run.release_id.to_string())
            .with_field("final_status", run.status.as_str().to_string());
        if let Some(version) = fetch_release_version(run.release_id).await {
            log_params = log_params.with_target_name(version);
        }
    }

    write_operation_log_for_result(
        OperationLogBiz::Release,
        OperationLogAction::Cancel,
        log_params,
        &result,
    )
    .await;

    result
}
