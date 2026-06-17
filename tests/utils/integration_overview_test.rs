use std::fs;

use wp_station::server::ProjectLayout;
use wp_station::utils::load_integration_runtime_overview_from_layout;

fn write_file(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directories");
    }
    fs::write(path, content).expect("write file");
}

#[test]
fn test_load_integration_runtime_overview_extracts_source_and_sink_details() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let layout = ProjectLayout {
        models_root: temp_dir.path().join("project_models"),
        infra_root: temp_dir.path().join("project_infra"),
    };

    write_file(
        &layout
            .infra_root
            .join("connectors/source.d/10-syslog-udp.toml"),
        r#"
id = "syslog_udp_src"
type = "syslog"
allow_override = ["addr", "port", "protocol", "header_mode"]

[connectors.params]
addr = "0.0.0.0"
port = 514
protocol = "udp"
header_mode = "strip"
"#,
    );
    write_file(
        &layout
            .infra_root
            .join("connectors/sink.d/02-file-json.toml"),
        r#"
id = "file_json_sink"
type = "file"
allow_override = ["base", "file", "sync"]

[connectors.params]
base = "./data/out_dat"
file = "default.json"
sync = false
"#,
    );
    write_file(
        &layout.infra_root.join("topology/sources/wpsrc.toml"),
        r#"
[[sources]]
key = "gen_udp"
enable = true
connect = "syslog_udp_src"

[sources.params]
addr = "0.0.0.0"
port = 31601
protocol = "udp"
header_mode = "strip"
"#,
    );
    write_file(
        &layout
            .infra_root
            .join("topology/sinks/business.d/sink.toml"),
        r#"
version = "1.0"

[sink_group]
name = "all"
oml = ["*"]
parallel = 1

[[sink_group.sinks]]
name = "all_sink"
connect = "file_json_sink"
tags = []

[sink_group.sinks.params]
base = "./data/out_dat/"
file = "all.json"
"#,
    );

    let overview =
        load_integration_runtime_overview_from_layout(&layout).expect("load integration overview");

    assert_eq!(overview.supported_source_type_count, 1);
    assert_eq!(overview.supported_sink_type_count, 1);
    assert_eq!(overview.sources.len(), 1);
    assert_eq!(overview.sinks.len(), 1);

    let source = &overview.sources[0];
    assert_eq!(source.title, "gen_udp");
    assert_eq!(source.type_key, "syslog-udp");
    assert!(source.detail.contains("地址 0.0.0.0"));
    assert!(source.detail.contains("端口 31601"));
    assert!(source.detail.contains("协议 udp"));

    let sink = &overview.sinks[0];
    assert_eq!(sink.title, "sink.toml");
    assert_eq!(sink.type_key, "file");
    assert_eq!(sink.detail, "文件路径 ./data/out_dat/all.json");
}
