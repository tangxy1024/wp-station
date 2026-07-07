//! 操作日志业务逻辑层。
//!
//! 统一收敛操作日志的三类职责：
//! - 公共请求/响应和枚举类型
//! - 写日志的组装与落库
//! - 日志列表查询与日期过滤

mod read;
mod write;

use crate::db::operation_log::OperationLog;
use crate::utils::pagination::{PageQuery, PageResponse};
use serde::{Deserialize, Serialize};

pub use self::read::list_logs_logic;
pub use self::write::{write_operation_log, write_operation_log_for_result};

// ============ 请求参数结构体 ============

/// 操作日志列表查询参数。
#[derive(Deserialize)]
pub struct LogListQuery {
    /// 操作人模糊匹配。
    pub operator: Option<String>,
    /// 操作类型精确匹配: create / update / delete / publish。
    pub operation: Option<String>,
    /// 开始日期，格式 YYYY-MM-DD。
    pub start_date: Option<String>,
    /// 结束日期，格式 YYYY-MM-DD。
    pub end_date: Option<String>,
    #[serde(flatten)]
    pub page: PageQuery,
}

// ============ 响应结构体 ============

/// 操作日志分页响应。
pub type LogListResponse = PageResponse<OperationLog>;

/// 操作日志写入状态。
#[derive(Debug, Clone, Copy, Serialize)]
pub enum OperationLogStatus {
    Success,
    Error,
}

impl OperationLogStatus {
    /// 返回数据库中使用的状态字符串。
    pub(super) fn as_str(self) -> &'static str {
        match self {
            OperationLogStatus::Success => "success",
            OperationLogStatus::Error => "error",
        }
    }
}

/// 操作日志业务对象。
#[derive(Debug, Clone, Copy, Serialize)]
pub enum OperationLogBiz {
    ConfigFile,
    RuleFile,
    KnowledgeConfig,
    Device,
    User,
    Release,
    ReleaseTarget,
    AssistTask,
}

impl OperationLogBiz {
    /// 返回用于展示的业务对象中文名称。
    pub(super) fn label(self) -> &'static str {
        match self {
            OperationLogBiz::ConfigFile => "配置文件",
            OperationLogBiz::RuleFile => "规则文件",
            OperationLogBiz::KnowledgeConfig => "知识库",
            OperationLogBiz::Device => "设备",
            OperationLogBiz::User => "用户",
            OperationLogBiz::Release => "发布单",
            OperationLogBiz::ReleaseTarget => "发布目标",
            OperationLogBiz::AssistTask => "辅助任务",
        }
    }
}

/// 操作日志动作枚举。
#[derive(Debug, Clone, Copy, Serialize)]
pub enum OperationLogAction {
    Create,
    Update,
    Delete,
    Submit,
    Cancel,
    Reply,
    Publish,
    Retry,
    Rollback,
    Validate,
    Login,
    ResetPassword,
    ChangePassword,
}

impl OperationLogAction {
    /// 返回数据库中使用的动作编码。
    pub(super) fn code(self) -> &'static str {
        match self {
            OperationLogAction::Create => "create",
            OperationLogAction::Update => "update",
            OperationLogAction::Delete => "delete",
            OperationLogAction::Submit => "submit",
            OperationLogAction::Cancel => "cancel",
            OperationLogAction::Reply => "reply",
            OperationLogAction::Publish => "publish",
            OperationLogAction::Retry => "retry",
            OperationLogAction::Rollback => "rollback",
            OperationLogAction::Validate => "validate",
            OperationLogAction::Login => "login",
            OperationLogAction::ResetPassword => "reset-password",
            OperationLogAction::ChangePassword => "change-password",
        }
    }

    /// 返回组装描述文案时使用的动作动词。
    pub(super) fn verb(self) -> &'static str {
        match self {
            OperationLogAction::Create => "新建",
            OperationLogAction::Update => "更新",
            OperationLogAction::Delete => "删除",
            OperationLogAction::Submit => "提交",
            OperationLogAction::Cancel => "取消",
            OperationLogAction::Reply => "写回",
            OperationLogAction::Publish => "发布",
            OperationLogAction::Retry => "重试",
            OperationLogAction::Rollback => "回滚",
            OperationLogAction::Validate => "校验",
            OperationLogAction::Login => "登录",
            OperationLogAction::ResetPassword => "重置密码",
            OperationLogAction::ChangePassword => "修改密码",
        }
    }
}

/// 业务层写入操作日志时传入的最小上下文。
#[derive(Debug, Clone, Default)]
pub struct OperationLogParams {
    pub operator: Option<String>,
    pub target_name: Option<String>,
    pub target_id: Option<String>,
    pub fields: Vec<(String, String)>,
}

impl OperationLogParams {
    /// 创建空的日志参数。
    pub fn new() -> Self {
        Self::default()
    }

    /// 指定操作人。
    pub fn with_operator(mut self, operator: impl Into<String>) -> Self {
        self.operator = Some(operator.into());
        self
    }

    /// 指定展示用目标名称。
    pub fn with_target_name(mut self, target_name: impl Into<String>) -> Self {
        self.target_name = Some(target_name.into());
        self
    }

    /// 指定展示用目标 ID。
    pub fn with_target_id(mut self, target_id: impl Into<String>) -> Self {
        self.target_id = Some(target_id.into());
        self
    }

    /// 追加审计内容中的键值对字段。
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push((key.into(), value.into()));
        self
    }
}
