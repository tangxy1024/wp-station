//! 项目导入摘要与预检辅助。
//!
//! 这一层负责在不改动真实仓库的前提下：
//! - 识别导入范围
//! - 构造预检目录
//! - 生成导入结果摘要

use std::path::Path;

use tempfile::tempdir;

use crate::constants::project::IMPORTABLE_ROOT_DIRS;
use crate::error::AppError;
use crate::server::RepoLayout;
use crate::utils::project_check::{ProjectCheckTarget, validate_project_in_dir};
use crate::utils::{
    ProjectSnapshot, SystemKind, compose_repo_layout_into, layout_for_system,
    load_project_snapshot, load_project_snapshot_from_repo_layout,
};

use super::{
    ImportScope, ProjectImportBreakdown, ProjectImportResponse, ProjectImportSummary,
    ProjectImportValidation,
};

/// 为归档预检构造摘要结果，不写入真实项目目录。
pub(super) fn validate_project_import_preview(
    system: SystemKind,
    source_dir: &Path,
) -> Result<ProjectImportSummary, AppError> {
    let scope = super::detect_import_scope(source_dir)?;
    let layout = layout_for_system(system).as_repo_layout();
    let preview_dir = build_preview_project_dir(system, source_dir, &layout, &scope)?;
    build_import_summary_from_dir(
        preview_dir.path(),
        source_dir.to_string_lossy().as_ref(),
        &layout,
        &scope,
    )
}

/// 构造预检用的临时项目目录，并在其中执行组件校验。
pub(super) fn build_preview_project_dir(
    system: SystemKind,
    source_dir: &Path,
    layout: &RepoLayout,
    scope: &ImportScope,
) -> Result<tempfile::TempDir, AppError> {
    let preview_dir = tempdir().map_err(AppError::internal)?;
    compose_repo_layout_into(layout, preview_dir.path())?;
    super::apply_import_scope_to_dir(source_dir, preview_dir.path(), scope)?;
    validate_project_in_dir(system, preview_dir.path(), ProjectCheckTarget::WholeProject)?;
    Ok(preview_dir)
}

/// 基于当前真实 layout 构造最终导入响应。
pub(super) fn build_import_response_from_repo_layout(
    layout: &RepoLayout,
    validation_message: &str,
    source_label: &str,
    scope: &ImportScope,
) -> Result<ProjectImportResponse, AppError> {
    let summary = build_import_summary_from_repo_layout(layout, source_label, scope)?;
    let validation = ProjectImportValidation {
        passed: true,
        message: scope.summary_message(validation_message),
    };

    Ok(ProjectImportResponse {
        summary,
        validation,
    })
}

/// 从当前真实 layout 构造导入摘要。
fn build_import_summary_from_repo_layout(
    layout: &RepoLayout,
    source_label: &str,
    scope: &ImportScope,
) -> Result<ProjectImportSummary, AppError> {
    let snapshot = load_project_snapshot_from_repo_layout(layout)?;
    build_import_summary_from_snapshot(
        snapshot,
        source_label,
        layout.models_root.to_string_lossy().to_string(),
        layout.infra_root.to_string_lossy().to_string(),
        scope,
    )
}

/// 从指定项目目录构造导入摘要。
fn build_import_summary_from_dir(
    project_dir: &Path,
    source_label: &str,
    layout: &RepoLayout,
    scope: &ImportScope,
) -> Result<ProjectImportSummary, AppError> {
    let snapshot = load_project_snapshot(project_dir)?;
    build_import_summary_from_snapshot(
        snapshot,
        source_label,
        layout.models_root.to_string_lossy().to_string(),
        layout.infra_root.to_string_lossy().to_string(),
        scope,
    )
}

/// 从项目快照构造统一导入摘要。
fn build_import_summary_from_snapshot(
    snapshot: ProjectSnapshot,
    source_label: &str,
    models_root: String,
    infra_root: String,
    scope: &ImportScope,
) -> Result<ProjectImportSummary, AppError> {
    if snapshot.rules.is_empty() && snapshot.knowledge.is_empty() {
        return Err(AppError::validation(
            "导入后的项目目录中未找到可导入的规则或知识库".to_string(),
        ));
    }

    let ProjectSnapshot {
        rules,
        knowledge,
        rule_stats,
        mut warnings,
        failed_files,
    } = snapshot;

    if !scope.retained_dir_names().is_empty() {
        warnings.push(format!(
            "本次归档仅覆盖目录: {}；保留当前目录: {}",
            scope.imported_dirs.join(", "),
            scope.retained_dir_names().join(", ")
        ));
    }

    let mut breakdown: Vec<ProjectImportBreakdown> = rule_stats
        .into_iter()
        .map(|(rule_type, count)| ProjectImportBreakdown {
            rule_type: rule_type.as_ref().to_string(),
            count,
        })
        .collect();
    breakdown.sort_by(|a, b| a.rule_type.cmp(&b.rule_type));

    Ok(ProjectImportSummary {
        rules_deleted: 0,
        rules_imported: rules.len(),
        knowledge_deleted: 0,
        knowledge_imported: knowledge.len(),
        imported_dirs: scope.imported_dir_names(),
        retained_dirs: scope.retained_dir_names(),
        rule_breakdown: breakdown,
        warnings,
        failed_files,
        source_dir: source_label.to_string(),
        models_root,
        infra_root,
    })
}

/// 在不覆盖真实项目目录的前提下校验导入范围是否有效。
pub(super) fn validate_import_scope_with_repo_layout(
    system: SystemKind,
    source_dir: &Path,
    layout: &RepoLayout,
    scope: &ImportScope,
) -> Result<(), AppError> {
    let _ = build_preview_project_dir(system, source_dir, layout, scope)?;
    Ok(())
}

/// 识别当前导入包实际包含哪些顶层目录。
pub(super) fn detect_import_scope(source_dir: &Path) -> Result<ImportScope, AppError> {
    let imported_dirs: Vec<&'static str> = IMPORTABLE_ROOT_DIRS
        .iter()
        .copied()
        .filter(|name| source_dir.join(name).is_dir())
        .collect();

    if imported_dirs.is_empty() {
        return Err(AppError::validation(
            "导入包中未找到可导入目录，至少需要包含 conf、connectors、topology、models 中的一个"
                .to_string(),
        ));
    }

    Ok(ImportScope { imported_dirs })
}
