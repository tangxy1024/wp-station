use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use wasmparser::Validator;

#[path = "src/utils/tree_sitter_sync_manifest.rs"]
mod tree_sitter_sync_manifest;

use tree_sitter_sync_manifest::{TREE_SITTER_ASSET_SOURCES, TreeSitterAssetSource};

#[derive(Debug, Deserialize, Serialize, Clone)]
struct EditorAssetManifest {
    language_id: String,
    parser_wasm: String,
    highlights_query: String,
    completion_bundle: Option<String>,
}

fn get_cargo_metadata() -> Value {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()
        .expect("Failed to run cargo metadata");
    serde_json::from_slice(&output.stdout).expect("Failed to parse cargo metadata JSON")
}

fn get_package_version<'a>(packages: &'a [Value], name: &str) -> &'a str {
    packages
        .iter()
        .find(|pkg| pkg.get("name").and_then(|v| v.as_str()) == Some(name))
        .and_then(|pkg| pkg.get("version").and_then(|v| v.as_str()))
        .unwrap_or("unknown")
}

fn get_package_root(packages: &[Value], name: &str) -> Option<PathBuf> {
    packages
        .iter()
        .find(|pkg| pkg.get("name").and_then(|v| v.as_str()) == Some(name))
        .and_then(|pkg| pkg.get("manifest_path").and_then(|v| v.as_str()))
        .and_then(|manifest_path| Path::new(manifest_path).parent().map(Path::to_path_buf))
}

fn is_tree_sitter_remote_sync_enabled() -> bool {
    std::env::var("WP_STATION_TREE_SITTER_REMOTE_SYNC")
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            normalized != "0" && normalized != "false" && normalized != "off"
        })
        .unwrap_or(true)
}

fn tree_sitter_remote_cache_root() -> PathBuf {
    if let Ok(path) = std::env::var("WP_STATION_TREE_SITTER_SYNC_ROOT") {
        return PathBuf::from(path);
    }

    PathBuf::from("target").join("tree-sitter-upstream")
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
            println!(
                "cargo:warning=创建 tree-sitter 远程缓存目录失败，跳过远程同步: path={}, error={}",
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
            println!(
                "cargo:warning=同步 tree-sitter 远程仓库失败，回退到本地资产: package={}, error={}",
                source.package_name, err
            );
            None
        }
    }
}

fn print_command_output(label: &str, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        println!("cargo:warning=[{} stdout] {}", label, line);
    }

    for line in stderr.lines().filter(|line| !line.trim().is_empty()) {
        println!("cargo:warning=[{} stderr] {}", label, line);
    }
}

fn ensure_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create asset parent directory");
    }
}

fn copy_asset(src_root: &Path, relative_path: &str, dest_root: &Path) {
    copy_asset_to(src_root, relative_path, dest_root, relative_path);
}

fn copy_asset_to(src_root: &Path, relative_path: &str, dest_root: &Path, dest_relative_path: &str) {
    let src_path = src_root.join(relative_path);
    if !src_path.exists() {
        println!(
            "cargo:warning=语言资产不存在，跳过复制: {}",
            src_path.display()
        );
        return;
    }

    println!("cargo:rerun-if-changed={}", src_path.display());

    let dest_path = dest_root.join(dest_relative_path);
    ensure_parent_dir(&dest_path);
    fs::copy(&src_path, &dest_path).unwrap_or_else(|err| {
        panic!(
            "Failed to copy asset from {} to {}: {}",
            src_path.display(),
            dest_path.display(),
            err
        )
    });
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

fn select_asset_root<'a>(
    source: &TreeSitterAssetSource,
    remote_root: Option<&'a PathBuf>,
    local_root: Option<PathBuf>,
    package_root: Option<PathBuf>,
) -> Option<(PathBuf, EditorAssetManifest)> {
    if let Some(root) = remote_root {
        match is_asset_root_usable(root, source) {
            Ok(manifest) => return Some((root.clone(), manifest)),
            Err(err) => {
                println!(
                    "cargo:warning=远程 tree-sitter 资产不可用，回退其他来源: package={}, manifest={}, error={}",
                    source.package_name, source.manifest_relative, err
                );
            }
        }
    }

    if let Some(root) = local_root {
        match is_asset_root_usable(&root, source) {
            Ok(manifest) => return Some((root, manifest)),
            Err(err) => {
                println!(
                    "cargo:warning=本地覆盖 tree-sitter 资产不可用: package={}, manifest={}, error={}",
                    source.package_name, source.manifest_relative, err
                );
            }
        }
    }

    if let Some(root) = package_root {
        match is_asset_root_usable(&root, source) {
            Ok(manifest) => return Some((root, manifest)),
            Err(err) => {
                println!(
                    "cargo:warning=Cargo 依赖 tree-sitter 资产不可用: package={}, manifest={}, error={}",
                    source.package_name, source.manifest_relative, err
                );
            }
        }
    }

    None
}

