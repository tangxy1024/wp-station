//! 配置模板扫描与渲染工具。
//!
//! 模板直接从运行时 `project_infra/connectors` 读取，再按业务规则转换成
//! source / sink 可插入的拓扑配置片段。

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::db::RuleType;
use crate::error::AppError;
use crate::server::ProjectLayout;

#[derive(Debug, Clone)]
pub struct ConfigTemplateField {
    pub name: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub advanced: bool,
}

#[derive(Debug, Clone)]
pub struct ConfigTemplateDef {
    pub scope: RuleType,
    pub template_file: String,
    pub connect: String,
    pub connector_type: String,
    pub default_enabled: Option<bool>,
    pub fields: Vec<ConfigTemplateField>,
}

#[derive(Debug, Clone)]
pub struct RenderedConfigTemplate {
    pub scope: RuleType,
    pub template_id: String,
    pub template_file: String,
    pub display_name: String,
    pub connect: String,
    pub instance_name: String,
    pub required_fields: Vec<String>,
    pub inserted_fields: Vec<String>,
    pub omitted_fields: Vec<String>,
    pub warnings: Vec<String>,
    pub snippet: String,
    pub content: String,
}

#[derive(Debug, Clone)]
struct ConnectorTemplateSource {
    template_file: String,
    connect: String,
    connector_type: String,
    allow_override: Vec<String>,
    params: BTreeMap<String, TomlValue>,
}

#[derive(Debug, Clone)]
enum TomlValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<TomlValue>),
    Table(BTreeMap<String, TomlValue>),
}

/// 返回指定 scope 的配置模板列表。
pub fn list_config_templates(scope: RuleType) -> Result<Vec<ConfigTemplateDef>, AppError> {
    let layout = crate::server::Setting::load().project_layout();
    list_config_templates_from_layout(&layout, scope)
}

/// 根据项目布局扫描 connectors 目录并构建模板。
pub fn list_config_templates_from_layout(
    layout: &ProjectLayout,
    scope: RuleType,
) -> Result<Vec<ConfigTemplateDef>, AppError> {
    let connectors_dir = match scope {
        RuleType::Source => layout.infra_root.join("connectors").join("source.d"),
        RuleType::Sink => layout.infra_root.join("connectors").join("sink.d"),
        _ => {
            return Err(AppError::validation(
                "配置模板 scope 仅支持 source 或 sink".to_string(),
            ));
        }
    };

    let mut items = scan_connector_templates(&connectors_dir)?
        .into_iter()
        .map(|source| convert_connector_template(scope, source))
        .collect::<Vec<_>>();

    items.sort_by(|left, right| {
        left.template_file
            .cmp(&right.template_file)
            .then_with(|| left.connect.cmp(&right.connect))
    });
    Ok(items)
}

/// 渲染配置模板预览与合并结果。
pub fn render_config_template(
    layout: &ProjectLayout,
    scope: RuleType,
    template_id: &str,
    current_content: &str,
) -> Result<RenderedConfigTemplate, AppError> {
    let templates = list_config_templates_from_layout(layout, scope)?;
    let template = templates
        .iter()
        .find(|item| template_id_from_file(&item.template_file) == template_id)
        .cloned()
        .ok_or_else(|| AppError::not_found(format!("未找到配置模板: {template_id}")))?;

    let instance_name = next_unique_instance_name(scope, current_content, template_id);
    let display_name = display_name_from_file(&template.template_file);
    let required_fields = template
        .fields
        .iter()
        .filter(|field| field.required)
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    let inserted_fields = template
        .fields
        .iter()
        .filter(|field| !field.advanced && field.default_value.is_some())
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    let omitted_fields = template
        .fields
        .iter()
        .filter(|field| field.advanced)
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();

    let warnings = Vec::new();
    let snippet = match scope {
        RuleType::Source => render_source_template(&template, &instance_name),
        RuleType::Sink => render_sink_template(&template, &instance_name, current_content),
        _ => {
            return Err(AppError::validation(
                "配置模板 scope 仅支持 source 或 sink".to_string(),
            ));
        }
    };
    let content = merge_template_content(current_content, &snippet);

    Ok(RenderedConfigTemplate {
        scope,
        template_id: template_id.to_string(),
        template_file: template.template_file,
        display_name,
        connect: template.connect,
        instance_name,
        required_fields,
        inserted_fields,
        omitted_fields,
        warnings,
        snippet,
        content,
    })
}

