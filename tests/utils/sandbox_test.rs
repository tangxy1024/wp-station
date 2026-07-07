//! 沙盒模块集成测试。
//!
//! 覆盖工作区管理（输出收集、目录渲染、日志打包）和进程管理（命令检测、版本查询）。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::common::{setup_db, test_infra_root};
use wp_station::constants::sandbox::OUTPUT_PATHS;
use wp_station::server::Setting;
use wp_station::server::sandbox::analyze::{RuntimeMetrics, finalize_conclusion};
use wp_station::utils::SystemKind;
use wp_station::utils::sandbox::{SandboxWorkspace, collect_output_checks, command_version_output};

// ============ 测试辅助函数 ============

fn temp_dir(prefix: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    fs::create_dir_all(&path).unwrap();
    path
}

fn temp_script(prefix: &str, contents: &str, ext: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        ext
    );
    let path = std::env::temp_dir().join(unique);
    fs::write(&path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
    path
}

fn create_sandbox_workspace_fixture(task_id: &str) -> SandboxWorkspace {
    let root = Setting::workspace_root()
        .join("tmp")
        .join("sandbox")
        .join(task_id);
    let project_dir = root.join("project");
    let logs_dir = root.join("logs");

    for relative in [
        "conf/wparse.toml",
        "connectors/source.d/00-file-default.toml",
        "models/wpl/demo/parse.wpl",
        "topology/sources/wpsrc.toml",
        "data/out_dat/miss.dat",
    ] {
        let path = project_dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "fixture\n").unwrap();
    }

    fs::create_dir_all(&logs_dir).unwrap();
    fs::write(logs_dir.join("analysis.log"), "fixture\n").unwrap();

    SandboxWorkspace {
        root: root.clone(),
        project_dir,
        logs_dir,
        source_models_root: root.join("source-models"),
        source_infra_root: root.join("source-infra"),
        source_connectors_root: root.join("source-connectors"),
    }
}

fn cleanup_sandbox_workspace_fixture(task_id: &str) {
    let root = Setting::workspace_root()
        .join("tmp")
        .join("sandbox")
        .join(task_id);
    let _ = fs::remove_dir_all(root);
}

// ============ 工作区测试 ============

#[test]
fn collect_output_checks_counts_lines() {
    let base = temp_dir("collect-output");
    for (idx, (relative, _)) in OUTPUT_PATHS.iter().enumerate() {
        let path = base.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let content = if idx % 2 == 0 { "line1\nline2\n" } else { "" };
        fs::write(&path, content).unwrap();
    }

    let status = collect_output_checks(&base).expect("collect output succeeds");
    assert_eq!(status.len(), OUTPUT_PATHS.len());

    let mut map: HashMap<String, (usize, bool)> = HashMap::new();
    for item in status {
        map.insert(item.relative_path.clone(), (item.line_count, item.is_empty));
    }

    assert!(map.get("data/out_dat/default.dat").unwrap().0 >= 2);
    assert!(map.get("data/out_dat/miss.dat").unwrap().1);

    fs::remove_dir_all(&base).unwrap();
}

#[test]
fn display_relative_prefers_workspace_root() {
    let workspace_root = Setting::workspace_root().clone();
    let logs_dir = workspace_root.join("logs");
    let ws = SandboxWorkspace {
        root: workspace_root.clone(),
        project_dir: workspace_root.clone(),
        logs_dir,
        source_models_root: workspace_root.clone(),
        source_infra_root: workspace_root.clone(),
        source_connectors_root: workspace_root.clone(),
    };

    let nested = workspace_root.join("foo/bar/example.txt");
    let display = ws.display_relative(&nested);

    assert!(display.contains("foo"));
    assert!(!display.starts_with('/'));
    assert!(!display.starts_with('\\'));
}

#[test]
fn render_tree_listing_displays_structure() {
    let base = temp_dir("render-tree");
    let logs_dir = base.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();
    let project_dir = base.join("project");
    fs::create_dir_all(project_dir.join("dir_a/dir_b")).unwrap();
    fs::write(project_dir.join("dir_a/file.txt"), "data").unwrap();

    let workspace = SandboxWorkspace {
        root: base.clone(),
        project_dir: project_dir.clone(),
        logs_dir,
        source_models_root: base.join("source-models"),
        source_infra_root: base.join("source-infra"),
        source_connectors_root: base.join("source-connectors"),
    };

    let listing = workspace
        .render_tree_listing(3, 10)
        .expect("render tree listing");
    assert!(listing.contains("dir_a"));
    assert!(listing.contains("file.txt"));

    fs::remove_dir_all(&base).unwrap();
}

