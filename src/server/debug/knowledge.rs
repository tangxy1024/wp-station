//! 知识库调试相关业务。

use crate::error::AppError;
use crate::server::RepoLayout;
use crate::server::Setting;
use crate::utils::{
    configured_provider_name, list_knowledge_dirs, load_knowledge, load_sqlite_knowledge,
    reload_knowledge, reload_sqlite_knowledge, should_reload_knowledge_source, sql_query_rows,
};

use super::{DebugKnowledgeQueryResponse, DebugKnowledgeStatusItem};

/// 知识库调试查询的数据源类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnowledgeQuerySource {
    ConfiguredProvider,
    LocalSqlite,
}

/// 查询知识库配置状态列表。
pub async fn debug_knowledge_status_logic() -> Result<Vec<DebugKnowledgeStatusItem>, AppError> {
    let setting = Setting::load();
    let layout = setting.wparse_layout();
    let provider_name = configured_provider_name(&layout)?;
    let local_tables = list_knowledge_dirs(&layout)?;
    let provider_name_ref = provider_name.as_deref().map(str::to_string);

    if provider_name.is_some() {
        reload_knowledge(&layout).map_err(AppError::internal)?;
    } else if !local_tables.is_empty() {
        reload_sqlite_knowledge(&layout).map_err(AppError::internal)?;
    }

    let mut list = Vec::new();
    if let Some(provider_name) = provider_name {
        list.push(provider_name);
    }
    list.extend(local_tables);

    let items: Vec<DebugKnowledgeStatusItem> = list
        .into_iter()
        .map(|file_name| {
            let source_kind = if provider_name_ref.as_deref() == Some(file_name.as_str()) {
                "provider"
            } else {
                "local"
            };
            let suggested_sql = if source_kind == "provider" {
                "select * from your_table limit 20;".to_string()
            } else {
                format!("select * from {file_name} limit 20;")
            };
            DebugKnowledgeStatusItem {
                tag_name: file_name.clone(),
                label: file_name.clone(),
                suggested_sql,
                source_kind: source_kind.to_string(),
                is_active: true,
            }
        })
        .collect();

    Ok(items)
}

/// 执行知识库 SQL 查询（调试用）。
pub async fn debug_knowledge_query_logic(
    table: String,
    source_kind: Option<String>,
    sql: String,
) -> Result<DebugKnowledgeQueryResponse, AppError> {
    let rows = debug_knowledge_query_rows_for_source_logic(Some(table), source_kind, sql).await?;
    let columns: Vec<String> = rows
        .first()
        .map(|row| {
            row.iter()
                .map(|field| field.get_name().to_string())
                .collect()
        })
        .unwrap_or_default();
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|field| field.get_value().to_string())
                .collect()
        })
        .collect();
    let total = table_rows.len();

    Ok(DebugKnowledgeQueryResponse {
        success: true,
        columns,
        rows: table_rows,
        total,
    })
}

/// 执行知识库 SQL 查询并返回原始字段行（供调试页表格适配使用）。
pub async fn debug_knowledge_query_rows_logic(
    sql: String,
) -> Result<Vec<Vec<wp_model_core::model::DataField>>, AppError> {
    debug_knowledge_query_rows_for_source_logic(None, None, sql).await
}

/// 执行知识库 SQL 查询并返回第一行字段（兼容旧调用方）。
pub async fn debug_knowledge_query_fields_logic(
    sql: String,
) -> Result<Vec<wp_model_core::model::DataField>, AppError> {
    let mut rows = debug_knowledge_query_rows_logic(sql).await?;
    Ok(if rows.is_empty() {
        Vec::new()
    } else {
        rows.remove(0)
    })
}

/// 根据显式 source_kind / 选择项 / provider 配置推断本次查询来源。
fn resolve_knowledge_query_source(
    selected_kind: Option<&str>,
    selected: Option<&str>,
    provider_name: Option<&str>,
) -> KnowledgeQuerySource {
    if let Some(kind) = selected_kind
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if kind.eq_ignore_ascii_case("provider") {
            return KnowledgeQuerySource::ConfiguredProvider;
        }
        if kind.eq_ignore_ascii_case("local") {
            return KnowledgeQuerySource::LocalSqlite;
        }
    }

    if let Some(selected) = selected.map(str::trim).filter(|value| !value.is_empty()) {
        if provider_name.is_some_and(|provider| provider == selected) {
            return KnowledgeQuerySource::ConfiguredProvider;
        }
        return KnowledgeQuerySource::LocalSqlite;
    }

    if provider_name.is_some() {
        KnowledgeQuerySource::ConfiguredProvider
    } else {
        KnowledgeQuerySource::LocalSqlite
    }
}

/// 按来源加载知识库，必要时执行 reload。
fn ensure_knowledge_source_loaded(
    layout: &RepoLayout,
    source: KnowledgeQuerySource,
    force_reload: bool,
) -> Result<(), AppError> {
    let should_reload = force_reload
        || match source {
            KnowledgeQuerySource::ConfiguredProvider => {
                should_reload_knowledge_source("configured")
            }
            KnowledgeQuerySource::LocalSqlite => should_reload_knowledge_source("sqlite"),
        };

    let result = match (source, should_reload) {
        (KnowledgeQuerySource::ConfiguredProvider, true) => reload_knowledge(layout),
        (KnowledgeQuerySource::ConfiguredProvider, false) => load_knowledge(layout),
        (KnowledgeQuerySource::LocalSqlite, true) => reload_sqlite_knowledge(layout),
        (KnowledgeQuerySource::LocalSqlite, false) => load_sqlite_knowledge(layout),
    };

    result.map_err(|e| AppError::validation(format!("加载知识库失败: {}", e)))
}

/// 按指定来源执行知识库 SQL 查询。
async fn debug_knowledge_query_rows_for_source_logic(
    table: Option<String>,
    source_kind: Option<String>,
    sql: String,
) -> Result<Vec<Vec<wp_model_core::model::DataField>>, AppError> {
    let sql = sql.trim().to_string();
    if sql.is_empty() {
        return Err(AppError::validation("SQL 不能为空"));
    }

    let setting = Setting::load();
    let layout = setting.wparse_layout();
    let provider_name = configured_provider_name(&layout)?;
    let source = resolve_knowledge_query_source(
        source_kind.as_deref(),
        table.as_deref(),
        provider_name.as_deref(),
    );

    ensure_knowledge_source_loaded(&layout, source, false)?;

    sql_query_rows(&sql)
        .await
        .map_err(|e| AppError::validation(format!("执行知识库 SQL 失败: {}", e)))
}
