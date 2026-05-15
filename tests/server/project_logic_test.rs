use std::fs;
use std::path::PathBuf;

use crate::common::{
    rand_suffix, setup_db, test_base_root, test_infra_root, test_models_root, test_project_layout,
};
use wp_station::server::project::{ProjectImportRequest, import_project_from_files_logic};
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
