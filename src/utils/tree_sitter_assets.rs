//! Tree-sitter 运行时资源辅助。
//!
//! - `/tree-sitter/*` 资源优先从 `web/public` 直接读取；
//! - 开发态启动时会尝试从远程仓库同步最新语言资产到 `web/public/tree-sitter`；
//! - 同步失败时回退到现有本地资产，不阻断服务启动。

use super::tree_sitter_sync_manifest::{TREE_SITTER_ASSET_SOURCES, TreeSitterAssetSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use wasmparser::Validator;

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Debug, Deserialize, Serialize, Clone)]
struct EditorAssetManifest {
    language_id: String,
    parser_wasm: String,
    highlights_query: String,
    completion_bundle: Option<String>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(MANIFEST_DIR)
}

fn tree_sitter_public_root() -> PathBuf {
    repo_root().join("web/public/tree-sitter")
}

fn tree_sitter_languages_root() -> PathBuf {
    tree_sitter_public_root().join("languages")
}

fn tree_sitter_remote_cache_root() -> PathBuf {
    if let Ok(path) = std::env::var("WP_STATION_TREE_SITTER_SYNC_ROOT") {
        return PathBuf::from(path);
    }

    repo_root().join("target/tree-sitter-upstream")
}

fn is_tree_sitter_remote_sync_enabled() -> bool {
    std::env::var("WP_STATION_TREE_SITTER_REMOTE_SYNC")
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            normalized != "0" && normalized != "false" && normalized != "off"
        })
        .unwrap_or(true)
}

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn copy_asset_to(
    src_root: &Path,
    relative_path: &str,
    dest_root: &Path,
    dest_relative_path: &str,
) -> std::io::Result<()> {
    let src_path = src_root.join(relative_path);
    if !src_path.exists() {
        return Ok(());
    }

    let dest_path = dest_root.join(dest_relative_path);
    ensure_parent_dir(&dest_path)?;
    fs::copy(src_path, dest_path)?;
    Ok(())
}

fn copy_asset(src_root: &Path, relative_path: &str, dest_root: &Path) -> std::io::Result<()> {
    copy_asset_to(src_root, relative_path, dest_root, relative_path)
}

fn read_editor_asset_manifest(
    crate_root: &Path,
    source: &TreeSitterAssetSource,
) -> Result<EditorAssetManifest, String> {
    let manifest_path = crate_root.join(source.manifest_relative);
    let content = fs::read_to_string(&manifest_path).map_err(|err| {
        format!(
            "读取语言清单失败: path={}, error={}",
            manifest_path.display(),
            err
        )
    })?;
    serde_json::from_str::<EditorAssetManifest>(&content).map_err(|err| {
        format!(
            "解析语言清单失败: path={}, error={}",
            manifest_path.display(),
            err
        )
    })
}

fn validate_parser_wasm(crate_root: &Path, manifest: &EditorAssetManifest) -> Result<(), String> {
    let wasm_path = crate_root.join(&manifest.parser_wasm);
    let bytes = fs::read(&wasm_path).map_err(|err| {
        format!(
            "读取 parser wasm 失败: path={}, error={}",
            wasm_path.display(),
            err
        )
    })?;
    Validator::new()
        .validate_all(&bytes)
        .map(|_| ())
        .map_err(|err| {
            format!(
                "校验 parser wasm 失败: path={}, error={}",
                wasm_path.display(),
                err
            )
        })
}

fn is_asset_root_usable(
    crate_root: &Path,
    source: &TreeSitterAssetSource,
) -> Result<EditorAssetManifest, String> {
    let manifest = read_editor_asset_manifest(crate_root, source)?;
    validate_parser_wasm(crate_root, &manifest)?;
    Ok(manifest)
}

fn resolve_local_override_root(source: &TreeSitterAssetSource) -> Option<PathBuf> {
    source
        .local_override_root
        .map(|relative| repo_root().join(relative))
        .filter(|path| path.exists())
}

