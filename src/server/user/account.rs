//! 用户账号管理相关业务。

use crate::db;
use crate::error::AppError;
use crate::server::{
    OperationLogAction, OperationLogBiz, OperationLogParams, write_operation_log_for_result,
};

use super::{
    CreateUserRequest, ResetPasswordResponse, UpdateUserRequest, UpdateUserStatusRequest,
    UserCreated, UserListQuery, UserListResponse, generate_strong_password, hash_password,
};

/// 获取用户列表（支持关键字/角色/状态筛选 + 分页）。
pub async fn list_users_logic(query: UserListQuery) -> Result<UserListResponse, AppError> {
    debug!(
        "获取用户列表: keyword={:?}, role={:?}, status={:?}",
        query.keyword, query.role, query.status
    );

    let (page, page_size) = query.page.normalize_default();

    let (items, total) = db::find_users_page(
        query.keyword.as_deref(),
        query.role.as_deref(),
        query.status.as_deref(),
        page,
        page_size,
    )
    .await?;

    debug!(
        "获取用户列表成功: 共 {} 条, page={}, page_size={}",
        total, page, page_size
    );

    Ok(UserListResponse::from_db(items, total, page, page_size))
}

/// 创建用户。
pub async fn create_user_logic(req: CreateUserRequest) -> Result<UserCreated, AppError> {
    info!("创建用户: username={}", req.username);

    let username = req.username.clone();
    let role = req.role.clone();
    let email = req.email.clone();
    let display_name = req.display_name.clone();

    let result = async move {
        if let Some(_existing) = db::find_user_by_username(&req.username).await? {
            return Err(AppError::validation(format!(
                "用户名 {} 已存在",
                req.username
            )));
        }

        let password_hash = hash_password(&req.password)?;

        let new_user = db::NewUser {
            username: req.username.clone(),
            password: password_hash,
            display_name: req.display_name,
            email: req.email,
            role: req.role,
            remark: req.remark,
        };

        let id = db::create_user(new_user).await?;
        info!("用户创建成功: id={}, username={}", id, req.username);

        Ok::<_, AppError>(UserCreated { id })
    }
    .await;

    write_operation_log_for_result(
        OperationLogBiz::User,
        OperationLogAction::Create,
        OperationLogParams::new()
            .with_target_name(username)
            .with_field("role", role)
            .with_field("email", email.unwrap_or_else(|| "-".to_string()))
            .with_field(
                "display_name",
                display_name.unwrap_or_else(|| "-".to_string()),
            ),
        &result,
    )
    .await;

    result
}

/// 编辑用户基本信息。
pub async fn update_user_logic(id: i32, req: UpdateUserRequest) -> Result<(), AppError> {
    info!("更新用户: id={}", id);

    let display_name = req.display_name.clone();
    let email = req.email.clone();
    let role = req.role.clone();
    let remark = req.remark.clone();

    let result = async move {
        let update_data = db::UpdateUser {
            display_name: req.display_name,
            email: req.email,
            role: req.role,
            remark: req.remark,
        };

        db::update_user(id, update_data).await?;
        info!("更新用户成功: id={}", id);

        Ok::<_, AppError>(())
    }
    .await;

    write_operation_log_for_result(
        OperationLogBiz::User,
        OperationLogAction::Update,
        OperationLogParams::new()
            .with_target_id(id.to_string())
            .with_field(
                "display_name",
                display_name.unwrap_or_else(|| "-".to_string()),
            )
            .with_field("email", email.unwrap_or_else(|| "-".to_string()))
            .with_field("role", role.unwrap_or_else(|| "-".to_string()))
            .with_field(
                "remark",
                remark
                    .and_then(|item| item)
                    .unwrap_or_else(|| "-".to_string()),
            ),
        &result,
    )
    .await;

    result
}

/// 更新用户状态（启用 / 禁用）。
pub async fn update_user_status_logic(
    id: i32,
    req: UpdateUserStatusRequest,
) -> Result<(), AppError> {
    info!("更新用户状态: id={}, status={}", id, req.status);

    let status = req.status.clone();
    let result = async {
        if req.status != "active" && req.status != "inactive" {
            return Err(AppError::validation("状态值必须是 active 或 inactive"));
        }

        db::update_user_status(id, &req.status).await?;
        info!("更新用户状态成功: id={}, status={}", id, req.status);

        Ok::<_, AppError>(())
    }
    .await;

    write_operation_log_for_result(
        OperationLogBiz::User,
        OperationLogAction::Update,
        OperationLogParams::new()
            .with_target_id(id.to_string())
            .with_field("status", status),
        &result,
    )
    .await;

    result
}

/// 重置用户密码（生成随机强密码）。
pub async fn reset_password_logic(
    id: i32,
    _req: super::ResetPasswordRequest,
) -> Result<ResetPasswordResponse, AppError> {
    info!("重置用户密码: id={}", id);

    let result = async {
        let new_password = generate_strong_password();
        let password_hash = hash_password(&new_password)?;

        db::reset_user_password(id, password_hash).await?;
        info!("重置用户密码成功: id={}", id);

        Ok::<_, AppError>(ResetPasswordResponse { new_password })
    }
    .await;

    write_operation_log_for_result(
        OperationLogBiz::User,
        OperationLogAction::ResetPassword,
        OperationLogParams::new()
            .with_target_id(id.to_string())
            .with_field("mode", "admin-reset"),
        &result,
    )
    .await;

    result
}

/// 删除用户（软删除）。
pub async fn delete_user_logic(id: i32) -> Result<(), AppError> {
    info!("删除用户: id={}", id);

    let result = async {
        db::delete_user(id).await?;
        info!("删除用户成功: id={}", id);
        Ok::<_, AppError>(())
    }
    .await;

    write_operation_log_for_result(
        OperationLogBiz::User,
        OperationLogAction::Delete,
        OperationLogParams::new()
            .with_target_id(id.to_string())
            .with_field("delete_mode", "soft"),
        &result,
    )
    .await;

    result
}
