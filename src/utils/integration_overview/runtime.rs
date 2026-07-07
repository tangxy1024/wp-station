//! 接入概览中的运行时输入源 / 输出源扫描逻辑。

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::constants::project::{DIR_BUSINESS_D, DIR_SINKS, DIR_SOURCES, DIR_TOPOLOGY, FILE_WPSRC};
use crate::db::RuleType;
use crate::error::AppError;
use crate::server::RepoLayout;
use crate::utils::config_templates::{display_name_from_file, list_config_templates_from_layout};

use super::detail::{
    ConnectorMeta, build_connector_detail, build_effective_params, infer_connector_type,
    parse_template_default_value,
};
use super::{IntegrationRuntimeItem, IntegrationRuntimeOverview};

/// 输入源拓扑文件结构。
#[derive(Debug, Default, Deserialize)]
struct SourceTopologyFile {
    #[serde(default)]
    sources: Vec<SourceTopologyItem>,
}

/// 单个输入源拓扑项。
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

/// 输出拓扑文件结构。
#[derive(Debug, Default, Deserialize)]
struct SinkTopologyFile {
    #[serde(default)]
    sink_group: SinkGroupTopology,
}

/// 输出分组拓扑结构。
#[derive(Debug, Default, Deserialize)]
struct SinkGroupTopology {
    #[serde(default)]
    sinks: Vec<SinkTopologyItem>,
}

/// 单个输出拓扑项。
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
    layout: &RepoLayout,
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

/// 从 connector 模板中构造 connect -> 类型与默认参数映射。
fn build_connector_meta_map(
    layout: &RepoLayout,
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

        let type_label =
            crate::utils::display::connector_type_display_name(&template.connector_type)
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

/// 汇总 `wpsrc.toml` 中启用的输入源摘要。
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

/// 汇总 `topology/sinks/business.d/*.toml` 中的业务输出摘要。
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

/// 解析 TOML 文件；文件不存在时返回默认值。
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

/// 从候选值中选择第一个非空值。
fn preferred_non_empty<'a>(values: &[&'a str]) -> &'a str {
    values
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .unwrap_or("")
}
