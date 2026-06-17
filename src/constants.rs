//! 统一业务常量定义。
//!
//! 收敛主应用中的目录名、文件名、发布分组、沙盒运行时默认值等可共享常量，
//! 避免在多个模块中重复硬编码。

pub mod project {
    pub const REPO_MODELS: &str = "project_models";
    pub const REPO_INFRA: &str = "project_infra";

    pub const DIR_CONF: &str = "conf";
    pub const DIR_CONNECTORS: &str = "connectors";
    pub const DIR_TOPOLOGY: &str = "topology";
    pub const DIR_MODELS: &str = "models";

    pub const DIR_PROJECT_MODELS_WRAPPER: &str = "project_models";
    pub const DIR_PROJECT_INFRA_WRAPPER: &str = "project_infra";

    pub const DIR_SOURCE_D: &str = "source.d";
    pub const DIR_SINK_D: &str = "sink.d";
    pub const DIR_SOURCES: &str = "sources";
    pub const DIR_SINKS: &str = "sinks";
    pub const DIR_WPL: &str = "wpl";
    pub const DIR_OML: &str = "oml";
    pub const DIR_KNOWLEDGE: &str = "knowledge";
    pub const DIR_BUSINESS_D: &str = "business.d";
    pub const DIR_INFRA_D: &str = "infra.d";

    pub const FILE_WPARSE: &str = "wparse.toml";
    pub const FILE_WPGEN: &str = "wpgen.toml";
    pub const FILE_KNOWDB: &str = "knowdb.toml";
    pub const FILE_WPL_PARSE: &str = "parse.wpl";
    pub const FILE_WPL_SAMPLE: &str = "sample.dat";
    pub const FILE_OML_ADM: &str = "adm.oml";
    pub const FILE_WPSRC: &str = "wpsrc.toml";
    pub const FILE_DEFAULTS: &str = "defaults.toml";
    pub const FILE_PRIVACY: &str = "privacy.toml";
    pub const FILE_BUSINESS_SINK: &str = "sink.toml";

    pub const IMPORTABLE_ROOT_DIRS: [&str; 4] =
        [DIR_CONF, DIR_CONNECTORS, DIR_TOPOLOGY, DIR_MODELS];
    pub const ARCHIVE_IMPORT_STAGING_DIR: &str = "project-archive-imports";

    pub const SINK_DISPLAY_FALLBACKS: &[(&str, &str)] = &[
        ("business.d/sink.toml", "输出配置"),
        ("infra.d/monitor.toml", "监控数据"),
        ("infra.d/miss.toml", "未命中WPL数据"),
        ("infra.d/default.toml", "未命中OML数据"),
        ("infra.d/error.toml", "异常数据"),
        ("infra.d/residue.toml", "残留数据"),
        ("infra.d/intercept.toml", "拦截数据"),
        ("privacy.toml", "隐私数据"),
    ];
}

pub mod config {
    pub const CONNECTOR_DISPLAY_FALLBACKS: &[(&str, &str)] = &[
        ("00-file-default.toml", "File"),
        ("10-syslog-udp.toml", "Syslog (UDP)"),
        ("11-syslog-tcp.toml", "Syslog (TCP)"),
        ("12-tcp.toml", "TCP"),
        ("20-http.toml", "HTTP"),
        ("30-kafka.toml", "Kafka"),
        ("40-mysql.toml", "MySQL"),
        ("50-postgres.toml", "Postgres"),
        ("60-dmdb-connect_string.toml", "DMDB (Connection String)"),
        ("61-dmdb-endpoint.toml", "DMDB (Endpoint)"),
        ("62-dmdb-dsn.toml", "DMDB (DSN)"),
        ("00-blackhole-sink.toml", "Blackhole"),
        ("01-file-prototext.toml", "File (Prototext)"),
        ("02-file-json.toml", "File (JSON)"),
        ("03-file-kv.toml", "File (KV)"),
        ("04-file-raw.toml", "File (RAW)"),
        ("09-file-test.toml", "Test Rescue"),
        ("13-udp.toml", "UDP"),
        ("14-count.toml", "Count"),
        ("40-prometheus.toml", "Prometheus"),
        ("50-mysql.toml", "MySQL"),
        ("60-doris.toml", "Doris"),
        ("60-postgres.toml", "Postgres"),
        ("70-victorialogs.toml", "VictoriaLogs"),
        ("80-victoriametrics.toml", "VictoriaMetrics"),
        ("90-elasticsearch.toml", "Elasticsearch"),
        ("100-clickhouse.toml", "ClickHouse"),
        ("101-http.toml", "HTTP"),
        ("110-dmdb-connect_string.toml", "DMDB (Connection String)"),
        ("111-dmdb-endpoint.toml", "DMDB (Endpoint)"),
        ("112-dmdb-dsn.toml", "DMDB (DSN)"),
    ];

