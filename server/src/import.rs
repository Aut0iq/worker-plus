use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Reader};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{hash_password, normalize_login, password_too_short, random_password};
use crate::dto::ImportRowResult;
use crate::error::{AppError, AppResult};
use crate::models::Role;
use crate::state::{AppState, AuditRecord};

struct Row {
    line: usize,
    full_name: String,
    login: String,
    password: Option<String>,
    role: Role,
    group: Option<String>,
}

fn fail(line: usize, login: &str, err: &str) -> ImportRowResult {
    ImportRowResult {
        line,
        login: login.to_string(),
        ok: false,
        generated_password: None,
        error: Some(err.to_string()),
    }
}

/// Parse CSV or XLSX bytes into rows. Expected columns:
/// ФИО, Логин, Пароль(optional), Роль, Группа(optional)
fn parse(name: &str, data: &[u8]) -> Result<Vec<Vec<String>>, AppError> {
    let lower = name.to_lowercase();
    if lower.ends_with(".xlsx") || lower.ends_with(".xlsm") || lower.ends_with(".xls") {
        let mut wb = open_workbook_auto_from_rs(Cursor::new(data.to_vec()))
            .map_err(|e| AppError::bad_request("bad_file", format!("Не удалось прочитать файл: {e}")))?;
        let sheet = wb
            .worksheet_range_at(0)
            .ok_or_else(|| AppError::bad_request("bad_file", "Файл не содержит листов"))?
            .map_err(|e| AppError::bad_request("bad_file", format!("Не удалось прочитать лист: {e}")))?;
        Ok(sheet
            .rows()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect())
    } else {
        // Excel saves "CSV" with a UTF-8 BOM, often with ';' and sometimes in cp1251
        let data = data.strip_prefix(b"\xEF\xBB\xBF".as_slice()).unwrap_or(data);
        if std::str::from_utf8(data).is_err() {
            return Err(AppError::bad_request(
                "bad_encoding",
                "Файл не в кодировке UTF-8. В Excel сохраните как «CSV UTF-8 (разделитель — запятая)» или загрузите .xlsx",
            ));
        }
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .delimiter(detect_delimiter(data))
            .from_reader(data);
        let mut out = Vec::new();
        for rec in rdr.records() {
            let rec = rec.map_err(|e| {
                AppError::bad_request("bad_file", format!("Не удалось прочитать CSV: {e}"))
            })?;
            out.push(rec.iter().map(|s| s.to_string()).collect());
        }
        Ok(out)
    }
}

/// ';' if the first line has more semicolons than commas (Russian Excel default), else ','.
fn detect_delimiter(data: &[u8]) -> u8 {
    let first = data.split(|&b| b == b'\n').next().unwrap_or(&[]);
    let count = |c: u8| first.iter().filter(|&&b| b == c).count();
    if count(b';') > count(b',') {
        b';'
    } else {
        b','
    }
}

fn to_rows(raw: Vec<Vec<String>>) -> (Vec<Row>, Vec<ImportRowResult>) {
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for (idx, cols) in raw.iter().enumerate() {
        let line = idx + 1;
        let get = |i: usize| cols.get(i).map(|s| s.trim()).unwrap_or("");
        let (fio, pass, role_s, group) = (get(0), get(2), get(3), get(4));
        let login_norm = normalize_login(get(1));
        let login = login_norm.as_str();
        // skip header row and fully empty rows
        if line == 1 && fio.to_lowercase().contains("фио") {
            continue;
        }
        if fio.is_empty() && login.is_empty() && role_s.is_empty() {
            continue;
        }
        if fio.is_empty() || login.is_empty() {
            errors.push(fail(line, login, "Пустое ФИО или логин"));
            continue;
        }
        let Some(role) = Role::from_ru(role_s) else {
            errors.push(fail(line, login, &format!("Неизвестная роль: {role_s}")));
            continue;
        };
        let password = if pass.is_empty() {
            None
        } else if password_too_short(pass) {
            errors.push(fail(line, login, "Пароль короче 6 символов"));
            continue;
        } else {
            Some(pass.to_string())
        };
        rows.push(Row {
            line,
            full_name: fio.to_string(),
            login: login.to_string(),
            password,
            role,
            group: if group.is_empty() {
                None
            } else {
                Some(group.to_string())
            },
        });
    }
    (rows, errors)
}

