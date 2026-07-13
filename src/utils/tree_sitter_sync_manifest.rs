/// Tree-sitter 语言资产远程源定义。
///
/// 这份定义同时被 `build.rs` 和运行时开发态同步逻辑复用，
/// 避免两边各自维护一套仓库地址和清单路径。
#[derive(Clone, Copy, Debug)]
pub(crate) struct TreeSitterAssetSource {
    pub package_name: &'static str,
    pub repo_url: &'static str,
    pub branch: &'static str,
    pub manifest_relative: &'static str,
    pub local_override_root: Option<&'static str>,
}

pub(crate) const TREE_SITTER_ASSET_SOURCES: &[TreeSitterAssetSource] = &[
    TreeSitterAssetSource {
        package_name: "tree-sitter-wpl",
        repo_url: "https://github.com/wp-labs/tree-sitter-wpl.git",
        branch: "main",
        manifest_relative: "editor/asset-manifest.json",
        local_override_root: Some("../wp-tree-sitter/tree-sitter-wpl"),
    },
    TreeSitterAssetSource {
        package_name: "tree-sitter-oml",
        repo_url: "https://github.com/wp-labs/tree-sitter-oml.git",
        branch: "main",
        manifest_relative: "editor/asset-manifest.json",
        local_override_root: Some("../wp-tree-sitter/tree-sitter-oml"),
    },
    TreeSitterAssetSource {
        package_name: "tree-sitter-wfl",
        repo_url: "https://github.com/wp-labs/tree-sitter-wfl.git",
        branch: "main",
        manifest_relative: "editor/wfs/asset-manifest.json",
        local_override_root: Some("../wp-tree-sitter/tree-sitter-wfl"),
    },
    TreeSitterAssetSource {
        package_name: "tree-sitter-wfl",
        repo_url: "https://github.com/wp-labs/tree-sitter-wfl.git",
        branch: "main",
        manifest_relative: "editor/wfl/asset-manifest.json",
        local_override_root: Some("../wp-tree-sitter/tree-sitter-wfl"),
    },
    TreeSitterAssetSource {
        package_name: "tree-sitter-wfl",
        repo_url: "https://github.com/wp-labs/tree-sitter-wfl.git",
        branch: "main",
        manifest_relative: "editor/wfg/asset-manifest.json",
        local_override_root: Some("../wp-tree-sitter/tree-sitter-wfl"),
    },
];
