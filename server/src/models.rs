use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, TS)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../shared/api-types/")]
pub enum Role {
    Student,
    Teacher,
    Komendant,
    Admin,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Student => "student",
            Role::Teacher => "teacher",
            Role::Komendant => "komendant",
            Role::Admin => "admin",
        }
    }

    pub fn from_ru(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "студент" | "student" => Some(Role::Student),
            "преподаватель" | "учитель" | "teacher" => Some(Role::Teacher),
            "комендант" | "komendant" => Some(Role::Komendant),
            "админ" | "администратор" | "admin" => Some(Role::Admin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, TS)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../shared/api-types/")]
pub enum Difficulty {
    Light,
    Hard,
}

impl Difficulty {
    pub fn coefficient(&self) -> i64 {
        match self {
            Difficulty::Light => 1,
            Difficulty::Hard => 2,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Difficulty::Light => "light",
            Difficulty::Hard => "hard",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, TS)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../shared/api-types/")]
pub enum SessionStatus {
    Active,
    PendingConfirmation,
    Credited,
    Expired,
    KomendantRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, TS)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../shared/api-types/")]
pub enum PhotoReview {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, TS)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../shared/api-types/")]
pub enum OrderStatus {
    Pending,
    Issued,
    Cancelled,
}

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: Uuid,
    pub role: Role,
    pub login: String,
    pub password_hash: String,
    pub full_name: String,
    pub group_id: Option<Uuid>,
    pub debt_minutes: i64,
    pub balance_minutes: i64,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Group {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct WorkSession {
    pub id: Uuid,
    pub student_id: Uuid,
    pub komendant_id: Uuid,
    pub difficulty: Difficulty,
    pub coefficient: i32,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: SessionStatus,
    pub raw_seconds: Option<i64>,
    pub rounded_minutes: Option<i64>,
    pub credited_minutes: Option<i64>,
    pub applied_to_debt: Option<i64>,
    pub applied_to_balance: Option<i64>,
    pub reverted_from_balance: Option<i64>,
    pub reverted_to_debt: Option<i64>,
    pub photo_path: Option<String>,
    pub photo_review: Option<PhotoReview>,
    pub photo_review_comment: Option<String>,
    pub photo_reviewed_by: Option<Uuid>,
    pub photo_reviewed_at: Option<DateTime<Utc>>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub rejected_reason: Option<String>,
    pub expired_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Assignment {
    pub id: Uuid,
    pub student_id: Uuid,
    pub assigned_by: Uuid,
    pub minutes: i64,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Reward {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub price_minutes: i64,
    pub stock: Option<i64>,
    pub is_active: bool,
    pub image_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Order {
    pub id: Uuid,
    pub user_id: Uuid,
    pub reward_id: Uuid,
    pub price_minutes: i64,
    pub status: OrderStatus,
    pub issued_by: Option<Uuid>,
    pub issued_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub data: serde_json::Value,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct MonthlySummary {
    pub id: Uuid,
    pub year: i32,
    pub month: i32,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SummaryRow {
    pub id: Uuid,
    pub summary_id: Uuid,
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

#[derive(Debug, Clone, FromRow)]
pub struct AuditEntry {
    pub id: i64,
    pub ts: DateTime<Utc>,
    pub actor_id: Option<Uuid>,
    pub actor_role: Option<String>,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub details: serde_json::Value,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct KomendantLogEntry {
    pub id: i64,
    pub ts: DateTime<Utc>,
    pub komendant_id: Uuid,
    pub action: String,
    pub student_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub details: serde_json::Value,
}
