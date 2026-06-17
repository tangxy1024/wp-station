use crate::common::{setup_db, test_infra_root};
use actix_web::{App, http::StatusCode, test};

fn write_file(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent directories");
    }
    std::fs::write(path, content).expect("write file");
}

#[actix_web::test]
async fn test_system_hello_endpoint() {
    setup_db().await;
    let app = test::init_service(App::new().service(wp_station::api::hello)).await;

    let req = test::TestRequest::get().uri("/api/hello").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let message: String = serde_json::from_slice(&body).expect("parse hello response");
    assert!(message.contains("Actix-web"));
}

#[actix_web::test]
async fn test_system_version_endpoint() {
    setup_db().await;
    let app = test::init_service(App::new().service(wp_station::api::get_version)).await;

    let req = test::TestRequest::get().uri("/api/version").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let payload: serde_json::Value = test::read_body_json(resp).await;
    assert!(payload.get("wp_station").is_some());
    assert!(payload.get("wp_parse").is_some());
}

#[actix_web::test]
async fn test_integration_runtime_overview_endpoint() {
    setup_db().await;
    let infra_root = test_infra_root();
    write_file(
        &infra_root.join("topology/sources/wpsrc.toml"),
        r#"
[[sources]]
key = "gen_udp"
enable = true
connect = "syslog_udp_src"

[sources.params]
addr = "0.0.0.0"
port = 31601
protocol = "udp"
"#,
    );
    write_file(
        &infra_root.join("topology/sinks/business.d/sink.toml"),
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

    let app =
        test::init_service(App::new().service(wp_station::api::get_integration_runtime_overview))
            .await;

    let req = test::TestRequest::get()
        .uri("/api/integration-overview/runtime")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let payload: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        payload["sources"].as_array().map(|items| items.len()),
        Some(1)
    );
    assert_eq!(
        payload["sinks"].as_array().map(|items| items.len()),
        Some(1)
    );
    assert_eq!(
        payload["sources"][0]["detail"],
        "地址 0.0.0.0，端口 31601，协议 udp"
    );
    assert_eq!(
        payload["sinks"][0]["detail"],
        "文件路径 ./data/out_dat/all.json"
    );
}