    pub const CONNECTION_FILE_ORDER: &[&str] = &[
        "00-file-default.toml",
        "10-syslog-udp.toml",
        "11-syslog-tcp.toml",
        "12-tcp.toml",
        "30-kafka.toml",
        "40-mysql.toml",
        "00-blackhole-sink.toml",
        "01-file-prototext.toml",
        "02-file-json.toml",
        "03-file-kv.toml",
        "04-file-raw.toml",
        "09-file-test.toml",
        "40-prometheus.toml",
        "50-mysql.toml",
        "60-doris.toml",
        "60-postgres.toml",
        "70-victorialogs.toml",
        "80-victoriametrics.toml",
        "90-elasticsearch.toml",
        "100-clickhouse.toml",
        "101-http.toml",
    ];

    pub const SINK_FILE_ORDER: &[&str] = &[
        "business.d/sink.toml",
        "infra.d/monitor.toml",
        "infra.d/miss.toml",
        "infra.d/default.toml",
        "infra.d/error.toml",
        "infra.d/residue.toml",
    ];

    pub const CONNECTOR_TYPE_DISPLAY_NAMES: &[(&str, &str)] = &[
        ("file", "文件"),
        ("kafka", "Kafka"),
        ("dmdb", "达梦数据库"),
        ("mysql", "MySQL"),
        ("postgres", "PostgreSQL"),
        ("doris", "Doris"),
        ("clickhouse", "ClickHouse"),
        ("elasticsearch", "Elasticsearch"),
        ("victorialogs", "VictoriaLogs"),
        ("victoriametrics", "VictoriaMetrics"),
        ("prometheus", "Prometheus"),
        ("http", "HTTP"),
        ("syslog-udp", "Syslog UDP"),
        ("syslog-tcp", "Syslog TCP"),
        ("tcp", "TCP"),
        ("udp", "UDP"),
    ];
}

pub mod sandbox {
    pub const DEFAULT_HISTORY_LIMIT: u64 = 20;
    pub const MAX_LOG_LINES: usize = 500;

    pub const OUTPUT_PATHS: [(&str, &str); 4] = [
        ("data/out_dat/default.dat", "数据命中兜底路由"),
        ("data/out_dat/miss.dat", "样本未命中任何规则"),
        ("data/out_dat/residue.dat", "存在残余未处理数据"),
        ("data/out_dat/error.dat", "处理过程中出现错误"),
    ];

    pub const BUSINESS_SINK_OVERRIDE: &str = r#"version = "1.0"

[sink_group]
name = "kafka_sink"
oml = ["*"]
parallel = 1

[[sink_group.sinks]]
name = "all_sink"
connect = "file_json_sink"
tags = []

[sink_group.sinks.params]
base = "./data/out_dat/"
file = "all.json"
"#;

    pub const RUNTIME_UDP_PORT: u16 = 31601;
    pub const RUNTIME_SOURCE_KEY: &str = "gen_udp";
    pub const RUNTIME_SOURCE_CONNECTOR: &str = "syslog_udp_src";
    pub const RUNTIME_OUTPUT_CONNECTOR: &str = "udp_out_sink";
    pub const RUNTIME_SOURCE_ADDR: &str = "0.0.0.0";
    pub const RUNTIME_OUTPUT_ADDR: &str = "0.0.0.0";
    pub const RUNTIME_PROTOCOL: &str = "udp";
    pub const RUNTIME_HEADER_MODE: &str = "keep";

    pub const DAEMON_READY_BEFORE_WPGEN_WAIT_MS: u64 = 1_000;
}

pub mod release {
    pub const GROUP_MODELS: &str = "models";
    pub const GROUP_INFRA: &str = "infra";
    pub const GROUP_ALL: &str = "all";
    pub const GROUP_DRAFT: &str = "draft";

    pub const MAX_BATCH_SIZE: u64 = 50;
    pub const LOOP_IDLE_SECONDS: u64 = 1;
    pub const FIRST_POLL_DELAY_SECONDS: i64 = 1;

    pub const STAGE_CALL_CLIENT: &str = "调用客户端";
    pub const STAGE_RUNTIME: &str = "运行状态";

    pub fn group_title(group: &str) -> &str {
        match group {
            GROUP_MODELS => "规则配置",
            GROUP_INFRA => "设施配置",
            GROUP_ALL => "全量配置",
            GROUP_DRAFT => "草稿",
            _ => group,
        }
    }

    pub fn publish_label(group: &str) -> &'static str {
        match group {
            GROUP_MODELS => "发布规则",
            GROUP_INFRA => "发布设施",
            GROUP_ALL => "发布",
            _ => "发布",
        }
    }
}

pub mod warparse {
    pub const DEPLOY_PATH: &str = "/admin/v1/reloads/model";
    pub const STATUS_PATH: &str = "/admin/v1/runtime/status";
}

pub mod gitea {
    pub const REPO_BASELINE_TAG: &str = "baseline";
}

pub mod assist {
    pub const STALE_AI_TASK_RELEASE_SECONDS: i64 = 30 * 60;
}

pub mod device {
    pub const CREATE_DEVICE_CONNECT_TIMEOUT_SECONDS: u64 = 3;
}

pub mod api {
    pub const MAX_ARCHIVE_BYTES: usize = 200 * 1024 * 1024;
}
