use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::SummaryRow;
use crate::state::{AppState, AuditRecord};

/// UTC bounds of a local (APP_TIMEZONE) calendar month.
pub fn month_range_utc(tz: chrono_tz::Tz, year: i32, month: i32) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = tz
        .with_ymd_and_hms(year, month as u32, 1, 0, 0, 0)
        .single()
        .expect("valid month start");
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let end = tz
        .with_ymd_and_hms(ny, nm as u32, 1, 0, 0, 0)
        .single()
        .expect("valid month end");
    (start.with_timezone(&Utc), end.with_timezone(&Utc))
}

#[derive(sqlx::FromRow)]
struct Agg {
    id: Uuid,
    full_name: String,
    group_name: Option<String>,
    sessions_count: i64,
    rounded_minutes: i64,
    credited_minutes: i64,
    assigned_minutes: i64,
    debt_minutes: i64,
    balance_minutes: i64,
}

/// Generate (or regenerate) the summary for a local month. Replaces rows if it exists.
/// Returns the summary id.
pub async fn generate(
    state: &AppState,
    year: i32,
    month: i32,
    actor: Option<Uuid>,
) -> AppResult<Uuid> {
    let (from, to) = month_range_utc(state.config.timezone, year, month);
    let now = state.clock.now();

    let mut tx = state.pool.begin().await?;

    // upsert summary row, wipe old detail rows
    let summary_id: Uuid = sqlx::query_scalar(
        "INSERT INTO monthly_summaries (year, month, generated_at)
         VALUES ($1, $2, $3)
         ON CONFLICT (year, month) DO UPDATE SET generated_at = EXCLUDED.generated_at
         RETURNING id",
    )
    .bind(year)
    .bind(month)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM summary_rows WHERE summary_id = $1")
        .bind(summary_id)
        .execute(&mut *tx)
        .await?;

    // per-student aggregates: credited sessions whose ended_at falls inside the
    // local month (photo-rejected sessions count as reverted -> 0 minutes),
    // assignments created inside the month, debt/balance snapshot.
    let rows = sqlx::query_as::<_, Agg>(
        "SELECT u.id, u.full_name, g.name AS group_name,
                COALESCE(s.cnt, 0) AS sessions_count,
                COALESCE(s.rmin, 0) AS rounded_minutes,
                COALESCE(s.minutes, 0) AS credited_minutes,
                COALESCE(a.minutes, 0) AS assigned_minutes,
                u.debt_minutes, u.balance_minutes
         FROM users u
         LEFT JOIN groups g ON g.id = u.group_id
         LEFT JOIN (
             SELECT student_id, count(*) AS cnt,
                    sum(CASE WHEN photo_review IS DISTINCT FROM 'rejected'
                             THEN rounded_minutes ELSE 0 END)::bigint AS rmin,
                    sum(CASE WHEN photo_review IS DISTINCT FROM 'rejected'
                             THEN credited_minutes ELSE 0 END)::bigint AS minutes
             FROM work_sessions
             WHERE status = 'credited' AND ended_at >= $1 AND ended_at < $2
             GROUP BY student_id
         ) s ON s.student_id = u.id
         LEFT JOIN (
             SELECT student_id, sum(minutes)::bigint AS minutes
             FROM assignments
             WHERE created_at >= $1 AND created_at < $2
             GROUP BY student_id
         ) a ON a.student_id = u.id
         WHERE u.role = 'student'
         ORDER BY g.name NULLS LAST, u.full_name",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&mut *tx)
    .await?;

    for r in &rows {
        sqlx::query(
            "INSERT INTO summary_rows
             (summary_id, student_id, full_name, group_name, sessions_count,
              rounded_minutes, credited_minutes, assigned_minutes, debt_minutes, balance_minutes)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        )
        .bind(summary_id)
        .bind(r.id)
        .bind(&r.full_name)
        .bind(&r.group_name)
        .bind(r.sessions_count)
        .bind(r.rounded_minutes)
        .bind(r.credited_minutes)
        .bind(r.assigned_minutes)
        .bind(r.debt_minutes)
        .bind(r.balance_minutes)
        .execute(&mut *tx)
        .await?;
    }

    state
        .audit(
            &mut *tx,
            AuditRecord::new("summary_generate")
                .actor_opt(actor, "admin")
                .entity("monthly_summary", summary_id)
                .details(json!({"year": year, "month": month, "rows": rows.len()})),
        )
        .await?;
    tx.commit().await?;
    Ok(summary_id)
}

/// minutes -> "1ч 44м" (or "45м" / "-1ч 44м")
pub fn fmt_hm(minutes: i64) -> String {
    let sign = if minutes < 0 { "-" } else { "" };
    let m = minutes.abs();
    let (h, mm) = (m / 60, m % 60);
    match (h, mm) {
        (0, mm) => format!("{sign}{mm}м"),
        (h, 0) => format!("{sign}{h}ч"),
        (h, mm) => format!("{sign}{h}ч {mm}м"),
    }
}

/// Build the XLSX export: one sheet, Russian headers, times as Ч:ММ,
/// rows sorted by group then name.
pub fn to_xlsx(year: i32, month: i32, rows: &[SummaryRow]) -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
    use rust_xlsxwriter::{Format, Workbook};

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet().set_name(format!("{year}-{month:02}"))?;
    let bold = Format::new().set_bold();
    let headers = [
        "ФИО",
        "Группа",
        "Сессий",
        "Отработано",
        "Зачтено",
        "Назначено",
        "Долг",
        "Баланс",
    ];
    for (c, h) in headers.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &bold)?;
    }
    for (i, r) in rows.iter().enumerate() {
        let row = (i + 1) as u32;
        ws.write_string(row, 0, &r.full_name)?;
        ws.write_string(row, 1, r.group_name.as_deref().unwrap_or(""))?;
        ws.write_number(row, 2, r.sessions_count as f64)?;
        ws.write_string(row, 3, fmt_hm(r.rounded_minutes))?;
        ws.write_string(row, 4, fmt_hm(r.credited_minutes))?;
        ws.write_string(row, 5, fmt_hm(r.assigned_minutes))?;
        ws.write_string(row, 6, fmt_hm(r.debt_minutes))?;
        ws.write_string(row, 7, fmt_hm(r.balance_minutes))?;
    }
    ws.set_column_width(0, 30)?;
    ws.set_column_width(1, 15)?;
    for c in 2..8u16 {
        ws.set_column_width(c, 12)?;
    }
    wb.save_to_buffer()
}
