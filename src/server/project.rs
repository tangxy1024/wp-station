use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::db::RuleType;
use crate::error::AppError;
use crate::server::sync::sync_to_gitea_all;
use crate::server::{
    OperationLogAction, OperationLogBiz, OperationLogParams, ProjectLayout, Setting,
    write_operation_log_for_result,
};
use crate::utils::knowledge::reload_knowledge;
use crate::utils::project_check::check_component_in_dir;
use crate::utils::{ProjectSnapshot, load_project_snapshot_from_layout};

const LEGACY_REQUIRED_DIRS: [&str; 4] = ["conf", "connectors", "topology", "models"];

#[derive(Debug, Deserialize)]
pub struct ProjectImportRequest {
    pub source_dir: String,
}

#[derive(Serialize)]
pub struct ProjectImportResponse {
    pub summary: ProjectImportSummary,
    pub validation: ProjectImportValidation,
}

#[derive(Serialize)]
pub struct ProjectImportSummary {
    pub rules_deleted: usize,
    pub rules_imported: usize,
    pub knowledge_deleted: usize,
    pub knowledge_imported: usize,
    pub rule_breakdown: Vec<ProjectImportBreakdown>,
    pub warnings: Vec<String>,
    pub failed_files: usize,
    pub source_dir: String,
    pub project_models: String,
    pub project_infra: String,
}

#[derive(Serialize)]
pub struct ProjectImportBreakdown {
    pub rule_type: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct ProjectImportValidation {
    pub passed: bool,
    pub message: String,
}

pub async fn import_project_from_files_logic(
    operator: Option<String>,
    req: ProjectImportRequest,
) -> Result<ProjectImportResponse, AppError> {
    let operator_for_log = operator.clone();
    let setting = Setting::load();
    let layout = setting.project_layout();
    let source_dir = normalize_source_dir(&req.source_dir)?;

    validate_legacy_project_dir(&source_dir)?;
    check_component_in_dir(&source_dir, RuleType::All.to_check_component())?;
    overwrite_project_layout_from_legacy_dir(&source_dir, &layout)?;

    let snapshot = load_project_snapshot_from_layout(&layout)?;
    if snapshot.rules.is_empty() && snapshot.knowledge.is_empty() {
        return Err(AppError::validation(
            "导入后的项目目录中未找到可导入的规则或知识库".to_string(),
        ));
    }

    let result = async {
        let ProjectSnapshot {
            rules,
            knowledge,
            rule_stats,
            warnings,
            failed_files,
        } = snapshot;

        let total_rules = rules.len();
        let total_knowledge = knowledge.len();

        if let Err(err) = reload_knowledge(&layout) {
            warn!("知识库重载失败（忽略）: {}", err);
        }

        let commit_message = format!("初始化更新配置 {}", Utc::now().format("%Y-%m-%d %H:%M:%S"));
        sync_to_gitea_all(&commit_message).await;

        let mut breakdown: Vec<ProjectImportBreakdown> = rule_stats
            .into_iter()
            .map(|(rule_type, count)| ProjectImportBreakdown {
                rule_type: rule_type.as_ref().to_string(),
                count,
            })
            .collect();
        breakdown.sort_by(|a, b| a.rule_type.cmp(&b.rule_type));

        let summary = ProjectImportSummary {
            rules_deleted: 0,
            rules_imported: total_rules,
            knowledge_deleted: 0,
            knowledge_imported: total_knowledge,
            rule_breakdown: breakdown,
            warnings,
            failed_files,
            source_dir: source_dir.to_string_lossy().to_string(),
            project_models: layout.models_root.to_string_lossy().to_string(),
            project_infra: layout.infra_root.to_string_lossy().to_string(),
        };

        let validation = ProjectImportValidation {
            passed: true,
            message: "目录拆分覆盖并校验通过".to_string(),
        };

        Ok::<_, AppError>(ProjectImportResponse {
            summary,
            validation,
        })
    }
    .await;

    let mut log_params = OperationLogParams::new();
    if let Some(op) = operator_for_log {
        log_params = log_params.with_operator(op);
    }
    log_params = log_params.with_field("source_dir", source_dir.to_string_lossy().to_string());
    if let Ok(ref resp) = result {
        log_params = log_params
            .with_field("rules_deleted", resp.summary.rules_deleted.to_string())
            .with_field("rules_imported", resp.summary.rules_imported.to_string())
            .with_field(
                "knowledge_imported",
                resp.summary.knowledge_imported.to_string(),
            )
            .with_field("project_models", resp.summary.project_models.clone())
            .with_field("project_infra", resp.summary.project_infra.clone());
    }

    write_operation_log_for_result(
        OperationLogBiz::RuleFile,
        OperationLogAction::Update,
        log_params,
        &result,
    )
    .await;

    result
}

fn normalize_source_dir(raw: &str) -> Result<PathBuf, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("请输入待导入的源目录".to_string()));
    }

    let path = PathBuf::from(trimmed);
    let normalized = if path.is_absolute() {
        path
    } else {
        Setting::workspace_root().join(path)
    };

    if !normalized.is_dir() {
        return Err(AppError::validation(format!(
            "源目录不存在或不是目录: {}",
            normalized.display()
        )));
    }

    Ok(normalized)
}