fn scan_connector_templates(dir: &Path) -> Result<Vec<ConnectorTemplateSource>, AppError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(AppError::internal)? {
        let entry = entry.map_err(AppError::internal)?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".toml") {
            continue;
        }

        let content = std::fs::read_to_string(&path).map_err(AppError::internal)?;
        let connector = parse_connector_template(file_name.to_string(), &content)?;
        result.push(connector);
    }

    Ok(result)
}

fn parse_connector_template(
    template_file: String,
    content: &str,
) -> Result<ConnectorTemplateSource, AppError> {
    let id = first_string_assignment(content, "id")
        .ok_or_else(|| AppError::validation(format!("connector 模板缺少 id: {template_file}")))?;
    let connector_type = first_string_assignment(content, "type")
        .ok_or_else(|| AppError::validation(format!("connector 模板缺少 type: {template_file}")))?;

    let allow_override = parse_allow_override(content);
    let params = parse_connector_params(content);

    Ok(ConnectorTemplateSource {
        template_file,
        connect: id,
        connector_type,
        allow_override,
        params,
    })
}

fn convert_connector_template(
    scope: RuleType,
    source: ConnectorTemplateSource,
) -> ConfigTemplateDef {
    let default_enabled = matches!(
        template_id_from_file(&source.template_file).as_str(),
        "file-default"
    )
    .then_some(true)
    .or(Some(false))
    .filter(|_| matches!(scope, RuleType::Source));

    let fields = source
        .allow_override
        .into_iter()
        .map(|name| {
            let required = is_required_field(scope, &source.connector_type, &name);
            let advanced = is_advanced_field(scope, &name);
            let default_value = if advanced {
                None
            } else {
                source.params.get(&name).map(render_toml_value)
            };

            ConfigTemplateField {
                name,
                required,
                default_value,
                advanced,
            }
        })
        .collect::<Vec<_>>();

    ConfigTemplateDef {
        scope,
        template_file: source.template_file,
        connect: source.connect,
        connector_type: source.connector_type,
        default_enabled,
        fields,
    }
}

fn is_required_field(scope: RuleType, connector_type: &str, field_name: &str) -> bool {
    match scope {
        RuleType::Source => match field_name {
            "base" | "file" | "addr" | "port" | "brokers" | "topic" | "endpoint" | "username"
            | "password" | "database" | "table" | "connection_string" | "driver" | "dsn"
            | "cursor_column" => true,
            _ => matches!(
                (connector_type, field_name),
                ("doris", "user") | ("elasticsearch", "host") | ("victoriametrics", "insert_url")
            ),
        },
        RuleType::Sink => match field_name {
            "base" | "file" | "addr" | "port" | "brokers" | "topic" | "endpoint" | "username"
            | "database" | "table" | "host" | "index" | "insert_url" => true,
            _ => matches!(
                (connector_type, field_name),
                ("doris", "user")
                    | ("mysql", "password")
                    | ("postgres", "password")
                    | ("clickhouse", "database")
                    | ("clickhouse", "table")
            ),
        },
        _ => false,
    }
}

fn is_advanced_field(scope: RuleType, field_name: &str) -> bool {
    let common = matches!(
        field_name,
        "batch"
            | "batch_size"
            | "poll_interval_ms"
            | "error_backoff_ms"
            | "connect_timeout_secs"
            | "query_timeout_secs"
            | "timeout_secs"
            | "max_retries"
            | "flush_interval_secs"
            | "udp_recv_buffer"
            | "tcp_recv_bytes"
            | "instances"
            | "headers"
            | "sync"
            | "max_backoff"
            | "attach_meta_tags"
            | "strip_header"
            | "create_time_field"
    );

    if common {
        return true;
    }

    matches!(scope, RuleType::Sink) && matches!(field_name, "num_partitions" | "replication")
}

fn render_source_template(template: &ConfigTemplateDef, instance_name: &str) -> String {
    let mut lines = vec![
        "[[sources]]".to_string(),
        format!("key = \"{instance_name}\""),
        format!("enable = {}", template.default_enabled.unwrap_or(false)),
        format!("connect = \"{}\"", template.connect),
        "tags = []".to_string(),
    ];

    let params = render_template_fields(&template.fields);
    if !params.is_empty() {
        lines.push(String::new());
        lines.push("[sources.params]".to_string());
        lines.extend(params);
    }

    lines.join("\n")
}

