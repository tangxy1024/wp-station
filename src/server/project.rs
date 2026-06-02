use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use tempfile::tempdir;
use zip::ZipArchive;

use crate::db::RuleType;
use crate::error::AppError;
use crate::server::sync::sync_to_gitea_all;
use crate::server::{
    OperationLogAction, OperationLogBiz, OperationLogParams, ProjectLayout, Setting,
    refresh_draft_release_logic, write_operation_log_for_result,
};
use crate::utils::knowledge::reload_knowledge;
use crate::utils::project_check::check_component_in_dir;
use crate::utils::{ProjectSnapshot, load_project_snapshot_from_layout};

const LEGACY_REQUIRED_DIRS: [&str; 4] = ["conf", "connectors", "topology", "models"];
const ARCHIVE_IMPORT_STAGING_DIR: &str = "project-archive-imports";

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

#[derive(Debug, Deserialize)]
pub struct ProjectArchiveConfirmRequest {
    pub import_id: String,
}

#[derive(Serialize)]
pub struct ProjectArchivePreviewResponse {
    pub import_id: String,
    pub file_name: String,
    pub summary: ProjectImportSummary,
    pub validation: ProjectImportValidation,
}

pub struct ProjectArchiveExport {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

pub async fn import_project_from_files_logic(
    operator: Option<String>,
    req: ProjectImportRequest,
) -> Result<ProjectImportResponse, AppError> {
    let operator_for_log = operator.clone();
    let setting = Setting::load();
    let layout = setting.project_layout();
    let source_dir = normalize_source_dir(&req.source_dir)?;

    let result = import_project_dir(&source_dir, &layout, "目录拆分覆盖并校验通过").await;

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

pub async fn preview_project_archive_logic(
    operator: Option<String>,
    file_name: &str,
    bytes: Vec<u8>,
) -> Result<ProjectArchivePreviewResponse, AppError> {
    let _ = operator;
    let import_id = new_archive_import_id();
    let staging_root = archive_import_staging_root();
    fs::create_dir_all(&staging_root).map_err(AppError::internal)?;
    let import_dir = staging_root.join(&import_id);
    fs::create_dir_all(&import_dir).map_err(AppError::internal)?;
    let archive_path = import_dir.join(sanitize_upload_name(file_name)?);
    fs::write(&archive_path, bytes).map_err(AppError::internal)?;
    let extract_dir = import_dir.join("extract");
    fs::create_dir_all(&extract_dir).map_err(AppError::internal)?;
    extract_archive(file_name, &archive_path, &extract_dir)?;

    let project_dir = find_import_project_root(&extract_dir)?;
    let summary = validate_project_import_preview(&project_dir)?;

    Ok(ProjectArchivePreviewResponse {
        import_id,
        file_name: file_name.to_string(),
        summary,
        validation: ProjectImportValidation {
            passed: true,
            message: "归档校验通过，请确认导入".to_string(),
        },
    })
}

pub async fn confirm_project_archive_import_logic(
    operator: Option<String>,
    import_id: &str,
) -> Result<ProjectImportResponse, AppError> {
    let import_dir = archive_import_dir(import_id)?;
    let project_dir_file = import_dir.join("project_dir.txt");
    let project_dir = fs::read_to_string(&project_dir_file)
        .map_err(|e| AppError::validation(format!("导入暂存记录不存在或已过期: {}", e)))?;
    let project_dir = PathBuf::from(project_dir.trim());
    if !project_dir.is_dir() {
        return Err(AppError::validation(
            "导入暂存目录不存在或已过期".to_string(),
        ));
    }

    let setting = Setting::load();
    let layout = setting.project_layout();
    let result = import_project_dir(&project_dir, &layout, "归档覆盖导入并校验通过").await;

    let mut log_params = OperationLogParams::new()
        .with_target_name(import_id)
        .with_field("import_id", import_id)
        .with_field("source", project_dir.to_string_lossy().to_string());
    if let Some(operator) = operator {
        log_params = log_params.with_operator(operator);
    }
    if let Ok(ref resp) = result {
        log_params = log_params
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

    if result.is_ok()
        && let Err(err) = fs::remove_dir_all(&import_dir)
    {
        warn!(
            "清理导入暂存目录失败: import_id={}, path={}, error={}",
            import_id,
            import_dir.display(),
            err
        );
    }

    result
}

pub async fn export_project_archive_logic() -> Result<ProjectArchiveExport, AppError> {
    let setting = Setting::load();
    let layout = setting.project_layout();
    let temp = tempdir().map_err(AppError::internal)?;
    let export_root = temp.path().join("wp-station-project");
    fs::create_dir_all(&export_root).map_err(AppError::internal)?;

    copy_dir_recursive(&layout.models_root, &export_root.join("project_models"))?;
    copy_dir_recursive(&layout.infra_root, &export_root.join("project_infra"))?;

    let archive_path = temp.path().join("wp-station-project.tar.gz");
    let archive_file = File::create(&archive_path).map_err(AppError::internal)?;
    let encoder = GzEncoder::new(archive_file, Compression::default());
    let mut builder = tar::Builder::new(encoder);
    builder
        .append_dir_all("wp-station-project", &export_root)
        .map_err(AppError::internal)?;
    let encoder = builder.into_inner().map_err(AppError::internal)?;
    encoder.finish().map_err(AppError::internal)?;

    let mut bytes = Vec::new();
    File::open(&archive_path)
        .map_err(AppError::internal)?
        .read_to_end(&mut bytes)
        .map_err(AppError::internal)?;
    let file_name = format!(
        "wp-station-project-{}.tar.gz",
        Utc::now().format("%Y%m%d%H%M%S")
    );

    Ok(ProjectArchiveExport { file_name, bytes })
}

async fn import_project_dir(
    source_dir: &Path,
    layout: &ProjectLayout,
    validation_message: &str,
) -> Result<ProjectImportResponse, AppError> {
    validate_legacy_project_dir(source_dir)?;
    check_component_in_dir(source_dir, RuleType::All.to_check_component())?;
    overwrite_project_layout_from_legacy_dir(source_dir, layout)?;

    let snapshot = load_project_snapshot_from_layout(layout)?;
    if snapshot.rules.is_empty() && snapshot.knowledge.is_empty() {
        return Err(AppError::validation(
            "导入后的项目目录中未找到可导入的规则或知识库".to_string(),
        ));
    }

    let ProjectSnapshot {
        rules,
        knowledge,
        rule_stats,
        warnings,
        failed_files,
    } = snapshot;

    let total_rules = rules.len();
    let total_knowledge = knowledge.len();

    if let Err(err) = reload_knowledge(layout) {
        warn!("知识库重载失败（忽略）: {}", err);
    }

    let commit_message = format!("导入项目配置 {}", Utc::now().format("%Y-%m-%d %H:%M:%S"));
    sync_to_gitea_all(&commit_message).await;
    let _ = refresh_draft_release_logic(Some(&commit_message)).await;

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
        message: validation_message.to_string(),
    };

    Ok(ProjectImportResponse {
        summary,
        validation,
    })
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

fn archive_import_staging_root() -> PathBuf {
    std::env::temp_dir().join(ARCHIVE_IMPORT_STAGING_DIR)
}

fn new_archive_import_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("import-{}-{}", std::process::id(), millis)
}

fn archive_import_dir(import_id: &str) -> Result<PathBuf, AppError> {
    let valid = !import_id.is_empty()
        && import_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
    if !valid {
        return Err(AppError::validation("导入任务 ID 无效".to_string()));
    }
    Ok(archive_import_staging_root().join(import_id))
}

fn validate_project_import_preview(source_dir: &Path) -> Result<ProjectImportSummary, AppError> {
    validate_legacy_project_dir(source_dir)?;
    check_component_in_dir(source_dir, RuleType::All.to_check_component())?;

    let snapshot = crate::utils::load_project_snapshot(source_dir)?;
    if snapshot.rules.is_empty() && snapshot.knowledge.is_empty() {
        return Err(AppError::validation(
            "导入包中未找到可导入的规则或知识库".to_string(),
        ));
    }

    let ProjectSnapshot {
        rules,
        knowledge,
        rule_stats,
        warnings,
        failed_files,
    } = snapshot;
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
        rule_breakdown: breakdown,
        warnings,
        failed_files,
        source_dir: source_dir.to_string_lossy().to_string(),
        project_models: "preview".to_string(),
        project_infra: "preview".to_string(),
    })
}

