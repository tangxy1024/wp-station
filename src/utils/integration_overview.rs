//! 接入概览运行时摘要提取工具。
//!
//! 负责扫描当前项目中的输入源与业务输出源配置，并结合 connectors 模板提取
//! 页面展示所需的关键信息，避免前端重复解析 TOML 与 connector 类型。

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use regex::Regex;
use serde::Deserialize;

use crate::constants::project::{
    DIR_BUSINESS_D, DIR_MODELS, DIR_SINKS, DIR_SOURCES, DIR_TOPOLOGY, DIR_WPL, FILE_WPL_PARSE,
    FILE_WPSRC,
};
use crate::db::RuleType;
use crate::error::AppError;
use crate::server::ProjectLayout;
use crate::utils::common::connector_type_display_name;
use crate::utils::config_templates::{display_name_from_file, list_config_templates_from_layout};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationRuntimeItem {
    pub key: String,
    pub title: String,
    pub connect: String,
    pub type_key: String,
    pub type_label: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationRuntimeOverview {
    pub sources: Vec<IntegrationRuntimeItem>,
    pub sinks: Vec<IntegrationRuntimeItem>,
    pub supported_source_type_count: usize,
    pub supported_sink_type_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationRuleLogType {
    pub key: String,
    pub log_type_name: String,
    pub rule_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationRuleItem {
    pub key: String,
    pub device_type: String,
    pub log_types: Vec<IntegrationRuleLogType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationRuleOverview {
    pub items: Vec<IntegrationRuleItem>,
}

#[derive(Debug, Clone)]
struct ConnectorMeta {
    type_key: String,
    type_label: String,
    default_params: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct SourceTopologyFile {
    #[serde(default)]
    sources: Vec<SourceTopologyItem>,
}

#[derive(Debug, Default, Deserialize)]
struct SourceTopologyItem {
    #[serde(default)]
    key: String,
    #[serde(default)]
    enable: bool,
    #[serde(default)]
    connect: String,
    #[serde(default)]
    params: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct SinkTopologyFile {
    #[serde(default)]
    sink_group: SinkGroupTopology,
}

#[derive(Debug, Default, Deserialize)]
struct SinkGroupTopology {
    #[serde(default)]
    sinks: Vec<SinkTopologyItem>,
}

#[derive(Debug, Default, Deserialize)]
struct SinkTopologyItem {
    #[serde(default)]
    name: String,
    #[serde(default)]
    connect: String,
    #[serde(default)]
    params: BTreeMap<String, toml::Value>,
}

/// 扫描当前项目中的输入源与业务输出源，并返回页面展示所需摘要。
pub fn load_integration_runtime_overview_from_layout(
    layout: &ProjectLayout,
) -> Result<IntegrationRuntimeOverview, AppError> {
    let source_meta_map = build_connector_meta_map(layout, RuleType::Source)?;
    let sink_meta_map = build_connector_meta_map(layout, RuleType::Sink)?;

    let source_file = layout
        .infra_root
        .join(DIR_TOPOLOGY)
        .join(DIR_SOURCES)
        .join(FILE_WPSRC);
    let sink_dir = layout
        .infra_root
        .join(DIR_TOPOLOGY)
        .join(DIR_SINKS)
        .join(DIR_BUSINESS_D);

    Ok(IntegrationRuntimeOverview {
        sources: summarize_sources(&source_file, &source_meta_map)?,
        sinks: summarize_business_sinks(&sink_dir, &sink_meta_map)?,
        supported_source_type_count: source_meta_map.len(),
        supported_sink_type_count: sink_meta_map.len(),
    })
}

/// 扫描项目中的 WPL 规则包，并提取接入概览页面展示所需的设备类型与日志类型摘要。
pub fn load_integration_rule_overview_from_layout(
    layout: &ProjectLayout,
) -> Result<IntegrationRuleOverview, AppError> {
    let wpl_root = layout.models_root.join(DIR_MODELS).join(DIR_WPL);
    if !wpl_root.exists() {
        return Ok(IntegrationRuleOverview { items: Vec::new() });
    }

    let mut package_dirs = fs::read_dir(&wpl_root)
        .map_err(AppError::internal)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    package_dirs.sort();

    let mut items = Vec::new();
    for package_dir in package_dirs {
        let package_key = package_dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if package_key.is_empty() || is_ignored_wpl_identifier(&package_key) {
            continue;
        }

        let parse_file = package_dir.join(FILE_WPL_PARSE);
        if !parse_file.exists() {
            continue;
        }

        let content = fs::read_to_string(&parse_file).map_err(AppError::internal)?;
        if let Some(item) = extract_wpl_rule_overview(&content, &package_key) {
            items.push(item);
        }
    }

    items.sort_by(|left, right| left.device_type.cmp(&right.device_type));
    Ok(IntegrationRuleOverview { items })
}

fn build_connector_meta_map(
    layout: &ProjectLayout,
    scope: RuleType,
) -> Result<HashMap<String, ConnectorMeta>, AppError> {
    let templates = list_config_templates_from_layout(layout, scope)?;
    let mut meta_map = HashMap::new();

    for template in templates {
        let default_params = template
            .fields
            .iter()
            .filter(|field| !field.advanced)
            .filter_map(|field| {
                field
                    .default_value
                    .as_ref()
                    .map(|value| (field.name.clone(), parse_template_default_value(value)))
            })
            .collect::<BTreeMap<_, _>>();

        let type_label = connector_type_display_name(&template.connector_type)
            .unwrap_or(&template.connector_type)
            .to_string();

        meta_map.insert(
            template.connect.clone(),
            ConnectorMeta {
                type_key: template.connector_type,
                type_label,
                default_params,
            },
        );
    }

    Ok(meta_map)
}

fn summarize_sources(
    path: &Path,
    meta_map: &HashMap<String, ConnectorMeta>,
) -> Result<Vec<IntegrationRuntimeItem>, AppError> {
    let parsed = parse_toml_file::<SourceTopologyFile>(path)?;
    let mut items = parsed
        .sources
        .into_iter()
        .filter(|item| item.enable && !item.connect.trim().is_empty())
        .map(|item| {
            let title = preferred_non_empty(&[&item.key, &item.connect]).to_string();
            let effective_params = build_effective_params(meta_map, &item.connect, &item.params);
            let (type_key, type_label) = infer_connector_type(
                meta_map.get(item.connect.as_str()),
                &item.connect,
                &effective_params,
            );

            IntegrationRuntimeItem {
                key: title.clone(),
                title,
                connect: item.connect.clone(),
                type_key: type_key.clone(),
                type_label,
                detail: build_connector_detail(&type_key, &effective_params),
            }
        })
        .collect::<Vec<_>>();

    items.sort_by(|left, right| {
        left.title
            .cmp(&right.title)
            .then_with(|| left.connect.cmp(&right.connect))
    });
    Ok(items)
}

fn summarize_business_sinks(
    dir: &Path,
    meta_map: &HashMap<String, ConnectorMeta>,
) -> Result<Vec<IntegrationRuntimeItem>, AppError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = fs::read_dir(dir)
        .map_err(AppError::internal)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    files.sort();

    let mut items = Vec::new();
    for path in files {
        let parsed = parse_toml_file::<SinkTopologyFile>(&path)?;
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        let title = display_name_from_file(&file_name);

        for (index, sink) in parsed.sink_group.sinks.into_iter().enumerate() {
            if sink.connect.trim().is_empty() {
                continue;
            }

            let effective_params = build_effective_params(meta_map, &sink.connect, &sink.params);
            let (type_key, type_label) = infer_connector_type(
                meta_map.get(sink.connect.as_str()),
                &sink.connect,
                &effective_params,
            );
            let index_label = index.to_string();
            let identity = preferred_non_empty(&[&sink.name, &sink.connect, &index_label]);

            items.push(IntegrationRuntimeItem {
                key: format!("{file_name}:{identity}"),
                title: title.clone(),
                connect: sink.connect.clone(),
                type_key: type_key.clone(),
                type_label,
                detail: build_connector_detail(&type_key, &effective_params),
            });
        }
    }

    items.sort_by(|left, right| {
        left.title
            .cmp(&right.title)
            .then_with(|| left.connect.cmp(&right.connect))
            .then_with(|| left.key.cmp(&right.key))
    });
    Ok(items)
}

fn parse_toml_file<T>(path: &Path) -> Result<T, AppError>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }

    let content = fs::read_to_string(path).map_err(AppError::internal)?;
    toml::from_str::<T>(&content).map_err(|err| {
        AppError::validation(format!(
            "解析 TOML 失败: path={}, error={}",
            path.display(),
            err
        ))
    })
}

fn build_effective_params(
    meta_map: &HashMap<String, ConnectorMeta>,
    connect: &str,
    params: &BTreeMap<String, toml::Value>,
) -> BTreeMap<String, String> {
    let mut merged = meta_map
        .get(connect)
        .map(|meta| meta.default_params.clone())
        .unwrap_or_default();

    for (key, value) in params {
        let rendered = render_runtime_value(value);
        if !rendered.is_empty() {
            merged.insert(key.clone(), rendered);
        }
    }

    merged
}

fn is_ignored_wpl_identifier(value: &str) -> bool {
    value.trim().to_ascii_lowercase().starts_with("ignore")
}

fn parse_tag_attributes(raw_tag: &str) -> HashMap<String, String> {
    let mut attributes = HashMap::new();
    let pattern = Regex::new(r#"([a-zA-Z0-9_]+)\s*:\s*"([^"]*)""#).expect("valid tag regex");

    for capture in pattern.captures_iter(raw_tag) {
        let key = capture
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default();
        let value = capture
            .get(2)
            .map(|value| value.as_str())
            .unwrap_or_default();
        if !key.is_empty() {
            attributes.insert(key.to_string(), value.to_string());
        }
    }

    attributes
}

fn extract_tag_payload(raw_annotation: &str) -> &str {
    let pattern = Regex::new(r#"tag\((.*?)\)"#).expect("valid tag wrapper regex");
    pattern
        .captures(raw_annotation)
        .and_then(|capture| capture.get(1).map(|value| value.as_str()))
        .unwrap_or(raw_annotation)
}

fn extract_decl_name(line: &str, keyword: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let remainder = trimmed.strip_prefix(keyword)?.trim_start();
    let name = remainder
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();

    if name.is_empty() { None } else { Some(name) }
}

fn extract_wpl_rule_overview(content: &str, fallback_package: &str) -> Option<IntegrationRuleItem> {
    let mut package_key = fallback_package.trim().to_string();
    let mut package_tag_attributes = HashMap::new();
    let mut grouped = BTreeMap::<String, BTreeSet<String>>::new();
    let mut pending_annotation = String::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("#[") && line.ends_with(']') {
            pending_annotation = line
                .trim_start_matches("#[")
                .trim_end_matches(']')
                .trim()
                .to_string();
            continue;
        }

        if let Some(name) = extract_decl_name(line, "package") {
            package_key = name;
            package_tag_attributes =
                parse_tag_attributes(extract_tag_payload(pending_annotation.as_str()));
            pending_annotation.clear();
            continue;
        }

        if let Some(rule_key) = extract_decl_name(line, "rule") {
            if rule_key.is_empty() || is_ignored_wpl_identifier(&rule_key) {
                pending_annotation.clear();
                continue;
            }

            let rule_tag_attributes =
                parse_tag_attributes(extract_tag_payload(pending_annotation.as_str()));
            let log_type_name = rule_tag_attributes
                .get("log_desc")
                .map(String::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(rule_key.as_str())
                .trim()
                .to_string();

            grouped.entry(log_type_name).or_default().insert(rule_key);
            pending_annotation.clear();
            continue;
        }

        pending_annotation.clear();
    }

    if package_key.is_empty() || is_ignored_wpl_identifier(&package_key) {
        return None;
    }

    let device_type = package_tag_attributes
        .get("dev_name")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            package_tag_attributes
                .get("dev_type")
                .map(String::as_str)
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or(package_key.as_str())
        .trim()
        .to_string();

    let log_types = grouped
        .into_iter()
        .map(|(log_type_name, rule_keys)| {
            let rule_keys = rule_keys.into_iter().collect::<Vec<_>>();
            let key = format!("{log_type_name}-{}", rule_keys.join("|"));

            IntegrationRuleLogType {
                key,
                log_type_name,
                rule_keys,
            }
        })
        .collect::<Vec<_>>();

    Some(IntegrationRuleItem {
        key: package_key,
        device_type,
        log_types,
    })
}

fn infer_connector_type(
    meta: Option<&ConnectorMeta>,
    connect: &str,
    params: &BTreeMap<String, String>,
) -> (String, String) {
    let normalized_connect = connect.trim().to_ascii_lowercase();
    let protocol = preferred_value(params, &["protocol"]).to_ascii_lowercase();
    let mut type_key = meta
        .map(|item| item.type_key.trim().to_ascii_lowercase())
        .unwrap_or_else(|| connect.trim().replace(['_', ' '], "-").to_ascii_lowercase());

    if type_key == "syslog" {
        if protocol == "udp" || normalized_connect.contains("udp") {
            type_key = "syslog-udp".to_string();
        } else if protocol == "tcp" || normalized_connect.contains("tcp") {
            type_key = "syslog-tcp".to_string();
        }
    }

    let type_label = connector_type_display_name(&type_key)
        .map(str::to_string)
        .or_else(|| meta.map(|item| item.type_label.clone()))
        .unwrap_or_else(|| type_key.clone());

    (type_key, type_label)
}

fn build_connector_detail(type_key: &str, params: &BTreeMap<String, String>) -> String {
    match type_key {
        "syslog-udp" | "syslog-tcp" | "tcp" | "udp" => join_detail_segments(&[
            format_prefixed("地址", preferred_value(params, &["addr", "host"])),
            format_prefixed("端口", preferred_value(params, &["port"])),
            format_prefixed("协议", preferred_value(params, &["protocol"])),
        ]),
        "mysql" | "postgres" | "doris" | "clickhouse" => {
            let endpoint = preferred_value(params, &["endpoint", "host"]);
            let (host, port) = split_host_port(endpoint);
            join_detail_segments(&[
                if !host.is_empty() {
                    format!("地址 {host}")
                } else {
                    format_prefixed("地址", endpoint)
                },
                format_prefixed("端口", if !port.is_empty() { &port } else { "" }),
                format_prefixed("数据库", preferred_value(params, &["database"])),
                format_prefixed("数据表", preferred_value(params, &["table"])),
            ])
        }
        "dmdb" => {
            let connection =
                parse_connection_string(preferred_value(params, &["connection_string"]));
            let endpoint = preferred_value(params, &["endpoint"]);
            let (host, port) = split_host_port(endpoint);
            let connection_host = connection
                .get("SERVER")
                .cloned()
                .unwrap_or_else(|| host.clone());
            let connection_port = connection
                .get("TCP_PORT")
                .cloned()
                .unwrap_or_else(|| port.clone());

            join_detail_segments(&[
                if !connection_host.is_empty() {
                    format!("地址 {connection_host}")
                } else {
                    format_prefixed("地址", endpoint)
                },
                format_prefixed("端口", &connection_port),
                format_prefixed("数据库", preferred_value(params, &["database", "schema"])),
                format_prefixed("数据表", preferred_value(params, &["table"])),
            ])
        }
        "file" => {
            let path = join_path_segments(
                preferred_value(params, &["base", "path"]),
                preferred_value(params, &["file", "file_path"]),
            );
            format_prefixed("文件路径", &path)
        }
        "kafka" => join_detail_segments(&[
            format_prefixed("地址", preferred_value(params, &["brokers"])),
            format_prefixed("Topic", preferred_value(params, &["topic"])),
        ]),
        "elasticsearch" => {
            let endpoint = preferred_value(params, &["host", "endpoint"]);
            let (host, endpoint_port) = split_host_port(endpoint);
            let port = preferred_value(params, &["port"]);
            join_detail_segments(&[
                if !host.is_empty() {
                    format!("地址 {host}")
                } else {
                    format_prefixed("地址", endpoint)
                },
                format_prefixed("端口", preferred_non_empty(&[port, &endpoint_port])),
                format_prefixed("索引", preferred_value(params, &["index"])),
            ])
        }
        _ => join_detail_segments(&[
            format_prefixed("地址", preferred_value(params, &["endpoint"])),
            format_prefixed("路径", preferred_value(params, &["api_path"])),
        ]),
    }
}

fn render_runtime_value(value: &toml::Value) -> String {
    match value {
        toml::Value::String(value) => value.trim().to_string(),
        toml::Value::Integer(value) => value.to_string(),
        toml::Value::Float(value) => value.to_string(),
        toml::Value::Boolean(value) => value.to_string(),
        toml::Value::Array(items) => items
            .iter()
            .map(render_runtime_value)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        toml::Value::Datetime(value) => value.to_string(),
        toml::Value::Table(_) => String::new(),
    }
}

fn parse_template_default_value(value: &str) -> String {
    let raw = value.trim();
    if raw.is_empty() {
        return String::new();
    }

    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        return raw[1..raw.len() - 1].to_string();
    }

    if raw.starts_with('[') && raw.ends_with(']') {
        return raw[1..raw.len() - 1]
            .split(',')
            .map(|item| item.trim().trim_matches('"').trim_matches('\''))
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
    }

    raw.to_string()
}

fn parse_connection_string(value: &str) -> HashMap<String, String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .filter_map(|item| {
            let (key, value) = item.split_once('=')?;
            let key = key.trim().to_ascii_uppercase();
            let value = value.trim().to_string();
            (!key.is_empty() && !value.is_empty()).then_some((key, value))
        })
        .collect()
}

fn split_host_port(value: &str) -> (String, String) {
    let sanitized = value
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("")
        .trim();
    if sanitized.is_empty() {
        return (String::new(), String::new());
    }

    let Some(index) = sanitized.rfind(':') else {
        return (sanitized.to_string(), String::new());
    };
    if index == 0 || index == sanitized.len() - 1 {
        return (sanitized.to_string(), String::new());
    }

    (
        sanitized[..index].to_string(),
        sanitized[index + 1..].to_string(),
    )
}

fn preferred_value<'a>(params: &'a BTreeMap<String, String>, keys: &[&str]) -> &'a str {
    for key in keys {
        if let Some(value) = params.get(*key)
            && !value.trim().is_empty()
        {
            return value.as_str();
        }
    }
    ""
}

fn preferred_non_empty<'a>(values: &[&'a str]) -> &'a str {
    values
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .unwrap_or("")
}

fn format_prefixed(prefix: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        String::new()
    } else {
        format!("{prefix} {value}")
    }
}

fn join_detail_segments(items: &[String]) -> String {
    let filtered = items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        "-".to_string()
    } else {
        filtered.join("，")
    }
}

fn join_path_segments(base: &str, file: &str) -> String {
    let base = base.trim();
    let file = file.trim();

    if base.is_empty() {
        return file.to_string();
    }
    if file.is_empty() {
        return base.to_string();
    }
    if file.starts_with('/') {
        return file.to_string();
    }

    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        file.trim_start_matches('/')
    )
}