fn render_sink_template(
    template: &ConfigTemplateDef,
    instance_name: &str,
    current_content: &str,
) -> String {
    let mut lines = Vec::new();

    if !has_named_section(current_content, "[sink_group]") {
        if !has_assignment(current_content, "version") {
            lines.push(r#"version = "1.0""#.to_string());
            lines.push(String::new());
        }

        lines.extend([
            "[sink_group]".to_string(),
            r#"name = "all""#.to_string(),
            r#"oml = ["*"]"#.to_string(),
            "parallel = 1".to_string(),
            String::new(),
        ]);
    }

    lines.push("[[sink_group.sinks]]".to_string());
    lines.push(format!("name = \"{instance_name}\""));
    lines.push(format!("connect = \"{}\"", template.connect));
    lines.push("tags = []".to_string());

    let params = render_template_fields(&template.fields);
    if !params.is_empty() {
        lines.push(String::new());
        lines.push("[sink_group.sinks.params]".to_string());
        lines.extend(params);
    }

    lines.join("\n")
}

fn render_template_fields(fields: &[ConfigTemplateField]) -> Vec<String> {
    fields
        .iter()
        .filter(|field| !field.advanced)
        .filter_map(|field| {
            field
                .default_value
                .as_ref()
                .map(|value| format!("{} = {}", field.name, value))
        })
        .collect()
}

fn parse_allow_override(content: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut in_allow_override = false;
    let mut buffer = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !in_allow_override {
            if let Some((lhs, rhs)) = trimmed.split_once('=')
                && lhs.trim() == "allow_override"
            {
                in_allow_override = true;
                buffer.push_str(rhs.trim());
                if rhs.contains(']') {
                    break;
                }
            }
            continue;
        }

        buffer.push(' ');
        buffer.push_str(trimmed);
        if trimmed.contains(']') {
            break;
        }
    }

    if buffer.is_empty() {
        return values;
    }

    let start = buffer.find('[').unwrap_or(0);
    let end = buffer.rfind(']').unwrap_or(buffer.len());
    let inner = &buffer[start + 1..end];
    for item in inner.split(',') {
        let value = item.trim().trim_matches('"').trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }
    }

    values
}

fn parse_connector_params(content: &str) -> BTreeMap<String, TomlValue> {
    let mut params = BTreeMap::new();
    let mut section_stack: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = strip_inline_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("[[") && trimmed.ends_with("]]") {
            section_stack.clear();
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed.trim_start_matches('[').trim_end_matches(']');
            section_stack = section
                .split('.')
                .map(|part| part.trim().to_string())
                .collect();
            continue;
        }

        if !matches!(
            section_stack.as_slice(),
            [head, tail @ ..] if head == "connectors" && tail.first().map(|s| s.as_str()) == Some("params")
        ) {
            continue;
        }

        let Some((lhs, rhs)) = trimmed.split_once('=') else {
            continue;
        };
        let key = lhs.trim();
        if key.is_empty() {
            continue;
        }

        let value = parse_toml_scalar_or_array(rhs.trim());
        let nested_path = &section_stack[2..];
        if nested_path.is_empty() {
            params.insert(key.to_string(), value);
        } else {
            let root = params
                .entry(nested_path[0].clone())
                .or_insert_with(|| TomlValue::Table(BTreeMap::new()));
            insert_nested_table_value(root, &nested_path[1..], key, value);
        }
    }

    params
}

fn insert_nested_table_value(
    root: &mut TomlValue,
    nested_path: &[String],
    key: &str,
    value: TomlValue,
) {
    let TomlValue::Table(table) = root else {
        return;
    };

    if nested_path.is_empty() {
        table.insert(key.to_string(), value);
        return;
    }

    let child = table
        .entry(nested_path[0].clone())
        .or_insert_with(|| TomlValue::Table(BTreeMap::new()));
    insert_nested_table_value(child, &nested_path[1..], key, value);
}