fn sanitize_upload_name(file_name: &str) -> Result<String, AppError> {
    let name = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .ok_or_else(|| AppError::validation("上传文件名无效".to_string()))?;

    if is_supported_archive(&name) {
        Ok(name)
    } else {
        Err(AppError::validation(
            "仅支持 tar、tar.gz、tgz、zip 格式".to_string(),
        ))
    }
}

fn is_supported_archive(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".tar")
        || lower.ends_with(".tar.gz")
        || lower.ends_with(".tgz")
        || lower.ends_with(".zip")
}

fn extract_archive(
    file_name: &str,
    archive_path: &Path,
    target_dir: &Path,
) -> Result<(), AppError> {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        extract_zip_archive(archive_path, target_dir)
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        let file = File::open(archive_path).map_err(AppError::internal)?;
        let decoder = GzDecoder::new(file);
        extract_tar_reader(decoder, target_dir)
    } else if lower.ends_with(".tar") {
        let file = File::open(archive_path).map_err(AppError::internal)?;
        extract_tar_reader(file, target_dir)
    } else {
        Err(AppError::validation(
            "仅支持 tar、tar.gz、tgz、zip 格式".to_string(),
        ))
    }
}

fn extract_tar_reader<R: Read>(reader: R, target_dir: &Path) -> Result<(), AppError> {
    let mut archive = tar::Archive::new(reader);
    for entry in archive.entries().map_err(AppError::internal)? {
        let mut entry = entry.map_err(AppError::internal)?;
        let entry_path = entry.path().map_err(AppError::internal)?.to_path_buf();
        if should_skip_archive_path(&entry_path) {
            continue;
        }
        let safe_path = safe_join(target_dir, &entry_path)?;
        if let Some(parent) = safe_path.parent() {
            fs::create_dir_all(parent).map_err(AppError::internal)?;
        }
        entry.unpack(&safe_path).map_err(AppError::internal)?;
    }
    Ok(())
}

