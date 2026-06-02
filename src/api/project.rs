use actix_web::{HttpRequest, HttpResponse, get, http::header, post, web};
use futures_util::StreamExt;
use urlencoding::decode;

use crate::error::AppError;
use crate::server::project::{
    ProjectArchiveConfirmRequest, ProjectImportRequest, confirm_project_archive_import_logic,
    export_project_archive_logic, import_project_from_files_logic, preview_project_archive_logic,
};

const MAX_ARCHIVE_BYTES: usize = 200 * 1024 * 1024;

fn operator_from_request(req: &HttpRequest) -> Option<String> {
    req.headers().get("x-operator").and_then(|value| {
        let raw = value.to_str().ok()?.trim();
        if raw.is_empty() {
            return None;
        }
        decode(raw)
            .ok()
            .map(|cow| cow.trim().to_string())
            .filter(|decoded| !decoded.is_empty())
    })
}

fn archive_file_name(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("x-file-name")
        .and_then(|value| value.to_str().ok())
        .and_then(|raw| decode(raw).ok())
        .map(|cow| cow.trim().to_string())
        .filter(|name| !name.is_empty())
}

#[post("/api/project/import")]
pub async fn import_project_from_files(
    http_req: HttpRequest,
    req: web::Json<ProjectImportRequest>,
) -> Result<HttpResponse, AppError> {
    let operator = operator_from_request(&http_req);
    let resp = import_project_from_files_logic(operator, req.into_inner()).await?;
    Ok(HttpResponse::Ok().json(resp))
}

#[post("/api/project/import/archive")]
pub async fn import_project_archive(
    http_req: HttpRequest,
    mut payload: web::Payload,
) -> Result<HttpResponse, AppError> {
    let operator = operator_from_request(&http_req);
    let file_name = archive_file_name(&http_req)
        .ok_or_else(|| AppError::validation("缺少上传文件名，请设置 X-File-Name"))?;
    let mut bytes = web::BytesMut::new();

    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|e| AppError::validation(format!("读取上传内容失败: {}", e)))?;
        if bytes.len() + chunk.len() > MAX_ARCHIVE_BYTES {
            return Err(AppError::validation("上传文件超过 200MB 限制"));
        }
        bytes.extend_from_slice(&chunk);
    }

    let resp = preview_project_archive_logic(operator, &file_name, bytes.freeze().to_vec()).await?;
    Ok(HttpResponse::Ok().json(resp))
}

#[post("/api/project/import/archive/confirm")]
pub async fn confirm_project_archive_import(
    http_req: HttpRequest,
    req: web::Json<ProjectArchiveConfirmRequest>,
) -> Result<HttpResponse, AppError> {
    let operator = operator_from_request(&http_req);
    let req = req.into_inner();
    let resp = confirm_project_archive_import_logic(operator, &req.import_id).await?;
    Ok(HttpResponse::Ok().json(resp))
}

#[get("/api/project/export/archive")]
pub async fn export_project_archive() -> Result<HttpResponse, AppError> {
    let archive = export_project_archive_logic().await?;
    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "application/gzip"))
        .insert_header((
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", archive.file_name),
        ))
        .body(archive.bytes))
}
