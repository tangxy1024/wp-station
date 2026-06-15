use std::fs;
use std::path::PathBuf;

use crate::common::{
    rand_suffix, setup_db, test_base_root, test_infra_root, test_models_root, test_project_layout,
};
use flate2::Compression;
use flate2::write::GzEncoder;
use wp_station::db::{
    ReleaseStatus, find_all_releases, find_latest_draft_release, update_release_status,
};
use wp_station::server::project::{
    ProjectImportRequest, confirm_project_archive_import_logic, import_project_from_files_logic,
    preview_project_archive_logic,
};
use wp_station::utils::compose_project_layout_into;

fn legacy_import_dir(name: &str) -> PathBuf {
    let path = test_base_root().join(format!("legacy-import-{}-{}", name, rand_suffix()));
    fs::create_dir_all(&path).expect("create legacy import dir");
    path
}

fn write_file(path: PathBuf, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

fn copy_dir(source: PathBuf, target: PathBuf) {
    fs::create_dir_all(&target).expect("create target dir");
    for entry in fs::read_dir(source).expect("read source dir") {
        let entry = entry.expect("read source entry");
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(source_path, target_path);
        } else {
            fs::copy(&source_path, &target_path).expect("copy file");
        }
    }
}

fn build_archive_with_dirs(source_dir: &PathBuf, dirs: &[&str]) -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for dir in dirs {
        builder
            .append_dir_all(*dir, source_dir.join(dir))
            .expect("append dir to archive");
    }
    let encoder = builder.into_inner().expect("finish tar builder");
    encoder.finish().expect("finish gzip encoder")
}

#[tokio::test]
async fn test_import_project_requires_legacy_directories() {
    setup_db().await;
    let source_dir = legacy_import_dir("missing-dirs");
    fs::create_dir_all(source_dir.join("conf")).expect("create conf only");

    let result = import_project_from_files_logic(
        Some("tester".to_string()),
        ProjectImportRequest {
            source_dir: source_dir.to_string_lossy().to_string(),
        },
    )
    .await;

    let err = match result {
        Ok(_) => panic!("should reject incomplete legacy directory"),
        Err(err) => err,
    };
    assert!(format!("{err}").contains("缺少必要目录"));

    let _ = fs::remove_dir_all(source_dir);
}

#[tokio::test]
async fn test_import_project_validates_source_dir_before_overwrite() {
    setup_db().await;
    let source_dir = legacy_import_dir("invalid-components");
    compose_project_layout_into(&test_project_layout(), &source_dir)
        .expect("compose dual repo into legacy project");

    write_file(
        source_dir.join("topology/sources/wpsrc.toml"),
        "[[sources]]\nkey = \"gen_udp\"\nenable = true\nconnect = \"broken\"\n",
    );
    write_file(test_infra_root().join("sentinel.txt"), "keep infra");
    write_file(test_models_root().join("sentinel.txt"), "keep models");

    let result = import_project_from_files_logic(
        Some("tester".to_string()),
        ProjectImportRequest {
            source_dir: source_dir.to_string_lossy().to_string(),
        },
    )
    .await;

    let err = match result {
        Ok(_) => panic!("should fail on source component validation"),
        Err(err) => err,
    };
    assert!(
        format!("{err}").contains("加载项目失败") || format!("{err}").contains("组件校验失败"),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read_to_string(test_infra_root().join("sentinel.txt")).expect("read infra sentinel"),
        "keep infra"
    );
    assert_eq!(
        fs::read_to_string(test_models_root().join("sentinel.txt")).expect("read models sentinel"),
        "keep models"
    );

    let _ = fs::remove_dir_all(source_dir);
}

