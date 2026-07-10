//! Tree-sitter 运行时资源辅助。
//!
//! 本地 `cargo run` 下的资源同步由 `build.rs` 负责；
//! 这里仅保留运行时按路径读取磁盘资源的能力。

use std::fs;
use std::path::PathBuf;

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn repo_root() -> PathBuf {
    PathBuf::from(MANIFEST_DIR)
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