// ============ 进程管理测试 ============

#[tokio::test]
async fn command_version_output_reads_stdout() {
    #[cfg(windows)]
    let script = temp_script("version-ok", "@echo off\necho v9.9.9\n", ".cmd");
    #[cfg(not(windows))]
    let script = temp_script("version-ok", "#!/bin/sh\necho v9.9.9\n", "");

    let output = command_version_output(script.to_str().unwrap())
        .await
        .expect("script should run");
    assert_eq!(output.trim(), "v9.9.9");
}

#[tokio::test]
async fn command_version_output_reports_failure() {
    #[cfg(windows)]
    let script = temp_script(
        "version-fail",
        "@echo off\necho error>&2\nexit /b 1\n",
        ".cmd",
    );
    #[cfg(not(windows))]
    let script = temp_script("version-fail", "#!/bin/sh\necho error 1>&2\nexit 1\n", "");

    let err = command_version_output(script.to_str().unwrap())
        .await
        .expect_err("failing script should error");
    assert!(format!("{}", err).contains("返回非 0"));
}

#[test]
fn patch_wpsrc_runtime_disables_non_udp_sources() {
    let runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(setup_db());

    let input = r#"[[sources]]
key = "gen_udp"
enable = false
connect = "old_udp"

[sources.params]
addr = "127.0.0.1"
port = 10000
protocol = "udp"

[[sources]]
key = "gen_kafka"
enable = true
connect = "kafka_src"

[sources.params]
brokers = "localhost:9092"
topic = "demo"

[[sources]]
key = "gen_ps"
connect = "ps_src"

[sources.params]
endpoint = "ps://127.0.0.1:6650"
"#;

    let source_file = test_infra_root().join("topology/sources/wpsrc.toml");
    fs::write(&source_file, input).expect("write custom source config");

    let workspace = SandboxWorkspace::prepare("sandbox-source-runtime", SystemKind::Wparse, &[])
        .expect("prepare sandbox workspace");
    let output = fs::read_to_string(workspace.project_dir.join("topology/sources/wpsrc.toml"))
        .expect("read patched source config");

    assert!(
        output
            .contains("key = \"gen_udp\"\nenable = true\nconnect = \"syslog_udp_src\"\ntags = []"),
        "sandbox source should rewrite target source block: {output}"
    );
    assert!(
        output.contains(
            "[sources.params]\naddr = \"0.0.0.0\"\nport = 31601\nprotocol = \"udp\"\nheader_mode = \"keep\""
        ),
        "sandbox source should use fixed runtime params: {output}"
    );
    assert!(output.contains("key = \"gen_kafka\"\nenable = false\nconnect = \"kafka_src\""));
    assert!(output.contains("key = \"gen_ps\"\nenable = false\nconnect = \"ps_src\""));

    let _ = fs::remove_dir_all(workspace.root);
}

#[test]
fn sandbox_prepare_overrides_infra_sinks_with_defaults() {
    let runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(setup_db());

    let user_infra_file = test_infra_root().join("topology/sinks/infra.d/error.toml");
    fs::write(
        &user_infra_file,
        "version = \"9.9\"\n[sink_group]\nname = \"custom\"\n",
    )
    .expect("write custom infra sink");

    let workspace = SandboxWorkspace::prepare("sandbox-infra-defaults", SystemKind::Wparse, &[])
        .expect("prepare sandbox workspace");

    let sandbox_file = workspace
        .project_dir
        .join("topology/sinks/infra.d/error.toml");
    let content = fs::read_to_string(&sandbox_file).expect("read sandbox infra sink");

    assert!(
        content.contains("connect = \"file_raw_sink\""),
        "sandbox infra sink should fall back to default config: {content}"
    );
    assert!(
        !content.contains("version = \"9.9\""),
        "sandbox should not keep user customized infra sink content: {content}"
    );

    let _ = fs::remove_dir_all(workspace.root);
}

