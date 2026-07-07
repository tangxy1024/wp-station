//! 操作日志 API。
//!
//! 提供操作日志列表查询入口。

use actix_web::{HttpResponse, get, web};

use crate::error::AppError;
use crate::server::{LogListQuery, list_logs_logic};

#[get("/api/operation-logs")]
/// 操作日志：获取分页列表。
pub async fn list_operation_logs(
    query: web::Query<LogListQuery>,
) -> Result<HttpResponse, AppError> {
    let resp = list_logs_logic(query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(resp))
}