fn select_asset_root(
    source: &TreeSitterAssetSource,
    remote_root: Option<&PathBuf>,
    local_root: Option<PathBuf>,
) -> Option<(PathBuf, EditorAssetManifest)> {
    if let Some(root) = remote_root {
        match is_asset_root_usable(root, source) {
            Ok(manifest) => return Some((root.clone(), manifest)),
            Err(err) => {
                tracing::warn!(
                    "远程 tree-sitter 资产不可用，回退其他来源: package={}, manifest={}, error={}",
                    source.package_name,
                    source.manifest_relative,
                    err
                );
            }
        }
    }

    if let Some(root) = local_root {
        match is_asset_root_usable(&root, source) {
            Ok(manifest) => return Some((root, manifest)),
            Err(err) => {
                tracing::warn!(
                    "本地覆盖 tree-sitter 资产不可用: package={}, manifest={}, error={}",
                    source.package_name,
                    source.manifest_relative,
                    err
                );
            }
        }
    }

    None
}

fn run_git_command(args: &[&str], cwd: Option<&Path>) -> Result<(), String> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    let output = command
        .output()
        .map_err(|err| format!("执行 git 命令失败 {:?}: {}", args, err))?;

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "git {:?} 失败: status={:?}, stdout={}, stderr={}",
        args,
        output.status.code(),
        stdout.trim(),
        stderr.trim()
    ))
}

fn sync_tree_sitter_repo(source: &TreeSitterAssetSource, cache_root: &Path) -> Option<PathBuf> {
    if !is_tree_sitter_remote_sync_enabled() {
        return None;
    }

    let repo_root = cache_root.join(source.package_name);
    if let Some(parent) = repo_root.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            tracing::warn!(
                "创建 tree-sitter 远程缓存目录失败: path={}, error={}",
                parent.display(),
                err
            );
            return None;
        }
    }

    let sync_result = if repo_root.join(".git").exists() {
        run_git_command(
            &["fetch", "--depth", "1", "origin", source.branch],
            Some(&repo_root),
        )
        .and_then(|_| run_git_command(&["checkout", "--force", source.branch], Some(&repo_root)))
        .and_then(|_| {
            let remote_ref = format!("origin/{}", source.branch);
            run_git_command(&["reset", "--hard", remote_ref.as_str()], Some(&repo_root))
        })
    } else if repo_root.exists() {
        Err(format!(
            "缓存目录已存在但不是 git 仓库: {}",
            repo_root.display()
        ))
    } else {
        let repo_root_string = repo_root.to_string_lossy().to_string();
        run_git_command(
            &[
                "clone",
                "--depth",
                "1",
                "--branch",
                source.branch,
                source.repo_url,
                repo_root_string.as_str(),
            ],
            None,
        )
    };

    match sync_result {
        Ok(()) => Some(repo_root),
        Err(err) => {
            tracing::warn!(
                "同步 tree-sitter 远程仓库失败，回退到本地资产: package={}, error={}",
                source.package_name,
                err
            );
            None
        }
    }
}

fn export_language_assets(
    crate_root: &Path,
    manifest: &EditorAssetManifest,
    source: &TreeSitterAssetSource,
) -> Result<EditorAssetManifest, String> {
    let language_root = tree_sitter_languages_root().join(&manifest.language_id);
    copy_asset_to(
        crate_root,
        source.manifest_relative,
        &language_root,
        "editor/asset-manifest.json",
    )
    .map_err(|err| {
        format!(
            "复制语言清单失败: language={}, error={}",
            manifest.language_id, err
        )
    })?;
    copy_asset(crate_root, &manifest.highlights_query, &language_root).map_err(|err| {
        format!(
            "复制高亮查询失败: language={}, error={}",
            manifest.language_id, err
        )
    })?;
    copy_asset(crate_root, &manifest.parser_wasm, &language_root).map_err(|err| {
        format!(
            "复制 parser wasm 失败: language={}, error={}",
            manifest.language_id, err
        )
    })?;

    if let Some(bundle) = manifest.completion_bundle.as_deref() {
        copy_asset(crate_root, bundle, &language_root).map_err(|err| {
            format!(
                "复制补全包失败: language={}, error={}",
                manifest.language_id, err
            )
        })?;
    }

    Ok(manifest.clone())
}

