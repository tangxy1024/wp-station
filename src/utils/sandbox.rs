//! 沙盒运行时管理模块。
//!
//! **沙盒内部专用模块，不允许被其他 API 或业务功能直接调用。**
//! 所有 `pub` 项仅供 `src/server/sandbox*.rs` 使用。
//!
//! 包含两大子模块：
//! - 工作区管理：目录创建、项目复制、文件覆盖、日志打包
//! - 进程管理：wparse daemon 启动/终止、wpgen/wproj 命令执行、端口可用性检查

use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::UdpSocket;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::time::sleep;

use crate::error::AppError;
use crate::server::{FileOverride, Setting, sandbox::OutputFileStatus};
use crate::utils::common::{
    BUSINESS_SINK_OVERRIDE, OUTPUT_PATHS, SANDBOX_RUNTIME_OUTPUT_CONNECTOR,
    SANDBOX_RUNTIME_SOURCE_CONNECTOR, SANDBOX_RUNTIME_SOURCE_KEY, SANDBOX_RUNTIME_UDP_PORT,
};
use crate::utils::compose_project_layout_into;

// ============ 命令解析 ============

/// 命令查找的优先搜索路径，沙盒环境中的 toolchain 安装目录。
const TOOLCHAIN_SEARCH_PATHS: [&str; 2] = ["/app", "/app/toolchain"];

/// 在预设搜索路径中定位命令二进制，若找不到则回退到 PATH 查找。
fn resolve_toolchain_command(cmd: &str) -> PathBuf {
    for base in TOOLCHAIN_SEARCH_PATHS {
        let candidate = Path::new(base).join(cmd);
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(cmd)
}

// ============ 沙盒工作区 ============

/// 管理沙盒运行时的临时项目目录与日志目录。
///
/// 每次沙盒任务会将双仓库合成到临时目录，并应用用户覆盖文件及沙盒运行时必需配置。
#[derive(Debug, Clone)]
pub struct SandboxWorkspace {
    /// 沙盒根目录（包含 project/ 和 logs/）。
    pub root: PathBuf,
    /// 沙盒项目目录（由 models + infra 合成而来）。
    pub project_dir: PathBuf,
    /// 日志目录。
    pub logs_dir: PathBuf,
    /// 源 models 仓库目录。
    pub source_models_root: PathBuf,
    /// 源 infra 仓库目录。
    pub source_infra_root: PathBuf,
}

impl SandboxWorkspace {
    /// 准备沙盒目录：复制项目模板并应用覆盖文件。
    pub fn prepare(task_id: &str, overrides: &[FileOverride]) -> Result<Self, AppError> {
        let workspace_root = Setting::workspace_root().clone();
        let base_dir = workspace_root.join("tmp").join("sandbox").join(task_id);
        if base_dir.exists() {
            fs::remove_dir_all(&base_dir).map_err(AppError::internal)?;
        }

        let project_dir = base_dir.join("project");
        let logs_dir = base_dir.join("logs");

        fs::create_dir_all(&project_dir).map_err(AppError::internal)?;
        fs::create_dir_all(&logs_dir).map_err(AppError::internal)?;

        let setting = Setting::load();
        let layout = setting.project_layout();
        compose_project_layout_into(&layout, &project_dir)?;

        apply_overrides(&project_dir, overrides)?;
        apply_sandbox_runtime_overrides(&project_dir)?;

        Ok(SandboxWorkspace {
            root: base_dir,
            project_dir,
            logs_dir,
            source_models_root: layout.models_root,
            source_infra_root: layout.infra_root,
        })
    }

    /// 返回指定日志文件的绝对路径。
    pub fn log_path(&self, name: &str) -> PathBuf {
        self.logs_dir.join(name)
    }

    /// 写入文本日志，返回生成的路径。
    pub fn write_text_log(&self, name: &str, content: &str) -> Result<PathBuf, AppError> {
        let target = self.log_path(name);
        fs::write(&target, content).map_err(AppError::internal)?;
        Ok(target)
    }

    /// 任务完成后清理由工具生成的 project 目录。
    /// 若 `keep_workspace` 为 `true` 则保留现场以便人工排查。
    pub fn cleanup_after_run(&self, keep_workspace: bool) -> Result<(), AppError> {
        if keep_workspace {
            return Ok(());
        }
        if self.project_dir.exists() {
            fs::remove_dir_all(&self.project_dir).map_err(AppError::internal)?;
        }
        Ok(())
    }

    /// 以树结构形式渲染目录概览，便于调试日志展示。
    pub fn render_tree_listing(
        &self,
        max_depth: usize,
        max_entries: usize,
    ) -> Result<String, AppError> {
        let root_label = self.display_relative(&self.project_dir);
        render_tree(&self.project_dir, &root_label, max_depth, max_entries)
    }

    /// 将路径转换为相对 base 目录的可读字符串。
    pub fn display_relative(&self, path: &Path) -> String {
        relative_to_workspace_root(path)
            .or_else(|| path.strip_prefix(&self.root).ok().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| path.to_path_buf())
            .to_string_lossy()
            .to_string()
    }
}

/// 收集 wparse 输出目录中的关键文件状态，供分析阶段判断是否有数据产出。
pub fn collect_output_checks(project_dir: &Path) -> Result<Vec<OutputFileStatus>, AppError> {
    let mut results = Vec::new();
    for (relative, meaning) in OUTPUT_PATHS {
        let path = project_dir.join(relative);
        let line_count = if path.exists() {
            count_lines(&path)?
        } else {
            0
        };
        results.push(OutputFileStatus {
            relative_path: relative.to_string(),
            is_empty: line_count == 0,
            line_count,
            meaning: meaning.to_string(),
        });
    }
    Ok(results)
}

/// 统计文件行数，用于判断输出文件是否为空。
fn count_lines(path: &Path) -> Result<usize, AppError> {
    let mut file = fs::File::open(path).map_err(AppError::internal)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).map_err(AppError::internal)?;
    Ok(buf.lines().count())
}