fn parse_toml_scalar_or_array(raw: &str) -> TomlValue {
    let value = raw.trim();
    if value.starts_with('[') && value.ends_with(']') {
        let inner = &value[1..value.len() - 1];
        let items = inner
            .split(',')
            .filter_map(|item| {
                let trimmed = item.trim();
                (!trimmed.is_empty()).then(|| parse_toml_scalar_or_array(trimmed))
            })
            .collect::<Vec<_>>();
        return TomlValue::Array(items);
    }

    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        return TomlValue::String(value[1..value.len() - 1].to_string());
    }

    if matches!(value, "true" | "false") {
        return TomlValue::Boolean(value == "true");
    }

    if let Ok(number) = value.replace('_', "").parse::<i64>() {
        return TomlValue::Integer(number);
    }

    if let Ok(number) = value.replace('_', "").parse::<f64>() {
        return TomlValue::Float(number);
    }

    TomlValue::String(value.to_string())
}

fn render_toml_value(value: &TomlValue) -> String {
    match value {
        TomlValue::String(value) => format!("\"{value}\""),
        TomlValue::Integer(value) => value.to_string(),
        TomlValue::Float(value) => value.to_string(),
        TomlValue::Boolean(value) => value.to_string(),
        TomlValue::Array(items) => {
            let values = items.iter().map(render_toml_value).collect::<Vec<_>>();
            format!("[{}]", values.join(", "))
        }
        TomlValue::Table(_) => "{}".to_string(),
    }
}

fn strip_inline_comment(line: &str) -> &str {
    let mut in_string = false;
    for (index, ch) in line.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '#' if !in_string => return &line[..index],
            _ => {}
        }
    }
    line
}

fn first_string_assignment(content: &str, key: &str) -> Option<String> {
    content
        .lines()
        .filter_map(|line| parse_string_assignment(line, key))
        .next()
}

fn next_unique_instance_name(scope: RuleType, current_content: &str, template_id: &str) -> String {
    let normalized_suffix = template_id.replace('-', "_");
    let base = match scope {
        RuleType::Source => format!("gen_{normalized_suffix}"),
        RuleType::Sink => format!("all_{normalized_suffix}"),
        _ => normalized_suffix,
    };
    let key_name = if matches!(scope, RuleType::Source) {
        "key"
    } else {
        "name"
    };

    let existing = extract_string_assignments(current_content, key_name)
        .into_iter()
        .collect::<HashSet<_>>();

    if !existing.contains(&base) {
        return base;
    }

    for index in 2.. {
        let candidate = format!("{base}_{index}");
        if !existing.contains(&candidate) {
            return candidate;
        }
    }

    base
}

fn extract_string_assignments(content: &str, key: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| parse_string_assignment(line, key))
        .collect()
}

fn parse_string_assignment(line: &str, key: &str) -> Option<String> {
    let trimmed = strip_inline_comment(line).trim();
    let (lhs, rhs) = trimmed.split_once('=')?;
    if lhs.trim() != key {
        return None;
    }

    let rhs = rhs.trim();
    if !rhs.starts_with('"') {
        return None;
    }

    let rhs = &rhs[1..];
    let end = rhs.find('"')?;
    Some(rhs[..end].to_string())
}

fn has_named_section(content: &str, section: &str) -> bool {
    content.lines().any(|line| line.trim() == section)
}

fn has_assignment(content: &str, key: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = strip_inline_comment(line).trim();
        let Some((lhs, _)) = trimmed.split_once('=') else {
            return false;
        };
        lhs.trim() == key
    })
}

fn merge_template_content(current_content: &str, snippet: &str) -> String {
    let trimmed = current_content.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() {
        return format!("{snippet}\n");
    }

    format!("{trimmed}\n\n{snippet}\n")
}

pub fn template_id_from_file(template_file: &str) -> String {
    strip_numeric_prefix(template_file)
        .trim_end_matches(".toml")
        .to_string()
}

pub fn display_name_from_file(template_file: &str) -> String {
    strip_numeric_prefix(template_file).to_string()
}

fn strip_numeric_prefix(template_file: &str) -> &str {
    let Some((prefix, rest)) = template_file.split_once('-') else {
        return template_file;
    };

    if prefix.chars().all(|ch| ch.is_ascii_digit()) {
        rest
    } else {
        template_file
    }
}

#[allow(dead_code)]
fn _connector_dir(layout: &ProjectLayout, scope: RuleType) -> PathBuf {
    match scope {
        RuleType::Source => layout.infra_root.join("connectors").join("source.d"),
        RuleType::Sink => layout.infra_root.join("connectors").join("sink.d"),
        _ => layout.infra_root.join("connectors"),
    }
}
