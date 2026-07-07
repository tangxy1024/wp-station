//! 操作日志写入逻辑。

use crate::db::{NewOperationLog, create_operation_log};
use crate::error::AppError;

use super::{OperationLogAction, OperationLogBiz, OperationLogParams, OperationLogStatus};

/// 根据业务对象和动作类型组装默认描述文案。
fn build_description(biz: OperationLogBiz, action: OperationLogAction) -> String {
    match (biz, action) {
        (OperationLogBiz::ConfigFile, OperationLogAction::Create) => "新建配置文件".to_string(),
        (OperationLogBiz::ConfigFile, OperationLogAction::Update) => "保存配置文件".to_string(),
        (OperationLogBiz::ConfigFile, OperationLogAction::Delete) => "删除配置文件".to_string(),
        (OperationLogBiz::RuleFile, OperationLogAction::Create) => "新建规则文件".to_string(),
        (OperationLogBiz::RuleFile, OperationLogAction::Update) => "保存规则文件".to_string(),
        (OperationLogBiz::RuleFile, OperationLogAction::Delete) => "删除规则文件".to_string(),
        (OperationLogBiz::KnowledgeConfig, OperationLogAction::Create) => "新建知识库".to_string(),
        (OperationLogBiz::KnowledgeConfig, OperationLogAction::Update) => "保存知识库".to_string(),
        (OperationLogBiz::KnowledgeConfig, OperationLogAction::Delete) => "删除知识库".to_string(),
        (OperationLogBiz::AssistTask, OperationLogAction::Submit) => "提交辅助任务".to_string(),
        (OperationLogBiz::AssistTask, OperationLogAction::Cancel) => "取消辅助任务".to_string(),
        (OperationLogBiz::AssistTask, OperationLogAction::Reply) => "写回辅助任务结果".to_string(),
        _ => format!("{}{}", action.verb(), biz.label()),
    }
}

/// 根据目标名称和目标 ID 组装面向审计页展示的 target 文案。
fn build_target(biz: OperationLogBiz, params: &OperationLogParams) -> Option<String> {
    match (&params.target_name, &params.target_id) {
        (Some(name), Some(id)) => Some(format!("{} {} [ID: {}]", biz.label(), name, id)),
        (Some(name), None) => Some(format!("{} {}", biz.label(), name)),
        (None, Some(id)) => Some(format!("{} [ID: {}]", biz.label(), id)),
        (None, None) => Some(biz.label().to_string()),
    }
}

/// 将扩展字段序列化为简短的 `key=value` 审计内容。
fn build_content(params: &OperationLogParams) -> Option<String> {
    if params.fields.is_empty() {
        return None;
    }

    Some(
        params
            .fields
            .iter()
            .map(|(key, value)| format!("{}={}", key, value))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// 将业务层传入的上下文转换为数据库写入模型。
fn to_new_operation_log(
    biz: OperationLogBiz,
    action: OperationLogAction,
    params: OperationLogParams,
    status: OperationLogStatus,
) -> NewOperationLog {
    let target = build_target(biz, &params);
    let description = build_description(biz, action);
    let content = build_content(&params);

    NewOperationLog {
        operator: params.operator.unwrap_or_else(|| "system".to_string()),
        operation: action.code().to_string(),
        target,
        description: Some(description),
        content,
        status: status.as_str().to_string(),
    }
}

/// Best-effort 写入操作日志。失败时只记运行日志，不影响主业务返回。
pub async fn write_operation_log(
    biz: OperationLogBiz,
    action: OperationLogAction,
    params: OperationLogParams,
    status: OperationLogStatus,
) {
    let log = to_new_operation_log(biz, action, params, status);
    if let Err(err) = create_operation_log(log).await {
        warn!("写入操作日志失败: error={}", err);
    }
}

/// 根据主业务结果自动推导 `success / error` 并 best-effort 写入操作日志。
pub async fn write_operation_log_for_result<T>(
    biz: OperationLogBiz,
    action: OperationLogAction,
    params: OperationLogParams,
    result: &Result<T, AppError>,
) {
    let status = if result.is_ok() {
        OperationLogStatus::Success
    } else {
        OperationLogStatus::Error
    };

    write_operation_log(biz, action, params, status).await;
}
