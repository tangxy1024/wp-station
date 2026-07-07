//! 项目组件校验模块。
//!
//! 按系统分发项目组件（规则、配置等）的完整性校验。

use crate::db::RuleType;
use wp_proj::project::{
    CheckComponents, WarpProject,
    checker::{self, CheckComponent, CheckOptions},
    init::PrjScope,
};

use crate::Setting;
use crate::error::AppError;
use crate::utils::{SystemKind, compose_repo_layout_into, layout_for_system};
use std::{path::Path, process::Command};

/// 系统级项目校验目标。
#[derive(Debug, Clone, Copy)]
pub enum ProjectCheckTarget {
    /// 校验整个项目。
    WholeProject,
    /// 校验某一种规则/配置类型。
    RuleType(RuleType),
}

impl ProjectCheckTarget {
    fn to_wparse_components(self) -> Vec<CheckComponent> {
        match self {
            ProjectCheckTarget::WholeProject => RuleType::All.to_wparse_check_components(),
            ProjectCheckTarget::RuleType(rule_type) => rule_type.to_wparse_check_components(),
        }
    }
}

/// 校验项目组件（全局共享项目目录）。
///
/// `wparse` 会合成双仓库到临时目录后调用 `wp_proj`；
/// `wfusion` 先保留入口，避免误调用 `wproj check`。
pub fn validate_project(system: SystemKind, target: ProjectCheckTarget) -> Result<(), AppError> {
    let layout = layout_for_system(system).as_repo_layout();
    let tmp_dir = Setting::workspace_root()
        .join("tmp")
        .join("project-check")
        .join(format!("{}", chrono::Utc::now().timestamp_millis()));
    std::fs::create_dir_all(&tmp_dir).map_err(AppError::internal)?;
    compose_repo_layout_into(&layout, &tmp_dir)?;
    let result = validate_project_in_dir(system, &tmp_dir, target);
    let _ = std::fs::remove_dir_all(&tmp_dir);
    result
}

/// 对指定目录执行系统级项目校验。
pub fn validate_project_in_dir(
    system: SystemKind,
    project_path: &Path,
    target: ProjectCheckTarget,
) -> Result<(), AppError> {
    match system {
        SystemKind::Wparse => {
            check_wparse_components_in_dir(project_path, target.to_wparse_components())
        }
        SystemKind::Wfusion => check_wfusion_in_dir(project_path, target),
    }
}

/// 对指定目录执行 `wparse/wproj` 组件校验。
fn check_wparse_components_in_dir(
    project_path: &Path,
    components: Vec<CheckComponent>,
) -> Result<(), AppError> {
    if !project_path.exists() {
        return Err(AppError::Validation(format!(
            "项目路径不存在: {}",
            project_path.display()
        )));
    }

    // 转换为绝对路径（规范化路径，去除 ./ ../ 等）
    let project_path = project_path.canonicalize().map_err(|e| {
        AppError::Validation(format!(
            "无法规范化项目路径: {} ({})",
            project_path.display(),
            e
        ))
    })?;

    let project_path_str = project_path
        .to_str()
        .ok_or_else(|| AppError::Validation("项目路径包含无效字符".to_string()))?
        .to_string();

    let dict = Default::default();
    let project = WarpProject::load(&project_path_str, PrjScope::Normal, &dict)
        .map_err(|e| AppError::Validation(format!("加载项目失败: {}", e)))?;

    let mut opts = CheckOptions::new(project_path_str);
    opts.console = true;
    opts.fail_fast = true;

    let components = CheckComponents::default().with_only(components);

    checker::check_with(&project, &opts, &components, &dict)
        .map_err(|e| AppError::Validation(format!("组件校验失败: {}", e)))?;

    Ok(())
}

/// 对指定目录执行 `wfusion/wfadm` 项目校验。
///
/// `wfadm` 当前只支持整体校验，因此 Station 在单文件校验场景下，
/// 会先把当前编辑内容覆盖到临时项目，再执行一次整体校验。
fn check_wfusion_in_dir(project_path: &Path, target: ProjectCheckTarget) -> Result<(), AppError> {
    if !project_path.exists() {
        return Err(AppError::Validation(format!(
            "项目路径不存在: {}",
            project_path.display()
        )));
    }

    let _ = target;

    let output = Command::new("wfadm")
        .arg("check")
        .current_dir(project_path)
        .output()
        .map_err(|e| AppError::internal(format!("执行 wfadm check 失败: {}", e)))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "未提供错误输出".to_string()
    };

    Err(AppError::validation(format!(
        "wfadm check 校验失败: {}",
        detail
    )))
}
