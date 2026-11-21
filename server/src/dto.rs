use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::models::{
    Difficulty, Group, OrderStatus, PhotoReview, Role, SessionStatus, User,
};

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct UserDto {
    pub id: Uuid,
    pub role: Role,
    pub login: String,
    pub full_name: String,
    pub group_id: Option<Uuid>,
    pub group_name: Option<String>,
    pub debt_minutes: i64,
    pub balance_minutes: i64,
    pub is_active: bool,
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
}

impl UserDto {
    pub fn of(user: User, group_name: Option<String>) -> Self {
        UserDto {
            id: user.id,
            role: user.role,
            login: user.login,
            full_name: user.full_name,
            group_id: user.group_id,
            group_name,
            debt_minutes: user.debt_minutes,
            balance_minutes: user.balance_minutes,
            is_active: user.is_active,
            created_at: user.created_at,
        }
    }
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct GroupDto {
    pub id: Uuid,
    pub name: String,
    // active students in the group
    pub student_count: i64,
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
}

impl GroupDto {
    pub fn of(g: Group, student_count: i64) -> Self {
        GroupDto {
            id: g.id,
            name: g.name,
            student_count,
            created_at: g.created_at,
        }
    }
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct LoginRequest {
    pub login: String,
    pub password: String,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct LoginResponse {
    pub token: String,
    pub user: UserDto,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct PushTokenRequest {
    pub token: String,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct QrOpenRequest {
    pub difficulty: Difficulty,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct QrOpenResponse {
    pub qr: String,
    #[ts(type = "string")]
    pub expires_at: DateTime<Utc>,
    pub difficulty: Difficulty,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct FinishQrResponse {
    pub qr: String,
    #[ts(type = "string")]
    pub expires_at: DateTime<Utc>,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct SessionStartRequest {
    pub qr: String,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct SessionDto {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: Option<String>,
    pub group_name: Option<String>,
    pub komendant_id: Uuid,
    pub komendant_name: Option<String>,
    pub difficulty: Difficulty,
    pub coefficient: i32,
    #[ts(type = "string")]
    pub started_at: DateTime<Utc>,
    #[ts(type = "string | null")]
    pub ended_at: Option<DateTime<Utc>>,
    pub status: SessionStatus,
    pub raw_seconds: Option<i64>,
    pub rounded_minutes: Option<i64>,
    pub credited_minutes: Option<i64>,
    pub applied_to_debt: Option<i64>,
    pub applied_to_balance: Option<i64>,
    pub photo_review: Option<PhotoReview>,
    pub photo_review_comment: Option<String>,
    #[ts(type = "string | null")]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub rejected_reason: Option<String>,
    pub has_photo: bool,
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct RejectSessionRequest {
    pub reason: String,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct PhotoReviewRequest {
    pub approved: bool,
    pub comment: Option<String>,
}

// Admin decision on a session stuck in `pending_confirmation`
// (komendant deactivated / unavailable): confirm = credit as the komendant would.
#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct AdminResolveSessionRequest {
    pub confirm: bool,
    pub reason: Option<String>,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct AssignmentCreateRequest {
    pub minutes: i64,
    // optional: teachers assign hours without a reason
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct AssignmentDto {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: Option<String>,
    pub assigned_by: Uuid,
    pub assigned_by_name: Option<String>,
    pub minutes: i64,
    pub reason: String,
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct RewardDto {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub price_minutes: i64,
    pub stock: Option<i64>,
    pub is_active: bool,
    pub image_url: Option<String>,
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct RewardUpsertRequest {
    pub title: String,
    pub description: Option<String>,
    pub price_minutes: i64,
    pub stock: Option<i64>,
    pub is_active: Option<bool>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct OrderDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_name: Option<String>,
    pub reward_id: Uuid,
    pub reward_title: Option<String>,
    pub price_minutes: i64,
    pub status: OrderStatus,
    pub issued_by: Option<Uuid>,
    #[ts(type = "string | null")]
    pub issued_at: Option<DateTime<Utc>>,
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct UserCreateRequest {
    pub role: Role,
    pub login: String,
    pub password: Option<String>,
    pub full_name: String,
    pub group_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct UserUpdateRequest {
    pub role: Option<Role>,
    pub login: Option<String>,
    pub full_name: Option<String>,
    // absent = keep, `null` = remove from group, uuid = move to group
    #[serde(default, deserialize_with = "double_option", skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub group_id: Option<Option<Uuid>>,
    pub is_active: Option<bool>,
}

/// Distinguish a missing field (None) from an explicit `null` (Some(None)).
fn double_option<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(de).map(Some)
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct UserCreateResponse {
    pub user: UserDto,
    pub generated_password: Option<String>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct ResetPasswordResponse {
    pub password: String,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct AdjustRequest {
    pub debt_delta: i64,
    pub balance_delta: i64,
    pub reason: String,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct ImportRowResult {
    pub line: usize,
    pub login: String,
    pub ok: bool,
    pub generated_password: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct ImportResponse {
    pub results: Vec<ImportRowResult>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct NotificationDto {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub data: serde_json::Value,
    #[ts(type = "string | null")]
    pub read_at: Option<DateTime<Utc>>,
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct SummaryDto {
    pub id: Uuid,
    pub year: i32,
    pub month: i32,
    #[ts(type = "string")]
    pub generated_at: DateTime<Utc>,
    pub rows: Option<Vec<SummaryRowDto>>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct SummaryRowDto {
    pub id: Uuid,
    pub student_id: Uuid,
    pub full_name: String,
    pub group_name: Option<String>,
    pub sessions_count: i64,
    pub rounded_minutes: i64,
    pub credited_minutes: i64,
    pub assigned_minutes: i64,
    pub debt_minutes: i64,
    pub balance_minutes: i64,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct GenerateSummaryRequest {
    pub year: i32,
    pub month: i32,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct AuditEntryDto {
    pub id: i64,
    #[ts(type = "string")]
    pub ts: DateTime<Utc>,
    pub actor_id: Option<Uuid>,
    pub actor_login: Option<String>,
    pub actor_role: Option<String>,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub details: serde_json::Value,
    pub ip: Option<String>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct KomendantLogEntryDto {
    pub id: i64,
    #[ts(type = "string")]
    pub ts: DateTime<Utc>,
    pub komendant_id: Uuid,
    pub komendant_name: Option<String>,
    pub action: String,
    pub student_id: Option<Uuid>,
    pub student_name: Option<String>,
    pub session_id: Option<Uuid>,
    pub details: serde_json::Value,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct Paginated<T: TS> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

impl PageQuery {
    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }
    pub fn per_page(&self) -> i64 {
        self.per_page.unwrap_or(20).clamp(1, 100)
    }
    pub fn offset(&self) -> i64 {
        (self.page() - 1) * self.per_page()
    }
}