fn validate_legacy_project_dir(source_dir: &Path) -> Result<(), AppError> {
    let missing_dirs: Vec<String> = LEGACY_REQUIRED_DIRS
        .iter()
        .filter_map(|name| {
            let path = source_dir.join(name);
            if path.is_dir() {
                None
            } else {
                Some((*name).to_string())
            }
        })
        .collect();

    if !missing_dirs.is_empty() {
        return Err(AppError::validation(format!(
            "源目录结构不完整，缺少必要目录: {}",
            missing_dirs.join(", ")
        )));
    }

    Ok(())
}

fn overwrite_project_layout_from_legacy_dir(
    source_dir: &Path,
    layout: &ProjectLayout,
) -> Result<(), AppError> {
    info!(
        "开始按旧目录结构拆分覆盖双仓库: source_dir={}, models_dir={}, infra_dir={}",
        source_dir.display(),
        layout.models_root.display(),
        layout.infra_root.display()
    );

    recreate_dir_preserving_git(&layout.models_root)?;
    recreate_dir_preserving_git(&layout.infra_root)?;

    copy_named_entry(source_dir, &layout.infra_root, "conf")?;
    copy_named_entry(source_dir, &layout.infra_root, "connectors")?;
    copy_named_entry(source_dir, &layout.infra_root, "topology")?;
    copy_named_entry(source_dir, &layout.models_root, "models")?;

    info!(
        "旧目录拆分覆盖完成: source_dir={}, models_dir={}, infra_dir={}",
        source_dir.display(),
        layout.models_root.display(),
        layout.infra_root.display()
    );
    Ok(())
}

fn recreate_dir_preserving_git(target_dir: &Path) -> Result<(), AppError> {
    fs::create_dir_all(target_dir).map_err(AppError::internal)?;

    for entry in fs::read_dir(target_dir).map_err(AppError::internal)? {
        let entry = entry.map_err(AppError::internal)?;
        let path = entry.path();
        if entry.file_name() == OsStr::new(".git") {
            continue;
        }

        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(AppError::internal)?;
        } else {
            fs::remove_file(&path).map_err(AppError::internal)?;
        }
    }

    Ok(())
}

fn copy_named_entry(source_root: &Path, target_root: &Path, name: &str) -> Result<(), AppError> {
    let source = source_root.join(name);
    if !source.exists() {
        return Err(AppError::validation(format!(
            "源目录缺少必要内容: {}",
            source.display()
        )));
    }

    let target = target_root.join(name);
    if target.exists() {
        if target.is_dir() {
            fs::remove_dir_all(&target).map_err(AppError::internal)?;
        } else {
            fs::remove_file(&target).map_err(AppError::internal)?;
        }
    }

    if source.is_dir() {
        copy_dir_recursive(&source, &target)?;
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(AppError::internal)?;
        }
        fs::copy(&source, &target).map_err(AppError::internal)?;
    }

    Ok(())
}

fn copy_dir_recursive(source_dir: &Path, target_dir: &Path) -> Result<(), AppError> {
    fs::create_dir_all(target_dir).map_err(AppError::internal)?;

    for entry in fs::read_dir(source_dir).map_err(AppError::internal)? {
        let entry = entry.map_err(AppError::internal)?;
        let source_path = entry.path();
        let target_path = target_dir.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if source_path.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(AppError::internal)?;
            }
            fs::copy(&source_path, &target_path).map_err(AppError::internal)?;
        }
    }

    Ok(())
}
