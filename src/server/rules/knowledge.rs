//! 知识库配置相关业务。
//!
//! 包括知识库目录内容保存，以及 `knowdb.toml` 的读写。

use crate::constants::project::FILE_KNOWDB;
use crate::error::AppError;
use crate::server::sync::sync_to_gitea;
use crate::server::{
    OperationLogAction, OperationLogBiz, OperationLogParams, refresh_draft_release_logic,
    write_operation_log_for_result,
};
use crate::utils::knowledge::reload_knowledge;
use crate::utils::{
    SystemKind, read_knowdb_config, read_knowledge_files, wfusion_not_implemented,
    write_knowdb_config, write_knowledge_files,
};

use super::{KnowdbConfigResponse, repo_layout, system_time_to_rfc3339};

/// 保存知识库规则配置。
pub async fn save_knowledge_rule_logic(
    system: SystemKind,
    file: String,
    config: Option<String>,
    create_sql: Option<String>,
    insert_sql: Option<String>,
    data: Option<String>,
    _operator: Option<String>,
) -> Result<(), AppError> {
    info!("保存知识库规则配置: file={}", file);
    if matches!(system, SystemKind::Wfusion) {
        return Err(wfusion_not_implemented("知识库保存"));
    }

    let layout = repo_layout(system);
    let is_update = read_knowledge_files(&layout, &file)?.is_some();
    let file_clone = file.clone();
    let config_clone = config.clone();
    let create_sql_clone = create_sql.clone();
    let insert_sql_clone = insert_sql.clone();
    let data_clone = data.clone();

    let file_for_log = file_clone.clone();
    let result = async move {
        let table_path = write_knowledge_files(&layout, &file, create_sql, insert_sql, data)?;
        if let Some(config_content) = config {
            let knowdb_path = write_knowdb_config(&layout, &config_content)?;
            debug!("全局 knowdb 配置已随知识库保存更新: path={}", knowdb_path);
        }

        info!("知识库规则配置保存成功: file={}, path={}", file, table_path);

        reload_knowledge(&layout).map_err(AppError::internal)?;

        let commit_message = format!("知识库改动: {}", file);
        sync_to_gitea(&commit_message, system, crate::db::ReleaseGroup::Models).await?;
        let _ = refresh_draft_release_logic(system, Some(&commit_message)).await;

        Ok::<_, AppError>(())
    }
    .await;

    write_operation_log_for_result(
        OperationLogBiz::KnowledgeConfig,
        if is_update {
            OperationLogAction::Update
        } else {
            OperationLogAction::Create
        },
        OperationLogParams::new()
            .with_target_name(file_for_log)
            .with_field("config", if config_clone.is_some() { "yes" } else { "no" })
            .with_field(
                "create_sql",
                if create_sql_clone.is_some() {
                    "yes"
                } else {
                    "no"
                },
            )
            .with_field(
                "insert_sql",
                if insert_sql_clone.is_some() {
                    "yes"
                } else {
                    "no"
                },
            )
            .with_field("data", if data_clone.is_some() { "yes" } else { "no" })
            .with_field("sync", "project+gitea")
            .with_field("knowledge_reload", "yes"),
        &result,
    )
    .await;

    result
}

/// 获取 `knowdb.toml` 内容。
pub async fn get_knowdb_config_logic(system: SystemKind) -> Result<KnowdbConfigResponse, AppError> {
    if matches!(system, SystemKind::Wfusion) {
        return Err(wfusion_not_implemented("knowdb 读取"));
    }
    let layout = repo_layout(system);
    let entry = read_knowdb_config(&layout)?;
    let response = KnowdbConfigResponse {
        file: FILE_KNOWDB.to_string(),
        content: entry.as_ref().map(|(content, _)| content.clone()),
        last_modified: entry
            .as_ref()
            .map(|(_, modified)| system_time_to_rfc3339(*modified)),
    };
    Ok(response)
}

/// 保存 `knowdb.toml` 全局配置。
pub async fn save_knowdb_config_logic(
    system: SystemKind,
    content: Option<String>,
    _operator: Option<String>,
) -> Result<(), AppError> {
    info!("保存 knowdb 配置");
    if matches!(system, SystemKind::Wfusion) {
        return Err(wfusion_not_implemented("knowdb 保存"));
    }
    let layout = repo_layout(system);

    let result = async move {
        let content = content.unwrap_or_default();
        let written_path = write_knowdb_config(&layout, &content)?;
        info!("knowdb 配置保存成功: path={}", written_path);

        reload_knowledge(&layout).map_err(AppError::internal)?;

        let commit_message = format!("知识库改动: {}", FILE_KNOWDB);
        sync_to_gitea(&commit_message, system, crate::db::ReleaseGroup::Models).await?;
        let _ = refresh_draft_release_logic(system, Some(&commit_message)).await;

        Ok::<_, AppError>(())
    }
    .await;

    write_operation_log_for_result(
        OperationLogBiz::KnowledgeConfig,
        OperationLogAction::Update,
        OperationLogParams::new()
            .with_target_name(FILE_KNOWDB)
            .with_field("config_only", "yes")
            .with_field("sync", "project+gitea")
            .with_field("knowledge_reload", "yes"),
        &result,
    )
    .await;

    result
}
