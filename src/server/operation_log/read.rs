//! 操作日志查询逻辑。

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

use crate::db::find_logs_page;
use crate::error::AppError;

use super::{LogListQuery, LogListResponse};

/// 将 `YYYY-MM-DD` 字符串解析为当天起始时刻（UTC 00:00:00）。
fn parse_start(date_str: &str) -> Result<DateTime<Utc>, AppError> {
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| AppError::validation(format!("无效的开始日期: {}", date_str)))?;
    let datetime = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::validation(format!("无效的开始日期: {}", date_str)))?;
    Ok(Utc.from_utc_datetime(&datetime))
}

/// 将 `YYYY-MM-DD` 字符串解析为当天结束时刻（UTC 23:59:59）。
fn parse_end(date_str: &str) -> Result<DateTime<Utc>, AppError> {
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| AppError::validation(format!("无效的结束日期: {}", date_str)))?;
    let datetime = date
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| AppError::validation(format!("无效的结束日期: {}", date_str)))?;
    Ok(Utc.from_utc_datetime(&datetime))
}

/// 获取操作日志分页列表。
pub async fn list_logs_logic(query: LogListQuery) -> Result<LogListResponse, AppError> {
    let (page, page_size) = query.page.normalize_default();

    let start_date = match query.start_date.as_deref() {
        Some(date) => Some(parse_start(date)?),
        None => None,
    };
    let end_date = match query.end_date.as_deref() {
        Some(date) => Some(parse_end(date)?),
        None => None,
    };

    if let (Some(start), Some(end)) = (start_date.as_ref(), end_date.as_ref())
        && start > end
    {
        return Err(AppError::validation("开始日期不能晚于结束日期"));
    }

    let (items, total) = find_logs_page(
        query.operator.as_deref(),
        query.operation.as_deref(),
        start_date,
        end_date,
        page,
        page_size,
    )
    .await?;

    Ok(LogListResponse::from_db(items, total, page, page_size))
}
