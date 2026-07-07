//! Gitea 同步辅助模块。
//!
//! 统一处理双仓库的初始化、同步、删除同步和基线 tag 管理。
//! 双系统改造后，仓库选择全部通过 `system + area/group` 解析。

mod publish;
mod setup;

use crate::constants::project::REPO_SHARED_CONNECTORS;
use crate::db::{ReleaseGroup, RuleType};
use crate::error::AppError;
use crate::server::{RepoLayout, Setting};
use crate::utils::{ProjectArea, SystemKind, layout_for_system, shared_connectors_root};
use gitea::{GiteaClient, GiteaConfig};
use std::path::PathBuf;

pub use self::publish::{get_next_version, push_and_tag_release};
pub use self::setup::{ensure_project_repositories, init_gitea_repo};

/// 构造 Gitea 客户端。
fn build_gitea_client(setting: &Setting) -> Result<GiteaClient, AppError> {
    let gitea_config = GiteaConfig::new(
        setting.gitea.base_url.clone(),
        setting.gitea.username.clone(),
        setting.gitea.password.clone(),
    )
    .with_branch("main".to_string());

    GiteaClient::new(gitea_config)
        .map_err(|e| AppError::internal(format!("创建 Gitea 客户端失败: {}", e)))
}

/// 根据发布分组返回对应的本地仓库目录。
fn repo_path_for_group(layout: &RepoLayout, group: ReleaseGroup) -> PathBuf {
    match group {
        ReleaseGroup::Models => layout.models_root.clone(),
        ReleaseGroup::Infra => layout.infra_root.clone(),
    }
}

/// 将发布分组映射为固定仓库区域。
fn area_from_group(group: ReleaseGroup) -> ProjectArea {
    match group {
        ReleaseGroup::Models => ProjectArea::Models,
        ReleaseGroup::Infra => ProjectArea::Infra,
    }
}

/// 是否通过环境变量跳过 Gitea 同步。
fn should_skip_gitea_sync() -> bool {
    std::env::var("WARP_STATION_SKIP_GITEA")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// 同步指定分组仓库到 Gitea（支持自动处理冲突）
pub async fn sync_to_gitea(
    commit_message: &str,
    system: SystemKind,
    group: ReleaseGroup,
) -> Result<(), AppError> {
    if should_skip_gitea_sync() {
        info!(
            "跳过 Gitea 同步: group={}, reason=WARP_STATION_SKIP_GITEA",
            group.as_ref()
        );
        return Ok(());
    }

    let setting = Setting::load();
    let layout = layout_for_system(system).as_repo_layout();
    let gitea_client = build_gitea_client(&setting)?;
    let project_path = repo_path_for_group(&layout, group);
    publish::sync_repo_with_retry(
        &gitea_client,
        &project_path,
        commit_message,
        system,
        area_from_group(group),
    )
}

/// 同步所有仓库到 Gitea。
pub async fn sync_to_gitea_all(commit_message: &str, system: SystemKind) -> Result<(), AppError> {
    sync_to_gitea(commit_message, system, ReleaseGroup::Models).await?;
    sync_to_gitea(commit_message, system, ReleaseGroup::Infra).await?;
    Ok(())
}

/// 同步共享 connectors 仓库到 Gitea。
pub async fn sync_shared_connectors_to_gitea(commit_message: &str) -> Result<(), AppError> {
    if should_skip_gitea_sync() {
        info!("跳过共享 connectors Gitea 同步: reason=WARP_STATION_SKIP_GITEA");
        return Ok(());
    }

    let setting = Setting::load();
    let gitea_client = build_gitea_client(&setting)?;
    let project_path = shared_connectors_root();
    publish::sync_named_repo_with_retry(
        &gitea_client,
        &project_path,
        commit_message,
        &format!("repo={}", REPO_SHARED_CONNECTORS),
    )
}

/// 同步删除到 Gitea
pub async fn sync_delete_to_gitea(
    system: SystemKind,
    rule_type: RuleType,
    file_name: &str,
) -> Result<(), AppError> {
    let commit_message = format!("删除 {} 文件: {}", rule_type.as_ref(), file_name);
    if matches!(rule_type, RuleType::SourceConnect | RuleType::SinkConnect) {
        sync_shared_connectors_to_gitea(&commit_message).await?;
    } else {
        sync_to_gitea(
            &commit_message,
            system,
            ReleaseGroup::from_rule_type(rule_type),
        )
        .await?;
    }
    Ok(())
}