/// Import users from a CSV/XLSX file. All-or-nothing: any row error aborts the
/// whole import and returns per-row errors.
pub async fn import_users(
    state: &AppState,
    actor: Uuid,
    name: &str,
    data: &[u8],
) -> AppResult<Vec<ImportRowResult>> {
    let raw = parse(name, data)?;
    let (rows, mut errors) = to_rows(raw);

    // validate duplicates inside the file and against the DB
    let mut seen = HashSet::new();
    for r in &rows {
        if !seen.insert(r.login.clone()) {
            errors.push(fail(r.line, &r.login, "Дубль логина в файле"));
        }
    }
    if !errors.is_empty() {
        return Ok(results_with_errors(&rows, errors));
    }

    let logins: Vec<String> = rows.iter().map(|r| r.login.clone()).collect();
    let existing: Vec<String> =
        sqlx::query_scalar("SELECT login FROM users WHERE lower(login) = ANY($1)")
            .bind(&logins)
            .fetch_all(&state.pool)
            .await?;
    let existing: HashSet<String> = existing.into_iter().map(|l| l.to_lowercase()).collect();
    for r in &rows {
        if existing.contains(&r.login) {
            errors.push(fail(r.line, &r.login, "Логин уже существует"));
        }
    }
    if !errors.is_empty() {
        return Ok(results_with_errors(&rows, errors));
    }

    // argon2 for every row is slow: do it off the async threads, before the transaction
    let plain: Vec<(Option<String>, String)> = rows
        .iter()
        .map(|r| {
            let generated = r.password.is_none().then(random_password);
            let password = r.password.clone().or_else(|| generated.clone()).unwrap_or_default();
            (generated, password)
        })
        .collect();
    let hashed: Vec<(Option<String>, String)> = tokio::task::spawn_blocking(move || {
        plain
            .into_iter()
            .map(|(generated, password)| hash_password(&password).map(|h| (generated, h)))
            .collect::<AppResult<Vec<_>>>()
    })
    .await
    .map_err(AppError::internal)??;

    // auto-create missing groups inside the same transaction
    let mut tx = state.pool.begin().await?;
    let group_names: Vec<String> = rows
        .iter()
        .filter_map(|r| r.group.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let mut groups: HashMap<String, Uuid> = HashMap::new();
    for name in &group_names {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO groups (name) VALUES ($1)
             ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
             RETURNING id",
        )
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;
        groups.insert(name.clone(), id);
    }

    let mut results = Vec::new();
    let mut imported = 0usize;
    for (r, (generated, hash)) in rows.iter().zip(hashed) {
        let group_id = r.group.as_ref().map(|g| groups[g]);
        sqlx::query(
            "INSERT INTO users (role, login, password_hash, full_name, group_id)
             VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(r.role)
        .bind(&r.login)
        .bind(&hash)
        .bind(&r.full_name)
        .bind(group_id)
        .execute(&mut *tx)
        .await?;
        imported += 1;
        results.push(ImportRowResult {
            line: r.line,
            login: r.login.clone(),
            ok: true,
            generated_password: generated,
            error: None,
        });
    }

    state
        .audit(
            &mut *tx,
            AuditRecord::new("users_import")
                .actor(actor, "admin")
                .details(json!({"file": name, "imported": imported})),
        )
        .await?;
    tx.commit().await?;
    Ok(results)
}

fn results_with_errors(rows: &[Row], errors: Vec<ImportRowResult>) -> Vec<ImportRowResult> {
    let bad: HashSet<usize> = errors.iter().map(|e| e.line).collect();
    let mut out = errors;
    for r in rows {
        if !bad.contains(&r.line) {
            out.push(fail(
                r.line,
                &r.login,
                "Импорт отменён из-за ошибок в других строках",
            ));
        }
    }
    out.sort_by_key(|r| r.line);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_semicolon_bom_and_login_case() {
        let data = "\u{feff}ФИО;Логин;Пароль;Роль;Группа\nИванов Иван; Ivanov ;;студент;1А\n";
        let raw = parse("users.csv", data.as_bytes()).unwrap();
        let (rows, errors) = to_rows(raw);
        assert!(errors.is_empty());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].login, "ivanov");
        assert_eq!(rows[0].group.as_deref(), Some("1А"));
    }

    #[test]
    fn csv_non_utf8_is_rejected() {
        // "ФИО" in cp1251
        let err = parse("users.csv", b"\xd4\xc8\xce,login").unwrap_err();
        assert_eq!(err.code, "bad_encoding");
    }

    #[test]
    fn delimiter_detection() {
        assert_eq!(detect_delimiter(b"a;b;c\n1,2"), b';');
        assert_eq!(detect_delimiter(b"a,b,c"), b',');
    }
}
