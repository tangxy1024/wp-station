// 配置模板业务逻辑层

use crate::db::RuleType;
use crate::error::AppError;
use crate::server::ProjectLayout;
use crate::server::Setting;
use crate::utils::{
    display_name_from_file, list_config_templates_from_layout, render_config_template,
    template_id_from_file,
};
use serde::{Deserialize, Serialize};

// ============ 请求参数结构体 ============

#[derive(Deserialize)]
pub struct ConfigTemplateQuery {
    pub scope: RuleType,
}

#[derive(Deserialize)]
pub struct RenderConfigTemplateRequest {
    pub scope: RuleType,
    pub template_id: String,
    pub content: String,
}

// ============ 响应结构体 ============

#[derive(Serialize)]
pub struct ConfigTemplateFieldItem {
    pub name: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub advanced: bool,
}

#[derive(Serialize)]
pub struct ConfigTemplateItem {
    pub scope: RuleType,
    pub template_file: String,
    pub template_id: String,
    pub display_name: String,
    pub connect: String,
    pub required_fields: Vec<String>,
    pub inserted_fields: Vec<String>,
    pub omitted_fields: Vec<String>,
    pub fields: Vec<ConfigTemplateFieldItem>,
}

#[derive(Serialize)]
pub struct ConfigTemplateListResponse {
    pub items: Vec<ConfigTemplateItem>,
}

#[derive(Serialize)]
pub struct RenderConfigTemplateResponse {
    pub scope: RuleType,
    pub template_file: String,
    pub template_id: String,
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

fn project_layout() -> ProjectLayout {
    Setting::load().project_layout()
}

/// 获取来源 / 输出配置模板列表
pub async fn get_config_templates_logic(
    scope: RuleType,
) -> Result<ConfigTemplateListResponse, AppError> {
    let layout = project_layout();
    let items = list_config_templates_from_layout(&layout, scope)?
        .into_iter()
        .map(|template| {
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
            let fields = template
                .fields
                .into_iter()
                .map(|field| ConfigTemplateFieldItem {
                    name: field.name,
                    required: field.required,
                    default_value: field.default_value,
                    advanced: field.advanced,
                })
                .collect::<Vec<_>>();

            ConfigTemplateItem {
                scope: template.scope,
                template_file: template.template_file.clone(),
                template_id: template_id_from_file(&template.template_file),
                display_name: display_name_from_file(&template.template_file),
                connect: template.connect,
                required_fields,
                inserted_fields,
                omitted_fields,
                fields,
            }
        })
        .collect();

    Ok(ConfigTemplateListResponse { items })
}

/// 渲染来源 / 输出配置模板片段
pub async fn render_config_template_logic(
    scope: RuleType,
    template_id: String,
    content: String,
) -> Result<RenderConfigTemplateResponse, AppError> {
    let layout = project_layout();
    let rendered = render_config_template(&layout, scope, &template_id, &content)?;

    Ok(RenderConfigTemplateResponse {
        scope: rendered.scope,
        template_file: rendered.template_file,
        template_id: rendered.template_id,
        display_name: rendered.display_name,
        connect: rendered.connect,
        instance_name: rendered.instance_name,
        required_fields: rendered.required_fields,
        inserted_fields: rendered.inserted_fields,
        omitted_fields: rendered.omitted_fields,
        warnings: rendered.warnings,
        snippet: rendered.snippet,
        content: rendered.content,
    })
}