#[tokio::test]
async fn test_import_project_splits_legacy_directory_into_dual_repos() {
    setup_db().await;
    let source_dir = legacy_import_dir("success");
    compose_project_layout_into(&test_project_layout(), &source_dir)
        .expect("compose dual repo into legacy project");

    write_file(
        source_dir.join("topology/sources/wpsrc.toml"),
        "[[sources]]\nkey = \"gen_udp\"\nenable = true\nconnect = \"syslog_udp_src\"\n[sources.params]\nport = 31609\n",
    );

    write_file(test_models_root().join("stale.txt"), "old models");
    write_file(test_infra_root().join("stale.txt"), "old infra");

    let response = import_project_from_files_logic(
        Some("tester".to_string()),
        ProjectImportRequest {
            source_dir: source_dir.to_string_lossy().to_string(),
        },
    )
    .await
    .expect("import legacy project");

    assert_eq!(
        response.summary.source_dir,
        source_dir.to_string_lossy().to_string()
    );
    assert_eq!(
        response.summary.project_models,
        test_models_root().to_string_lossy().to_string()
    );
    assert_eq!(
        response.summary.project_infra,
        test_infra_root().to_string_lossy().to_string()
    );
    assert!(response.summary.rules_imported > 0);
    assert!(response.validation.passed);

    assert!(test_infra_root().join("conf/wparse.toml").exists());
    assert!(
        test_infra_root()
            .join("topology/sources/wpsrc.toml")
            .exists()
    );
    assert!(test_models_root().join("models/wpl").exists());
    assert!(test_models_root().join("models/knowledge").exists());
    assert!(!test_models_root().join("stale.txt").exists());
    assert!(!test_infra_root().join("stale.txt").exists());

    let _ = fs::remove_dir_all(source_dir);
}

#[tokio::test]
async fn test_import_project_archive_supports_models_only_directory() {
    setup_db().await;
    if let Some(draft) = find_latest_draft_release()
        .await
        .expect("query draft before import")
    {
        update_release_status(draft.id, ReleaseStatus::INIT, None, None)
            .await
            .expect("archive existing draft before import");
    }
    let source_dir = legacy_import_dir("archive-models-only");
    copy_dir(test_models_root().join("models"), source_dir.join("models"));
    write_file(
        source_dir.join("models/archive-only.txt"),
        "archive models only",
    );
    write_file(test_infra_root().join("sentinel.txt"), "keep infra");

    let preview = preview_project_archive_logic(
        Some("tester".to_string()),
        "models-only.tar.gz",
        build_archive_with_dirs(&source_dir, &["models"]),
    )
    .await
    .expect("preview models-only archive");

    assert_eq!(preview.summary.imported_dirs, vec!["models".to_string()]);
    assert_eq!(
        preview.summary.retained_dirs,
        vec![
            "conf".to_string(),
            "connectors".to_string(),
            "topology".to_string()
        ]
    );

    let response =
        confirm_project_archive_import_logic(Some("tester".to_string()), &preview.import_id)
            .await
            .expect("confirm models-only archive");

    assert_eq!(response.summary.imported_dirs, vec!["models".to_string()]);
    assert_eq!(
        fs::read_to_string(test_infra_root().join("sentinel.txt")).expect("read infra sentinel"),
        "keep infra"
    );
    assert_eq!(
        fs::read_to_string(test_models_root().join("models/archive-only.txt"))
            .expect("read imported models marker"),
        "archive models only"
    );
    let draft = find_latest_draft_release()
        .await
        .expect("query draft after import")
        .expect("draft should be recreated after archive import");
    assert_eq!(draft.status, ReleaseStatus::WAIT.as_ref());
    let (releases, total) = find_all_releases(1, 20, None, None, None, None)
        .await
        .expect("query release list after import");
    assert!(
        total >= 1,
        "expected at least one visible release after import"
    );
    assert!(
        releases.iter().any(|release| release.id == draft.id),
        "draft release should be visible in release list"
    );

    let _ = fs::remove_dir_all(source_dir);
}

#[tokio::test]
async fn test_import_project_archive_supports_conf_only_directory() {
    setup_db().await;
    let source_dir = legacy_import_dir("archive-conf-only");
    copy_dir(test_infra_root().join("conf"), source_dir.join("conf"));
    write_file(
        test_models_root().join("sentinel-models.txt"),
        "keep models",
    );

    let preview = preview_project_archive_logic(
        Some("tester".to_string()),
        "conf-only.tar.gz",
        build_archive_with_dirs(&source_dir, &["conf"]),
    )
    .await
    .expect("preview conf-only archive");

    assert_eq!(preview.summary.imported_dirs, vec!["conf".to_string()]);
    assert_eq!(
        preview.summary.retained_dirs,
        vec![
            "connectors".to_string(),
            "topology".to_string(),
            "models".to_string()
        ]
    );

    let response =
        confirm_project_archive_import_logic(Some("tester".to_string()), &preview.import_id)
            .await
            .expect("confirm conf-only archive");

    assert_eq!(response.summary.imported_dirs, vec!["conf".to_string()]);
    assert_eq!(
        fs::read_to_string(test_models_root().join("sentinel-models.txt"))
            .expect("read models sentinel"),
        "keep models"
    );

    let _ = fs::remove_dir_all(source_dir);
}