/// 应用用户提交的文件覆盖，写入选定文件到沙盒项目目录。
fn apply_overrides(project_dir: &Path, overrides: &[FileOverride]) -> Result<(), AppError> {
    for override_file in overrides {
        let relative = override_file.file.trim();
        if relative.is_empty() {
            continue;
        }
        validate_override_path(relative)?;
        let target = project_dir.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(AppError::internal)?;
        }
        let mut file = fs::File::create(&target).map_err(AppError::internal)?;
        file.write_all(override_file.content.as_bytes())
            .map_err(AppError::internal)?;
    }
    Ok(())
}

/// 将指定内容写入 project_dir 内的相对路径文件。
fn write_override_file(project_dir: &Path, relative: &str, content: &str) -> Result<(), AppError> {
    validate_override_path(relative)?;
    let target = project_dir.join(relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(AppError::internal)?;
    }
    fs::write(&target, content).map_err(AppError::internal)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum SandboxOverrideKind {
    PatchWparseAdminApi,
    PatchWpsrcRuntime,
    PatchWpgenRuntime,
    RewriteBusinessSink,
}

#[derive(Clone, Copy)]
struct SandboxOverrideSpec {
    relative_path: &'static str,
    kind: SandboxOverrideKind,
}

const SANDBOX_OVERRIDE_SPECS: [SandboxOverrideSpec; 4] = [
    SandboxOverrideSpec {
        relative_path: "conf/wparse.toml",
        kind: SandboxOverrideKind::PatchWparseAdminApi,
    },
    SandboxOverrideSpec {
        relative_path: "topology/sources/wpsrc.toml",
        kind: SandboxOverrideKind::PatchWpsrcRuntime,
    },
    SandboxOverrideSpec {
        relative_path: "conf/wpgen.toml",
        kind: SandboxOverrideKind::PatchWpgenRuntime,
    },
    SandboxOverrideSpec {
        relative_path: "topology/sinks/business.d/sink.toml",
        kind: SandboxOverrideKind::RewriteBusinessSink,
    },
];

impl SandboxOverrideSpec {
    fn summary(self) -> String {
        match self.kind {
            SandboxOverrideKind::PatchWparseAdminApi => "admin_api.enabled=false".to_string(),
            SandboxOverrideKind::PatchWpsrcRuntime => format!(
                "connect={}, port={}",
                SANDBOX_RUNTIME_SOURCE_CONNECTOR, SANDBOX_RUNTIME_UDP_PORT
            ),
            SandboxOverrideKind::PatchWpgenRuntime => format!(
                "connect={}, port={}",
                SANDBOX_RUNTIME_OUTPUT_CONNECTOR, SANDBOX_RUNTIME_UDP_PORT
            ),
            SandboxOverrideKind::RewriteBusinessSink => "已固定复写为沙盒输出 sink".to_string(),
        }
    }

    fn apply(self, project_dir: &Path) -> Result<(), AppError> {
        match self.kind {
            SandboxOverrideKind::PatchWparseAdminApi => patch_override_file(
                project_dir,
                self.relative_path,
                patch_wparse_admin_api_runtime,
            ),
            SandboxOverrideKind::PatchWpsrcRuntime => {
                patch_override_file(project_dir, self.relative_path, patch_wpsrc_runtime)
            }
            SandboxOverrideKind::PatchWpgenRuntime => {
                patch_override_file(project_dir, self.relative_path, patch_wpgen_runtime)
            }
            SandboxOverrideKind::RewriteBusinessSink => {
                write_override_file(project_dir, self.relative_path, BUSINESS_SINK_OVERRIDE)
            }
        }
    }
}

/// 统一应用沙盒运行时文件覆盖，避免覆盖逻辑分散在多个函数中。
fn apply_sandbox_runtime_overrides(project_dir: &Path) -> Result<(), AppError> {
    for spec in SANDBOX_OVERRIDE_SPECS {
        spec.apply(project_dir)?;
    }
    Ok(())
}

/// 输出沙盒运行时覆盖摘要，供 prepare.log 与前端诊断展示复用。
pub(crate) fn sandbox_runtime_override_log_lines(workspace: &SandboxWorkspace) -> Vec<String> {
    SANDBOX_OVERRIDE_SPECS
        .iter()
        .map(|spec| {
            format!(
                "{} -> {}",
                workspace.display_relative(&workspace.project_dir.join(spec.relative_path)),
                spec.summary()
            )
        })
        .collect()
}

/// 读取并补丁 project_dir 内现有文件，保留未修改部分内容。
fn patch_override_file(
    project_dir: &Path,
    relative: &str,
    patcher: fn(&str) -> Result<String, AppError>,
) -> Result<(), AppError> {
    validate_override_path(relative)?;
    let target = project_dir.join(relative);
    let content = fs::read_to_string(&target).map_err(AppError::internal)?;
    let patched = patcher(&content)?;
    fs::write(&target, patched).map_err(AppError::internal)?;
    Ok(())
}

/// 将 wparse.toml 的 admin_api.enabled 固定关闭。
fn patch_wparse_admin_api_runtime(content: &str) -> Result<String, AppError> {
    Ok(patch_admin_api_enabled_false(content))
}

/// 将 wpsrc.toml 中 gen_udp source 的 connector 与端口切到沙盒运行时值。
fn patch_wpsrc_runtime(content: &str) -> Result<String, AppError> {
    patch_wpsrc_source_runtime(
        content,
        SANDBOX_RUNTIME_SOURCE_CONNECTOR,
        SANDBOX_RUNTIME_UDP_PORT,
    )
}

/// 将 wpgen.toml 的输出 connector 与端口切到沙盒运行时值。
fn patch_wpgen_runtime(content: &str) -> Result<String, AppError> {
    patch_wpgen_output_runtime(
        content,
        SANDBOX_RUNTIME_OUTPUT_CONNECTOR,
        SANDBOX_RUNTIME_UDP_PORT,
    )
}

/// 将 wparse.toml 中 [admin_api] 节的 enabled 设为 false。
/// 若该节不存在则追加 [admin_api] + enabled = false。
fn patch_admin_api_enabled_false(content: &str) -> String {
    let mut lines = Vec::new();
    let mut in_admin_api = false;
    let mut found_admin_api = false;
    let mut patched_enabled = false;
    let mut pending_enabled_insert = false;

    for line in content.lines() {
        let trimmed = line.trim();
        let is_section = trimmed.starts_with('[') && trimmed.ends_with(']');

        if in_admin_api
            && pending_enabled_insert
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("enabled")
        {
            lines.push("enabled = false".to_string());
            pending_enabled_insert = false;
            patched_enabled = true;
        }

        if in_admin_api && is_section && trimmed != "[admin_api]" {
            in_admin_api = false;
        }

        if trimmed == "[admin_api]" {
            in_admin_api = true;
            found_admin_api = true;
            patched_enabled = false;
            pending_enabled_insert = true;
            lines.push(line.to_string());
            continue;
        }

        if in_admin_api && trimmed.starts_with("enabled") {
            let indent = line
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            lines.push(format!("{indent}enabled = false"));
            patched_enabled = true;
            pending_enabled_insert = false;
            continue;
        }

        lines.push(line.to_string());
    }

    if found_admin_api {
        if in_admin_api && !patched_enabled {
            lines.push("enabled = false".to_string());
        }
    } else {
        if !lines.last().is_none_or(|line| line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push("[admin_api]".to_string());
        lines.push("enabled = false".to_string());
    }

    let mut output = lines.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }
    output
}

/// 在保留原格式与注释的前提下，仅更新 wpsrc.toml 中目标 source 的 connector 和端口。
fn patch_wpsrc_source_runtime(content: &str, connect: &str, port: u16) -> Result<String, AppError> {
    let mut lines = Vec::new();
    let mut block_lines: Vec<String> = Vec::new();
    let mut block_connect_value: Option<String> = None;
    let mut block_enable_index: Option<usize> = None;
    let mut block_connect_index: Option<usize> = None;
    let mut block_port_index: Option<usize> = None;
    let mut in_sources_block = false;
    let mut in_source_params = false;
    let mut found_target = false;
    let mut patched_connect = false;
    let mut patched_port = false;

    let flush_source_block = |lines: &mut Vec<String>,
                              block_lines: &mut Vec<String>,
                              block_connect_value: &mut Option<String>,
                              block_enable_index: &mut Option<usize>,
                              block_connect_index: &mut Option<usize>,
                              block_port_index: &mut Option<usize>,
                              found_target: &mut bool,
                              patched_connect: &mut bool,
                              patched_port: &mut bool| {
        if block_connect_value.as_deref() == Some(connect) {
            *found_target = true;
            if let Some(index) = *block_enable_index {
                if let Some(line) = block_lines.get(index) {
                    let indent = line
                        .chars()
                        .take_while(|ch| ch.is_whitespace())
                        .collect::<String>();
                    block_lines[index] = format!("{indent}enable = true");
                }
            } else {
                let insert_at = block_connect_index.unwrap_or(block_lines.len());
                block_lines.insert(insert_at, "enable = true".to_string());
            }
            if let Some(index) = *block_connect_index
                && let Some(line) = block_lines.get(index)
            {
                let indent = line
                    .chars()
                    .take_while(|ch| ch.is_whitespace())
                    .collect::<String>();
                block_lines[index] = format!("{indent}connect = \"{connect}\"");
                *patched_connect = true;
            }
            if let Some(index) = *block_port_index {
                if let Some(line) = block_lines.get(index) {
                    let indent = line
                        .chars()
                        .take_while(|ch| ch.is_whitespace())
                        .collect::<String>();
                    block_lines[index] = format!("{indent}port = {port}");
                    *patched_port = true;
                }
            } else {
                block_lines.push(format!("params = {{ addr = \"0.0.0.0\", port = {port} }}"));
                *patched_port = true;
            }
        }

        lines.append(block_lines);
        *block_connect_value = None;
        *block_enable_index = None;
        *block_connect_index = None;
        *block_port_index = None;
    };

    for line in content.lines() {
        let trimmed = line.trim();
        let is_array_section = trimmed.starts_with("[[") && trimmed.ends_with("]]");
        let is_section = !is_array_section && trimmed.starts_with('[') && trimmed.ends_with(']');

        if is_array_section {
            if in_sources_block {
                flush_source_block(
                    &mut lines,
                    &mut block_lines,
                    &mut block_connect_value,
                    &mut block_enable_index,
                    &mut block_connect_index,
                    &mut block_port_index,
                    &mut found_target,
                    &mut patched_connect,
                    &mut patched_port,
                );
            }

            in_sources_block = trimmed == "[[sources]]";
            in_source_params = false;
            if in_sources_block {
                block_lines.push(line.to_string());
            } else {
                lines.push(line.to_string());
            }
            continue;
        }

        if in_sources_block {
            if is_section {
                in_source_params = trimmed == "[sources.params]";
            }

            if block_connect_value.is_none()
                && let Some(value) = parse_toml_string_assignment(trimmed, "connect")
            {
                block_connect_value = Some(value);
            }

            if !in_source_params && trimmed.starts_with("enable") {
                block_enable_index = Some(block_lines.len());
            }

            if !in_source_params && trimmed.starts_with("connect") {
                block_connect_index = Some(block_lines.len());
            }

            if in_source_params && trimmed.starts_with("port") {
                block_port_index = Some(block_lines.len());
            }

            block_lines.push(line.to_string());
            continue;
        }

        lines.push(line.to_string());
    }

    if in_sources_block {
        flush_source_block(
            &mut lines,
            &mut block_lines,
            &mut block_connect_value,
            &mut block_enable_index,
            &mut block_connect_index,
            &mut block_port_index,
            &mut found_target,
            &mut patched_connect,
            &mut patched_port,
        );
    }

    if !found_target {
        if !lines.last().is_none_or(|line| line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push("[[sources]]".to_string());
        lines.push(format!("key = \"{}\"", SANDBOX_RUNTIME_SOURCE_KEY));
        lines.push("enable = true".to_string());
        lines.push(format!("connect = \"{connect}\""));
        lines.push(format!("params = {{ addr = \"0.0.0.0\", port = {port} }}"));
        patched_connect = true;
        patched_port = true;
    }

    if !patched_connect {
        return Err(AppError::validation(
            "wpsrc.toml 未能应用沙盒输入 connector 覆盖".to_string(),
        ));
    }

    if !patched_port {
        return Err(AppError::validation(
            "wpsrc.toml 未能应用沙盒输入端口覆盖".to_string(),
        ));
    }

    let mut output = lines.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

/// 在保留原格式与注释的前提下，仅更新 wpgen.toml 的输出 connector 和端口。
fn patch_wpgen_output_runtime(content: &str, connect: &str, port: u16) -> Result<String, AppError> {
    let mut lines = Vec::new();
    let mut in_output = false;
    let mut in_output_params = false;
    let mut found_output = false;
    let mut found_output_params = false;
    let mut patched_connect = false;
    let mut patched_port = false;

    for line in content.lines() {
        let trimmed = line.trim();
        let is_section = trimmed.starts_with('[') && trimmed.ends_with(']');

        if is_section {
            in_output = trimmed == "[output]";
            in_output_params = trimmed == "[output.params]";
            found_output |= in_output;
            found_output_params |= in_output_params;
        }

        if in_output && trimmed.starts_with("connect") {
            let indent = line
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            lines.push(format!("{indent}connect = \"{connect}\""));
            patched_connect = true;
            continue;
        }

        if in_output_params && trimmed.starts_with("port") {
            let indent = line
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            lines.push(format!("{indent}port = {port}"));
            patched_port = true;
            continue;
        }

        lines.push(line.to_string());
    }

    if !found_output || !patched_connect {
        return Err(AppError::validation(
            "wpgen.toml 缺少 [output] 或 connect 配置，无法应用沙盒输出覆盖".to_string(),
        ));
    }

    if !found_output_params || !patched_port {
        return Err(AppError::validation(
            "wpgen.toml 缺少 [output.params] 或 port 配置，无法应用沙盒输出覆盖".to_string(),
        ));
    }

    let mut output = lines.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

/// 解析形如 `key = "value"` 的 TOML 字符串赋值。
fn parse_toml_string_assignment(line: &str, key: &str) -> Option<String> {
    let value = line.strip_prefix(key)?.trim_start();
    let value = value.strip_prefix('=')?.trim_start();
    let value = value.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

/// 校验用户提交的文件覆盖路径为 project_root 内的安全相对路径，
/// 防止路径穿越攻击。
fn validate_override_path(path_str: &str) -> Result<(), AppError> {
    let path = Path::new(path_str);
    if path.is_absolute() {
        return Err(AppError::validation(format!(
            "override 文件路径必须是 project_root 内的相对路径: {}",
            path_str
        )));
    }

    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(AppError::validation(format!(
            "override 文件路径不允许包含 .. 或根路径: {}",
            path_str
        )));
    }

    Ok(())
}

/// 将路径转换为相对于工作区根目录的路径，超出根目录时返回 None。
fn relative_to_workspace_root(path: &Path) -> Option<PathBuf> {
    path.strip_prefix(Setting::workspace_root())
        .map(|p| p.to_path_buf())
        .ok()
}

/// 渲染目录树结构文本，深度和条目数有上限防止输出过大。
fn render_tree(
    root: &Path,
    root_label: &str,
    max_depth: usize,
    max_entries: usize,
) -> Result<String, AppError> {
    let mut lines = Vec::new();
    lines.push(root_label.to_string());
    let mut counter = 0;
    build_tree_lines(
        root,
        "",
        0,
        max_depth,
        max_entries,
        &mut counter,
        &mut lines,
    )?;
    Ok(lines.join("\n"))
}

/// 递归构建树状目录的每一行输出。
fn build_tree_lines(
    path: &Path,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    counter: &mut usize,
    lines: &mut Vec<String>,
) -> Result<(), AppError> {
    if depth >= max_depth {
        return Ok(());
    }
    let mut entries = fs::read_dir(path)
        .map_err(AppError::internal)?
        .filter_map(|entry| entry.ok())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    for (idx, entry) in entries.iter().enumerate() {
        if *counter >= max_entries {
            lines.push(format!("{}└── ...", prefix));
            break;
        }
        let is_last = idx == entries.len() - 1;
        let branch = if is_last { "└──" } else { "├──" };
        let name = entry.file_name().to_string_lossy().to_string();
        lines.push(format!("{}{} {}", prefix, branch, name));
        *counter += 1;
        if entry.file_type().map_err(AppError::internal)?.is_dir() {
            let next_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            build_tree_lines(
                &entry.path(),
                &next_prefix,
                depth + 1,
                max_depth,
                max_entries,
                counter,
                lines,
            )?;
        }
    }
    Ok(())
}

// ============ 进程管理 ============

/// 对 wparse daemon 进程的轻量封装，方便查询与终止。
pub struct DaemonProcess {
    child: Child,
    log_path: PathBuf,
}

/// wpgen 命令执行的输出摘要。
pub struct WpgenOutput {
    /// 进程退出码，正常退出为 Some(0)。
    pub exit_code: Option<i32>,
    /// 输出日志文件路径。
    pub log_path: PathBuf,
}

impl DaemonProcess {
    /// 创建新的进程句柄。
    pub fn new(child: Child, log_path: PathBuf) -> Self {
        Self { child, log_path }
    }

    /// 等待日志中出现指定标记，常用于探测 daemon 是否就绪。
    /// 超时或进程提前退出时返回错误。
    pub async fn wait_for_marker(
        &mut self,
        marker: &str,
        timeout: Duration,
    ) -> Result<(), AppError> {
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().map_err(AppError::internal)? {
                return Err(AppError::internal(format!(
                    "wparse daemon 已退出，状态: {}",
                    status
                )));
            }

            if read_file_contains(&self.log_path, marker).await? {
                return Ok(());
            }

            if start.elapsed() > timeout {
                return Err(AppError::internal("等待 wparse daemon 就绪超时"));
            }

            sleep(Duration::from_millis(500)).await;
        }
    }

    /// 终止进程，先尝试 SIGTERM（Unix）再 wait 回收。
    /// 终止失败不影响流程。
    pub async fn terminate(mut self) -> Result<(), AppError> {
        if let Some(id) = self.child.id() {
            #[cfg(unix)]
            {
                use nix::sys::signal::{Signal, kill};
                use nix::unistd::Pid;

                let _ = kill(Pid::from_raw(id as i32), Signal::SIGTERM);
            }
            #[cfg(windows)]
            {
                let _ = self.child.start_kill();
            }
        }

        let _ = self.child.wait().await;
        Ok(())
    }
}

/// 启动 wparse daemon 并将 stdout/stderr 重定向到日志文件。
pub async fn spawn_daemon(project_dir: &Path, log_path: &Path) -> Result<DaemonProcess, AppError> {
    let binary = resolve_toolchain_command("wparse");
    let mut cmd = Command::new(&binary);
    let log_file = File::create(log_path).map_err(AppError::internal)?;
    let stdout = log_file.try_clone().map_err(AppError::internal)?;
    let stderr = log_file.try_clone().map_err(AppError::internal)?;
    cmd.arg("daemon")
        .current_dir(project_dir)
        .stdout(stdout)
        .stderr(stderr);

    let child = cmd.spawn().map_err(|err| {
        AppError::internal(format!(
            "启动 wparse daemon 失败: {}。请检查可执行文件 {} 是否可用",
            err,
            binary.display()
        ))
    })?;

    Ok(DaemonProcess::new(child, log_path.to_path_buf()))
}

/// 执行 `wpgen sample`，并将输出写入日志文件。
/// 支持超时控制，超时后返回错误。
pub async fn run_wpgen(
    project_dir: &Path,
    log_path: &Path,
    sample_count: u32,
    timeout: Duration,
) -> Result<WpgenOutput, AppError> {
    let binary = resolve_toolchain_command("wpgen");
    let mut cmd = Command::new(&binary);
    let log_file = File::create(log_path).map_err(AppError::internal)?;
    let stdout = log_file.try_clone().map_err(AppError::internal)?;
    let stderr = log_file.try_clone().map_err(AppError::internal)?;
    cmd.args([
        "sample",
        "-w",
        ".",
        "-n",
        &sample_count.to_string(),
        "--print_stat",
    ])
    .current_dir(project_dir)
    .stdout(stdout)
    .stderr(stderr);

    let mut child = cmd.spawn().map_err(|err| {
        AppError::internal(format!(
            "执行 wpgen sample 失败: {}。请确认可执行文件 {} 是否可用",
            err,
            binary.display()
        ))
    })?;

    let status = tokio::time::timeout(timeout, child.wait())
        .await
        .map_err(|_| AppError::internal("wpgen 运行超时"))?
        .map_err(AppError::internal)?;

    Ok(WpgenOutput {
        exit_code: status.code(),
        log_path: log_path.to_path_buf(),
    })
}

/// 运行 `<cmd> --version`，返回 stdout/stderr 中非空内容，优先返回 stdout。
pub async fn command_version_output(cmd: &str) -> Result<String, AppError> {
    let binary = resolve_toolchain_command(cmd);
    let output = Command::new(&binary)
        .arg("--version")
        .output()
        .await
        .map_err(|err| {
            AppError::internal(format!("{} --version 执行失败: {}", binary.display(), err))
        })?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !stdout.is_empty() {
            Ok(stdout)
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(AppError::internal(format!(
            "{} --version 返回非 0 状态: {}",
            binary.display(),
            if stderr.is_empty() {
                "未提供错误输出".to_string()
            } else {
                stderr
            }
        )))
    }
}

/// 检查指定 UDP 端口当前是否可绑定，用于预先识别残留进程占用。
pub fn ensure_udp_port_available(port: u16) -> Result<(), AppError> {
    let bind_addr = format!("0.0.0.0:{port}");
    UdpSocket::bind(&bind_addr)
        .map_err(|err| AppError::validation(format!("UDP 端口 {} 不可用: {}", port, err)))?;
    Ok(())
}

/// 在指定目录执行 `wproj check` 并返回输出。
pub async fn run_wproj_check(project_dir: &Path) -> Result<String, AppError> {
    let binary = resolve_toolchain_command("wproj");
    let output = Command::new(&binary)
        .arg("check")
        .current_dir(project_dir)
        .output()
        .await
        .map_err(|err| {
            AppError::internal(format!(
                "执行 wproj check 失败 ({}): {}",
                binary.display(),
                err
            ))
        })?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(AppError::internal(format!(
            "wproj check 返回非 0 状态 ({}): {}",
            binary.display(),
            if stderr.is_empty() {
                "未提供错误输出"
            } else {
                &stderr
            }
        )))
    }
}

/// 读取文件内容并检查是否包含指定字符串，文件不存在时返回 false。
async fn read_file_contains(path: &Path, needle: &str) -> Result<bool, AppError> {
    if !path.exists() {
        return Ok(false);
    }

    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(AppError::internal)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .await
        .map_err(AppError::internal)?;
    Ok(buf.contains(needle))
}