#[test]
fn sandbox_prepare_overrides_wpgen_output_runtime() {
    let runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(setup_db());

    let wpgen_file = test_infra_root().join("conf/wpgen.toml");
    fs::write(
        &wpgen_file,
        r#"[output]
connect = "custom_sink"

[output.params]
addr = "127.0.0.1"
port = 9999
"#,
    )
    .expect("write custom wpgen config");

    let workspace = SandboxWorkspace::prepare("sandbox-wpgen-runtime", SystemKind::Wparse, &[])
        .expect("prepare sandbox workspace");
    let content = fs::read_to_string(workspace.project_dir.join("conf/wpgen.toml"))
        .expect("read patched wpgen config");

    assert!(content.contains("[output]\nconnect = \"udp_out_sink\""));
    assert!(content.contains("[output.params]\naddr = \"0.0.0.0\"\nport = 31601"));
    assert!(
        !content.contains("protocol = "),
        "sandbox wpgen output params should not keep protocol override: {content}"
    );

    let _ = fs::remove_dir_all(workspace.root);
}

#[test]
fn cleanup_after_run_keeps_recent_workspace_outputs() {
    let task_id = "sandbox-9000000000100-keep";
    cleanup_sandbox_workspace_fixture(task_id);
    let workspace = create_sandbox_workspace_fixture(task_id);

    workspace
        .cleanup_after_run(false)
        .expect("cleanup recent sandbox workspace");

    assert!(workspace.project_dir.join("conf").is_dir());
    assert!(workspace.project_dir.join("connectors").is_dir());
    assert!(workspace.project_dir.join("models").is_dir());
    assert!(workspace.project_dir.join("topology").is_dir());
    assert!(
        workspace
            .project_dir
            .join("data/out_dat/miss.dat")
            .is_file()
    );
    assert!(workspace.logs_dir.join("analysis.log").is_file());

    cleanup_sandbox_workspace_fixture(task_id);
}

#[test]
fn cleanup_after_run_prunes_old_runtime_artifacts_but_keeps_merged_config() {
    let task_ids = [
        "sandbox-9000000000201-oldest",
        "sandbox-9000000000202-middle-a",
        "sandbox-9000000000203-middle-b",
        "sandbox-9000000000204-latest",
    ];

    for task_id in task_ids {
        cleanup_sandbox_workspace_fixture(task_id);
    }

    let oldest = create_sandbox_workspace_fixture(task_ids[0]);
    let middle_a = create_sandbox_workspace_fixture(task_ids[1]);
    let middle_b = create_sandbox_workspace_fixture(task_ids[2]);
    let latest = create_sandbox_workspace_fixture(task_ids[3]);

    latest
        .cleanup_after_run(false)
        .expect("cleanup sandbox history");

    assert!(oldest.project_dir.join("conf").is_dir());
    assert!(oldest.project_dir.join("connectors").is_dir());
    assert!(oldest.project_dir.join("models").is_dir());
    assert!(oldest.project_dir.join("topology").is_dir());
    assert!(!oldest.project_dir.join("data").exists());
    assert!(!oldest.logs_dir.exists());

    assert!(middle_a.project_dir.join("data/out_dat/miss.dat").is_file());
    assert!(middle_a.logs_dir.join("analysis.log").is_file());
    assert!(middle_b.project_dir.join("data/out_dat/miss.dat").is_file());
    assert!(middle_b.logs_dir.join("analysis.log").is_file());
    assert!(latest.project_dir.join("data/out_dat/miss.dat").is_file());
    assert!(latest.logs_dir.join("analysis.log").is_file());

    for task_id in task_ids {
        cleanup_sandbox_workspace_fixture(task_id);
    }
}

#[test]
fn finalize_conclusion_respects_runtime_analysis_result() {
    let metrics = RuntimeMetrics {
        input_count: 50,
        output_count: 5,
        passed: true,
        ..Default::default()
    };

    let conclusion = finalize_conclusion(&[], &metrics);
    assert!(conclusion.passed);
    assert_eq!(conclusion.input_count, 50);
    assert_eq!(conclusion.runtime_output_count, 5);
}
