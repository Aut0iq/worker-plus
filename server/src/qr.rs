use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::Difficulty;

const PREFIX: &str = "prila1";
const WINDOW_SECS: i64 = 30;
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

/// Verify a QR payload. Valid while the window is current or the previous one (~<=60s).
pub fn verify(secret: &str, qr: &str, now: DateTime<Utc>) -> Result<VerifiedQr, AppError> {
    let parts: Vec<&str> = qr.split('.').collect();
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
        return Err(AppError::bad_request(
            "expired_qr",
            "QR-код устарел, попросите коменданта обновить его",
        ));
    }

    let payload = format!("{PREFIX}.{komendant_id}.{}.{window}", difficulty.as_str());
    let mut expected = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac accepts any key");
    expected.update(payload.as_bytes());
    let mac_bytes = hex::decode(mac).map_err(|_| invalid())?;
    let tag = expected.finalize().into_bytes();
    let ok = mac_bytes.len() == MAC_HEX_LEN / 2
        && tag[..MAC_HEX_LEN / 2]
            .iter()
            .zip(mac_bytes.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0;
    if !ok {
        return Err(invalid());
    }

    Ok(VerifiedQr {
        komendant_id,
        difficulty,
    })
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
    }
}
