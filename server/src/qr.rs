use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::Difficulty;

const PREFIX: &str = "prila1";
const WINDOW_SECS: i64 = 30;
/// Finish QR: a separate kind (cannot start a session), changes every 3 minutes.
const FINISH_PREFIX: &str = "prila1f";
const FINISH_WINDOW_SECS: i64 = 180;
const MAC_HEX_LEN: usize = 16; // 8 bytes of the HMAC, hex-encoded

type HmacSha256 = Hmac<Sha256>;

fn mac_hex(secret: &str, payload: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac accepts any key");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())[..MAC_HEX_LEN].to_string()
}

pub struct IssuedQr {
    pub qr: String,
    pub expires_at: DateTime<Utc>,
    pub difficulty: Difficulty,
}

/// Build the QR payload `prila1.<komendant_id>.<difficulty>.<window>.<mac>`
/// where window = unix_seconds / 30.
pub fn issue(secret: &str, komendant_id: Uuid, difficulty: Difficulty, now: DateTime<Utc>) -> IssuedQr {
    let window = now.timestamp() / WINDOW_SECS;
    let payload = format!("{PREFIX}.{komendant_id}.{}.{window}", difficulty.as_str());
    let qr = format!("{payload}.{}", mac_hex(secret, &payload));
    IssuedQr {
        qr,
        expires_at: DateTime::from_timestamp((window + 2) * WINDOW_SECS, 0)
            .expect("window timestamp in range"),
        difficulty,
    }
}

#[derive(Debug)]
pub struct VerifiedQr {
    pub komendant_id: Uuid,
    pub difficulty: Difficulty,
}

fn invalid() -> AppError {
    AppError::bad_request("invalid_qr", "Недействительный QR-код")
}

fn expired() -> AppError {
    AppError::bad_request("expired_qr", "QR-код устарел, попросите коменданта обновить его")
}

