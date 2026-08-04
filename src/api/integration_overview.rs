use actix_web::{HttpResponse, get};

use crate::error::AppError;
use crate::server::{get_integration_rule_overview_logic, get_integration_runtime_overview_logic};

/// 接入概览：返回规则侧设备类型与日志类型摘要。
#[get("/api/integration-overview/rules")]
pub async fn get_integration_rule_overview() -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().json(get_integration_rule_overview_logic()?))
}

/// 接入概览：返回已启用输入源与业务输出源摘要。
#[get("/api/integration-overview/runtime")]
pub async fn get_integration_runtime_overview() -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().json(get_integration_runtime_overview_logic()?))
}