fn extract_zip_archive(archive_path: &Path, target_dir: &Path) -> Result<(), AppError> {
    let file = File::open(archive_path).map_err(AppError::internal)?;
    let mut archive = ZipArchive::new(file).map_err(AppError::internal)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(AppError::internal)?;
        let Some(enclosed_name) = entry.enclosed_name().map(|path| path.to_path_buf()) else {
            continue;
        };
        if should_skip_archive_path(&enclosed_name) {
            continue;
        }
        let out_path = safe_join(target_dir, &enclosed_name)?;
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(AppError::internal)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(AppError::internal)?;
            }
            let mut out_file = File::create(&out_path).map_err(AppError::internal)?;
            std::io::copy(&mut entry, &mut out_file).map_err(AppError::internal)?;
        }
    }
    Ok(())
}

fn should_skip_archive_path(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name == "__MACOSX" || name == ".DS_Store" || name.starts_with("._")
    })
}

fn safe_join(root: &Path, relative: &Path) -> Result<PathBuf, AppError> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(AppError::validation(format!(
            "压缩包包含不安全路径: {}",
            relative.display()
        )));
    }
    Ok(root.join(relative))
}

fn find_import_project_root(extract_dir: &Path) -> Result<PathBuf, AppError> {
    if is_default_configs_layout(extract_dir) || is_split_repo_layout(extract_dir) {
        let normalized = normalize_import_root(extract_dir)?;
        persist_preview_project_dir(extract_dir, &normalized)?;
        return Ok(normalized);
    }

    let mut dirs = Vec::new();
    for entry in fs::read_dir(extract_dir).map_err(AppError::internal)? {
        let entry = entry.map_err(AppError::internal)?;
        if entry.path().is_dir() {
            dirs.push(entry.path());
        }
    }

    if dirs.len() == 1 && (is_default_configs_layout(&dirs[0]) || is_split_repo_layout(&dirs[0])) {
        let normalized = normalize_import_root(&dirs[0])?;
        persist_preview_project_dir(extract_dir, &normalized)?;
        Ok(normalized)
    } else {
        Err(AppError::validation(
            "压缩包结构无效，需包含 conf/connectors/models/topology 或 project_models/project_infra"
                .to_string(),
        ))
    }
}

fn persist_preview_project_dir(extract_dir: &Path, project_dir: &Path) -> Result<(), AppError> {
    let Some(import_dir) = extract_dir.parent() else {
        return Ok(());
    };
    fs::write(
        import_dir.join("project_dir.txt"),
        project_dir.to_string_lossy().as_ref(),
    )
    .map_err(AppError::internal)
}

fn is_default_configs_layout(dir: &Path) -> bool {
    LEGACY_REQUIRED_DIRS
        .iter()
        .all(|name| dir.join(name).is_dir())
}

fn is_split_repo_layout(dir: &Path) -> bool {
    dir.join("project_models").join("models").is_dir()
        && dir.join("project_infra").join("conf").is_dir()
        && dir.join("project_infra").join("connectors").is_dir()
        && dir.join("project_infra").join("topology").is_dir()
}

fn normalize_import_root(dir: &Path) -> Result<PathBuf, AppError> {
    if is_default_configs_layout(dir) {
        return Ok(dir.to_path_buf());
    }

    let normalized = dir.join("__normalized_default_configs");
    fs::create_dir_all(&normalized).map_err(AppError::internal)?;
    copy_named_entry(&dir.join("project_models"), &normalized, "models")?;
    copy_named_entry(&dir.join("project_infra"), &normalized, "conf")?;
    copy_named_entry(&dir.join("project_infra"), &normalized, "connectors")?;
    copy_named_entry(&dir.join("project_infra"), &normalized, "topology")?;
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
        if should_skip_archive_path(&PathBuf::from(entry.file_name())) {
            continue;
        }
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
