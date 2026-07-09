use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct TreeSitterAssetSource {
    package_name: &'static str,
    manifest_relative: &'static str,
    local_override_root: Option<&'static str>,
}

const TREE_SITTER_ASSET_SOURCES: &[TreeSitterAssetSource] = &[
    TreeSitterAssetSource {
        package_name: "tree-sitter-wpl",
        manifest_relative: "editor/asset-manifest.json",
        local_override_root: Some("../wp-tree-sitter/tree-sitter-wpl"),
    },
    TreeSitterAssetSource {
        package_name: "tree-sitter-oml",
        manifest_relative: "editor/asset-manifest.json",
        local_override_root: Some("../wp-tree-sitter/tree-sitter-oml"),
    },
    TreeSitterAssetSource {
        package_name: "tree-sitter-wfl",
        manifest_relative: "editor/wfs/asset-manifest.json",
        local_override_root: Some("../wp-tree-sitter/tree-sitter-wfl"),
    },
    TreeSitterAssetSource {
        package_name: "tree-sitter-wfl",
        manifest_relative: "editor/wfl/asset-manifest.json",
        local_override_root: Some("../wp-tree-sitter/tree-sitter-wfl"),
    },
    TreeSitterAssetSource {
        package_name: "tree-sitter-wfl",
        manifest_relative: "editor/wfg/asset-manifest.json",
        local_override_root: Some("../wp-tree-sitter/tree-sitter-wfl"),
    },
];

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

fn resolve_asset_root(
    packages: &[Value],
    source: &TreeSitterAssetSource,
) -> Option<PathBuf> {
    if let Some(local_root) = source.local_override_root {
        let path = PathBuf::from(local_root);
        if path.exists() {
            return Some(path);
        }
    }

    get_package_root(packages, source.package_name)
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

    for source in TREE_SITTER_ASSET_SOURCES {
        let Some(crate_root) = resolve_asset_root(packages, source) else {
            println!(
                "cargo:warning=未找到 tree-sitter 依赖目录，跳过语言资产导出: {}",
                source.package_name
            );
            continue;
        };

        let manifest_path = crate_root.join(source.manifest_relative);
        if !manifest_path.exists() {
            println!(
                "cargo:warning=未找到语言 asset-manifest，跳过: {}",
                manifest_path.display()
            );
            continue;
        }

        register_tree_sitter_inputs(&crate_root);
        println!("cargo:rerun-if-changed={}", manifest_path.display());

        let manifest = fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|content| serde_json::from_str::<EditorAssetManifest>(&content).ok());

        let Some(manifest) = manifest else {
            println!(
                "cargo:warning=读取语言 asset-manifest 失败，跳过: {}",
                manifest_path.display()
            );
            continue;
        };

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
    // 检查 npm 是否可用
    let npm_check = Command::new("npm").arg("--version").output();

    if npm_check.is_err() {
        println!("cargo:warning=未检测到 npm，跳过前端构建");
        return;
    }

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
    // 判断是否为 release 构建
    let is_release = std::env::var("PROFILE").unwrap_or_default() == "release";

    // 只获取一次 metadata
    let metadata = get_cargo_metadata();
    ensure_frontend_tree_sitter_assets(&metadata);

    if !is_release {
        run_npm_build();
    } else {
        println!("cargo:warning=Release 构建，跳过 npm 构建");
    }

    // 补充版本号
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