fn register_tree_sitter_inputs(crate_root: &Path) {
    for relative in ["editor", "queries", "completions"] {
        let path = crate_root.join(relative);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn export_tree_sitter_assets(metadata: &Value) {
    let packages = metadata
        .get("packages")
        .and_then(|v| v.as_array())
        .expect("No packages found in cargo metadata");

    let public_root = Path::new("web/public/tree-sitter");
    let languages_root = public_root.join("languages");
    fs::create_dir_all(&languages_root).expect("Failed to create tree-sitter public directory");

    let mut exported_manifests = Vec::new();
    let remote_cache_root = tree_sitter_remote_cache_root();
    let mut synced_roots: HashMap<&'static str, Option<PathBuf>> = HashMap::new();

    for source in TREE_SITTER_ASSET_SOURCES {
        let remote_root = if let Some(cached) = synced_roots.get(source.package_name) {
            cached.clone()
        } else {
            let synced = sync_tree_sitter_repo(source, &remote_cache_root);
            synced_roots.insert(source.package_name, synced.clone());
            synced
        };
        let local_root = source
            .local_override_root
            .map(PathBuf::from)
            .filter(|path| path.exists());
        let package_root = get_package_root(packages, source.package_name);

        let Some((crate_root, manifest)) =
            select_asset_root(source, remote_root.as_ref(), local_root, package_root)
        else {
            println!(
                "cargo:warning=未找到 tree-sitter 依赖目录，跳过语言资产导出: {}",
                source.package_name
            );
            continue;
        };

        register_tree_sitter_inputs(&crate_root);
        println!(
            "cargo:rerun-if-changed={}",
            crate_root.join(source.manifest_relative).display()
        );

        let language_root = languages_root.join(&manifest.language_id);
        copy_asset_to(
            &crate_root,
            source.manifest_relative,
            &language_root,
            "editor/asset-manifest.json",
        );
        copy_asset(&crate_root, &manifest.highlights_query, &language_root);
        copy_asset(&crate_root, &manifest.parser_wasm, &language_root);

        if let Some(bundle) = manifest.completion_bundle.as_deref() {
            copy_asset(&crate_root, bundle, &language_root);
        }

        exported_manifests.push(manifest);
    }

    let index_path = languages_root.join("index.json");
    ensure_parent_dir(&index_path);
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&exported_manifests)
            .expect("Failed to serialize tree-sitter manifest index"),
    )
    .expect("Failed to write tree-sitter manifest index");
}

fn export_web_tree_sitter_runtime() {
    let runtime_root = Path::new("web/public/tree-sitter");
    fs::create_dir_all(runtime_root).expect("Failed to create tree-sitter runtime directory");

    let runtime_src = Path::new("web/node_modules/web-tree-sitter/web-tree-sitter.wasm");
    if !runtime_src.exists() {
        println!(
            "cargo:warning=未找到 web-tree-sitter 运行时 wasm，跳过复制: {}",
            runtime_src.display()
        );
        return;
    }

    println!("cargo:rerun-if-changed={}", runtime_src.display());

    let runtime_dest = runtime_root.join("tree-sitter.wasm");
    fs::copy(runtime_src, &runtime_dest).unwrap_or_else(|err| {
        panic!(
            "Failed to copy runtime wasm from {} to {}: {}",
            runtime_src.display(),
            runtime_dest.display(),
            err
        )
    });
}

fn ensure_frontend_tree_sitter_assets(metadata: &Value) {
    println!("cargo:rerun-if-changed=web/package.json");
    println!("cargo:rerun-if-changed=web/package-lock.json");
    println!("cargo:rerun-if-changed=web/src");
    println!("cargo:rerun-if-changed=web/index.html");
    println!("cargo:rerun-if-changed=web/vite.config.js");

    let npm_check = Command::new("npm").arg("--version").output();

    if npm_check.is_err() {
        println!("cargo:warning=未检测到 npm，跳过前端构建");
        return;
    }

    let runtime_src = Path::new("web/node_modules/web-tree-sitter/web-tree-sitter.wasm");
    if !runtime_src.exists() {
        let install_result = Command::new("npm")
            .arg("install")
            .current_dir("web")
            .output();

        match install_result {
            Ok(output) => {
                if !output.status.success() {
                    println!("cargo:warning=npm install 失败");
                    print_command_output("npm install", &output);
                    println!(
                        "cargo:warning=npm install 失败，退出码: {:?}",
                        output.status.code()
                    );
                    return;
                }
            }
            Err(e) => {
                println!("cargo:warning=npm install 失败: {}", e);
                return;
            }
        }
    }

    export_tree_sitter_assets(metadata);
    export_web_tree_sitter_runtime();
}

fn run_npm_build() {
    let build_result = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir("web")
        .output();

    match build_result {
        Ok(output) => {
            if !output.status.success() {
                println!("cargo:warning=前端构建失败");
                print_command_output("npm run build", &output);
                println!(
                    "cargo:warning=前端构建失败，退出码: {:?}",
                    output.status.code()
                );
            }
        }
        Err(e) => {
            println!("cargo:warning=npm run build 失败: {}", e);
        }
    }
}

fn main() {
    let is_release = std::env::var("PROFILE").unwrap_or_default() == "release";

    let metadata = get_cargo_metadata();
    if is_release {
        println!("cargo:warning=Release 构建，跳过前端资源同步与 npm 构建");
    } else {
        ensure_frontend_tree_sitter_assets(&metadata);
        run_npm_build();
    }

    let app_name = env!("CARGO_PKG_NAME");
    let wp_parse_pkg_name = "wp-engine";

    let packages = metadata
        .get("packages")
        .and_then(|v| v.as_array())
        .expect("No packages found in cargo metadata");

    let wp_station = get_package_version(packages, app_name);
    let wp_parse = get_package_version(packages, wp_parse_pkg_name);

    println!("cargo:rustc-env=WP_STATION_VERSION={}", wp_station);
    println!("cargo:rustc-env=WP_PARSE_VERSION={}", wp_parse);
}