fn export_tree_sitter_runtime() -> Result<(), String> {
    let runtime_src = repo_root().join("web/node_modules/web-tree-sitter/web-tree-sitter.wasm");
    if !runtime_src.exists() {
        return Ok(());
    }

    let runtime_dest = tree_sitter_public_root().join("tree-sitter.wasm");
    ensure_parent_dir(&runtime_dest)
        .map_err(|err| format!("创建 runtime wasm 目录失败: {}", err))?;
    fs::copy(&runtime_src, &runtime_dest).map_err(|err| {
        format!(
            "复制 runtime wasm 失败: from={}, to={}, error={}",
            runtime_src.display(),
            runtime_dest.display(),
            err
        )
    })?;
    Ok(())
}

/// 开发态服务启动时同步 tree-sitter 语言资产。
///
/// 说明：
/// - `build.rs` 只能在 Cargo 判定需要重新构建时运行，无法保证每次 `cargo run`
///   都会执行；
/// - 因此这里在开发态启动阶段补一次同步，确保 `/tree-sitter/*` 总能尽量拿到远程最新资产。
pub fn sync_tree_sitter_assets_for_dev_start() -> Result<(), String> {
    if !cfg!(debug_assertions) || !is_tree_sitter_remote_sync_enabled() {
        return Ok(());
    }

    fs::create_dir_all(tree_sitter_languages_root())
        .map_err(|err| format!("创建 tree-sitter 语言目录失败: {}", err))?;

    let remote_cache_root = tree_sitter_remote_cache_root();
    let mut synced_roots: HashMap<&'static str, Option<PathBuf>> = HashMap::new();
    let mut exported_manifests = Vec::new();

    for source in TREE_SITTER_ASSET_SOURCES {
        let remote_root = if let Some(cached) = synced_roots.get(source.package_name) {
            cached.clone()
        } else {
            let synced = sync_tree_sitter_repo(source, &remote_cache_root);
            synced_roots.insert(source.package_name, synced.clone());
            synced
        };
        let local_root = resolve_local_override_root(source);

        let Some((crate_root, manifest)) =
            select_asset_root(source, remote_root.as_ref(), local_root)
        else {
            tracing::warn!(
                "未找到可用的 tree-sitter 资产目录，跳过导出: package={}",
                source.package_name
            );
            continue;
        };

        match export_language_assets(&crate_root, &manifest, source) {
            Ok(manifest) => exported_manifests.push(manifest),
            Err(err) => {
                tracing::warn!(
                    "导出 tree-sitter 语言资产失败: package={}, error={}",
                    source.package_name,
                    err
                );
            }
        }
    }

    if let Err(err) = export_tree_sitter_runtime() {
        tracing::warn!("导出 tree-sitter runtime wasm 失败: error={}", err);
    }

    let index_path = tree_sitter_languages_root().join("index.json");
    ensure_parent_dir(&index_path).map_err(|err| format!("创建语言索引目录失败: {}", err))?;
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&exported_manifests)
            .map_err(|err| format!("序列化语言索引失败: {}", err))?,
    )
    .map_err(|err| format!("写入语言索引失败: {}", err))?;

    Ok(())
}

/// 尝试从本地磁盘读取 `/tree-sitter/*` 资源。
pub fn read_runtime_asset_from_public(request_path: &str) -> std::io::Result<Option<Vec<u8>>> {
    let relative_path = request_path.trim_start_matches('/');
    if !relative_path.starts_with("tree-sitter/") {
        return Ok(None);
    }

    let asset_path = repo_root().join("web/public").join(relative_path);
    if !asset_path.exists() {
        return Ok(None);
    }

    Ok(Some(fs::read(asset_path)?))
}
