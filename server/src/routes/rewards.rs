use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthUser, StudentUser};
use crate::dto::{OrderDto, RewardDto};
use crate::error::{AppError, AppResult};
use crate::models::{Order, Reward, Role, User};
use crate::rows::{OrderRow, ORDER_SELECT};
use crate::state::{AppState, AuditRecord};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rewards", get(list_rewards))
        .route("/rewards/{id}/buy", post(buy_reward))
        .route("/rewards/{id}/image", get(reward_image))
}

pub fn reward_dto(r: Reward) -> RewardDto {
    RewardDto {
        id: r.id,
        title: r.title,
        description: r.description,
        price_minutes: r.price_minutes,
        stock: r.stock,
        is_active: r.is_active,
        image_url: r.image_path.map(|_| format!("/api/rewards/{}/image", r.id)),
        created_at: r.created_at,
    }
}

async fn list_rewards(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<RewardDto>>> {
    // students see only active rewards; staff see all
    let rows = if auth.user.role == Role::Student {
        sqlx::query_as::<_, Reward>(
            "SELECT * FROM rewards WHERE is_active ORDER BY price_minutes, title",
        )
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, Reward>("SELECT * FROM rewards ORDER BY created_at DESC")
            .fetch_all(&state.pool)
            .await?
    };
    Ok(Json(rows.into_iter().map(reward_dto).collect()))
}

async fn buy_reward(
    State(state): State<AppState>,
    auth: StudentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OrderDto>> {
    let mut tx = state.pool.begin().await?;
    let reward = sqlx::query_as::<_, Reward>("SELECT * FROM rewards WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::not_found("Награда не найдена"))?;
    if !reward.is_active {
        return Err(AppError::bad_request("inactive", "Награда недоступна"));
    }
    if matches!(reward.stock, Some(0)) {
        return Err(AppError::conflict("out_of_stock", "Награда закончилась"));
    }
    // lock the student row; only balance is spendable, never debt
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 FOR UPDATE")
        .bind(auth.user.id)
        .fetch_one(&mut *tx)
        .await?;
    if user.balance_minutes < reward.price_minutes {
        return Err(AppError::bad_request(
            "insufficient_balance",
            "Недостаточно минут на балансе",
        ));
    }
    sqlx::query("UPDATE users SET balance_minutes = balance_minutes - $1 WHERE id = $2")
        .bind(reward.price_minutes)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    if reward.stock.is_some() {
        sqlx::query("UPDATE rewards SET stock = stock - 1 WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let order = sqlx::query_as::<_, Order>(
        "INSERT INTO orders (user_id, reward_id, price_minutes, status)
         VALUES ($1,$2,$3,'pending') RETURNING *",
    )
    .bind(user.id)
    .bind(id)
    .bind(reward.price_minutes)
    .fetch_one(&mut *tx)
    .await?;

    state
        .audit(
            &mut *tx,
            AuditRecord::new("order_buy")
                .actor(user.id, "student")
                .entity("order", order.id)
                .details(json!({"reward_id": id, "price_minutes": reward.price_minutes})),
        )
        .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, OrderRow>(sqlx::AssertSqlSafe(format!("{ORDER_SELECT} WHERE o.id = $1")))
        .bind(order.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

async fn reward_image(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Response> {
    // public endpoint: reward images are not sensitive
    let path: Option<String> = sqlx::query_scalar("SELECT image_path FROM rewards WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .flatten();
    let rel = path.ok_or_else(|| AppError::not_found("Изображение отсутствует"))?;
    if rel.contains('/') || rel.contains("..") {
        return Err(AppError::internal("bad image path"));
    }
    let bytes = tokio::fs::read(state.config.photo_dir.join("rewards").join(&rel))
        .await
        .map_err(|_| AppError::not_found("Изображение не найдено"))?;
    let mime = match rel.rsplit('.').next().unwrap_or("") {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
    Ok((StatusCode::OK, headers, bytes).into_response())
}
