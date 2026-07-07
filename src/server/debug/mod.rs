//! 调试功能业务逻辑层。
//!
//! 汇总解析调试、格式化和知识库调试等能力，
//! 对外保持统一的调试业务入口。

mod knowledge;

use crate::error::AppError;
use crate::utils::warp_check_record;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use wp_data_fmt::{FormatType, Json, RecordFormatter};
use wp_model_core::model::DataRecord;

pub use self::knowledge::{
    debug_knowledge_query_fields_logic, debug_knowledge_query_logic,
    debug_knowledge_query_rows_logic, debug_knowledge_status_logic,
};

/// API 层之间共享的解析结果缓存。
pub type SharedRecord = Arc<Mutex<Option<DataRecord>>>;

// ============ 请求参数结构体 ============

/// 调试解析请求体。
#[derive(Deserialize)]
pub struct DebugParseRequest {
    pub rules: String,
    pub logs: String,
}

/// 调试转换请求体。
#[derive(Deserialize)]
pub struct DebugTransformRequest {
    pub oml: String,
    #[serde(default)]
    pub parse_result: Option<serde_json::Value>,
}

/// 知识库状态查询参数。
#[derive(Deserialize)]
pub struct DebugKnowledgeStatusQuery {}

/// 调试知识库查询请求体。
#[derive(Deserialize)]
pub struct DebugKnowledgeQueryRequest {
    pub table: String,
    #[serde(default)]
    pub source_kind: Option<String>,
    pub sql: String,
}

// ============ 响应结构体 ============

/// 解析结果原始记录响应体。
#[derive(Serialize, Deserialize)]
pub struct RecordResponseRaw {
    pub fields: DataRecord,
    pub format_json: String,
}

/// 知识库状态列表项。
#[derive(Serialize)]
pub struct DebugKnowledgeStatusItem {
    pub tag_name: String,
    pub label: String,
    pub suggested_sql: String,
    pub source_kind: String,
    pub is_active: bool,
}

/// 调试知识库查询响应体。
#[derive(Serialize)]
pub struct DebugKnowledgeQueryResponse {
    pub success: bool,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total: usize,
}

/// 解析日志并返回字段列表
pub async fn debug_parse_logic(
    shared_record: Arc<Mutex<Option<DataRecord>>>,
    rules: String,
    logs: String,
) -> Result<RecordResponseRaw, AppError> {
    // 调用 warp_check_record 获取 DataRecord
    let record = warp_check_record(&rules, &logs)?;

    // 存入 SharedRecord，供后续转换使用
    let mut record_guard = shared_record.lock().await;
    *record_guard = Some(record.clone());

    // 生成 format_json
    let formatter = FormatType::Json(Json);
    let json_string = formatter.fmt_record(&record);

    // 返回 RecordResponseRaw，包含完整的 DataRecord 和 format_json
    Ok(RecordResponseRaw {
        fields: record,
        format_json: json_string,
    })
}

/// 使用最近一次解析结果执行 OML 转换
pub async fn debug_transform_logic(
    shared_record: Arc<Mutex<Option<DataRecord>>>,
    oml: String,
) -> Result<RecordResponseRaw, AppError> {
    let record = {
        let record_guard = shared_record.lock().await;
        record_guard.clone()
    }
    .ok_or(AppError::NoParseResult)?;

    let transformed = crate::utils::oml::convert_record(&oml, record).await?;
    let formatter = FormatType::Json(Json);
    let json_string = formatter.fmt_record(&transformed);

    Ok(RecordResponseRaw {
        fields: transformed,
        format_json: json_string,
    })
}

/// WPL 代码格式化
pub fn wpl_format_logic(code: String) -> Result<String, AppError> {
    use crate::utils::WplFormatter;

    let formatter = WplFormatter::new();
    formatter
        .format_with_error(&code)
        .map_err(|e| AppError::validation(format!("格式化 WPL 代码失败: {}", e)))
}

/// OML 代码格式化
pub fn oml_format_logic(code: String) -> Result<String, AppError> {
    use crate::utils::OmlFormatter;

    let formatter = OmlFormatter::new();
    formatter
        .format_content(&code)
        .map_err(|e| AppError::validation(format!("格式化 OML 代码失败: {}", e)))
}

/// 获取调试示例列表
pub fn debug_examples_logic() -> BTreeMap<String, serde_json::Value> {
    // wp-station 通过连接管理访问项目，示例应该从连接的项目中加载
    // 目前返回空列表，让前端使用默认示例
    BTreeMap::new()
}