/// Constant-time check of the truncated HMAC of `payload`.
fn mac_ok(secret: &str, payload: &str, mac: &str) -> bool {
    let Ok(mac_bytes) = hex::decode(mac) else { return false };
    let mut expected = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac accepts any key");
    expected.update(payload.as_bytes());
    let tag = expected.finalize().into_bytes();
    mac_bytes.len() == MAC_HEX_LEN / 2
        && tag[..MAC_HEX_LEN / 2]
            .iter()
            .zip(mac_bytes.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// Verify a QR payload. Valid while the window is current or the previous one (~<=60s).
pub fn verify(secret: &str, qr: &str, now: DateTime<Utc>) -> Result<VerifiedQr, AppError> {
    let parts: Vec<&str> = qr.split('.').collect();
    if parts[0].eq_ignore_ascii_case(FINISH_PREFIX) {
        return Err(AppError::bad_request(
            "wrong_qr_kind",
            "Это QR для завершения ОПТ. Чтобы начать, попросите коменданта открыть QR начала",
        ));
    }
    if parts.len() != 5 || !parts[0].eq_ignore_ascii_case(PREFIX) {
        return Err(invalid());
    }
    let komendant_id = Uuid::parse_str(parts[1]).map_err(|_| invalid())?;
    let difficulty = match parts[2] {
        "light" => Difficulty::Light,
        "hard" => Difficulty::Hard,
        _ => return Err(invalid()),
    };
    let window: i64 = parts[3].parse().map_err(|_| invalid())?;
    let mac = parts[4];
    if mac.len() != MAC_HEX_LEN {
        return Err(invalid());
    }

    let current = now.timestamp() / WINDOW_SECS;
    if window != current && window != current - 1 {
        return Err(expired());
    }

    let payload = format!("{PREFIX}.{komendant_id}.{}.{window}", difficulty.as_str());
    if !mac_ok(secret, &payload, mac) {
        return Err(invalid());
    }

    Ok(VerifiedQr {
        komendant_id,
        difficulty,
    })
}

pub struct IssuedFinishQr {
    pub qr: String,
    pub expires_at: DateTime<Utc>,
}

/// Build the finish QR `prila1f.<komendant_id>.<window>.<mac>` where window = unix_seconds / 180.
pub fn issue_finish(secret: &str, komendant_id: Uuid, now: DateTime<Utc>) -> IssuedFinishQr {
    let window = now.timestamp() / FINISH_WINDOW_SECS;
    let payload = format!("{FINISH_PREFIX}.{komendant_id}.{window}");
    let qr = format!("{payload}.{}", mac_hex(secret, &payload));
    IssuedFinishQr {
        qr,
        expires_at: DateTime::from_timestamp((window + 2) * FINISH_WINDOW_SECS, 0)
            .expect("window timestamp in range"),
    }
}

/// Verify a finish QR; returns the komendant who showed it. Valid in the current or
/// the previous 3-minute window (3–6 min), like the start QR.
pub fn verify_finish(secret: &str, qr: &str, now: DateTime<Utc>) -> Result<Uuid, AppError> {
    let parts: Vec<&str> = qr.split('.').collect();
    if parts.len() == 5 && parts[0].eq_ignore_ascii_case(PREFIX) {
        return Err(AppError::bad_request(
            "wrong_qr_kind",
            "Это QR для начала ОПТ. Для завершения попросите коменданта открыть QR завершения",
        ));
    }
    if parts.len() != 4 || !parts[0].eq_ignore_ascii_case(FINISH_PREFIX) {
        return Err(invalid());
    }
    let komendant_id = Uuid::parse_str(parts[1]).map_err(|_| invalid())?;
    let window: i64 = parts[2].parse().map_err(|_| invalid())?;
    let mac = parts[3];
    if mac.len() != MAC_HEX_LEN {
        return Err(invalid());
    }
    let current = now.timestamp() / FINISH_WINDOW_SECS;
    if window != current && window != current - 1 {
        return Err(expired());
    }
    if !mac_ok(secret, &format!("{FINISH_PREFIX}.{komendant_id}.{window}"), mac) {
        return Err(invalid());
    }
    Ok(komendant_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const SECRET: &str = "test-qr-secret";

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn issue_and_verify_current_window() {
        let id = Uuid::new_v4();
        let now = at(1_700_000_000);
        let issued = issue(SECRET, id, Difficulty::Hard, now);
        assert!(issued.qr.starts_with(&format!("prila1.{id}.hard.")));
        let v = verify(SECRET, &issued.qr, now).unwrap();
        assert_eq!(v.komendant_id, id);
        assert_eq!(v.difficulty, Difficulty::Hard);
    }

    #[test]
    fn verify_previous_window_ok() {
        let id = Uuid::new_v4();
        let issued = issue(SECRET, id, Difficulty::Light, at(1_700_000_000));
        // 35 seconds later -> previous window, still valid
        let v = verify(SECRET, &issued.qr, at(1_700_000_035)).unwrap();
        assert_eq!(v.komendant_id, id);
    }

    #[test]
    fn verify_expired_window_rejected() {
        let id = Uuid::new_v4();
        let issued = issue(SECRET, id, Difficulty::Light, at(1_700_000_000));
        // 90 seconds later -> two windows ago, invalid
        let err = verify(SECRET, &issued.qr, at(1_700_000_090)).unwrap_err();
        assert_eq!(err.code, "expired_qr");
    }

    #[test]
    fn tampered_mac_rejected() {
        let id = Uuid::new_v4();
        let issued = issue(SECRET, id, Difficulty::Light, at(1_700_000_000));
        let tampered = issued.qr.replace("light", "hard");
        verify(SECRET, &tampered, at(1_700_000_000)).unwrap_err();
        let bad_mac = format!("{}0000", &issued.qr[..issued.qr.len() - 4]);
        verify(SECRET, &bad_mac, at(1_700_000_000)).unwrap_err();
    }

    #[test]
    fn wrong_secret_rejected() {
        let id = Uuid::new_v4();
        let issued = issue("other-secret", id, Difficulty::Light, at(1_700_000_000));
        verify(SECRET, &issued.qr, at(1_700_000_000)).unwrap_err();
    }

    #[test]
    fn garbage_rejected() {
        verify(SECRET, "not-a-qr", at(1_700_000_000)).unwrap_err();
        verify(SECRET, "prila1.xyz.light.0.abc", at(1_700_000_000)).unwrap_err();
        verify_finish(SECRET, "not-a-qr", at(1_700_000_000)).unwrap_err();
        verify_finish(SECRET, "prila1f.xyz.0.abc", at(1_700_000_000)).unwrap_err();
    }

    #[test]
    fn finish_qr_three_minute_windows() {
        let id = Uuid::new_v4();
        // window boundary: 1_700_000_100 is the start of a 180 s window
        let issued = issue_finish(SECRET, id, at(1_700_000_100));
        assert!(issued.qr.starts_with(&format!("prila1f.{id}.")));
        assert_eq!(verify_finish(SECRET, &issued.qr, at(1_700_000_100)).unwrap(), id);
        // next window (up to 6 min after issue) still accepted
        assert_eq!(verify_finish(SECRET, &issued.qr, at(1_700_000_100 + 359)).unwrap(), id);
        let err = verify_finish(SECRET, &issued.qr, at(1_700_000_100 + 360)).unwrap_err();
        assert_eq!(err.code, "expired_qr");
        // tampered komendant id
        let other = issued.qr.replace(&id.to_string(), &Uuid::new_v4().to_string());
        assert_eq!(verify_finish(SECRET, &other, at(1_700_000_100)).unwrap_err().code, "invalid_qr");
    }

    #[test]
    fn start_and_finish_qr_not_interchangeable() {
        let id = Uuid::new_v4();
        let now = at(1_700_000_100);
        let start = issue(SECRET, id, Difficulty::Light, now);
        let finish = issue_finish(SECRET, id, now);
        assert_eq!(verify_finish(SECRET, &start.qr, now).unwrap_err().code, "wrong_qr_kind");
        assert_eq!(verify(SECRET, &finish.qr, now).unwrap_err().code, "wrong_qr_kind");
    }
}
