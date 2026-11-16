//! Pure domain logic — no I/O, fully unit-testable.

/// Round raw seconds to minutes: <30s remainder drops, >=30s rounds up.
/// e.g. 1h43m31s -> 104 min, 29s -> 0, 30s -> 1.
pub fn rounded_minutes(raw_seconds: i64) -> i64 {
    debug_assert!(raw_seconds >= 0);
    (raw_seconds + 30) / 60
}

/// credited minutes = rounded minutes * difficulty coefficient.
pub fn credited_minutes(rounded_minutes: i64, coefficient: i64) -> i64 {
    rounded_minutes * coefficient
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    pub to_debt: i64,
    pub to_balance: i64,
}

/// On confirm: credited minutes first cover debt, the rest goes to balance.
pub fn allocate(credited: i64, debt_minutes: i64) -> Allocation {
    let to_debt = credited.min(debt_minutes.max(0));
    Allocation {
        to_debt,
        to_balance: credited - to_debt,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revert {
    /// amount actually taken back from balance
    pub from_balance: i64,
    /// amount restored to debt (applied_to_debt + unrecoverable balance part)
    pub to_debt: i64,
}

/// On photo rejection: take back from balance what is still there;
/// whatever cannot be recovered from balance is converted back into debt.
pub fn revert(applied_to_debt: i64, applied_to_balance: i64, balance: i64) -> Revert {
    let from_balance = applied_to_balance.min(balance.max(0));
    Revert {
        from_balance,
        to_debt: applied_to_debt + (applied_to_balance - from_balance),
    }
}

/// Detect an image type by magic bytes and return its file extension.
/// Supports jpeg, png, webp and heic/heif — matches the accepted photo formats.
pub fn sniff_image_ext(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some("png");
    }
    if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return Some("webp");
    }
    // ISO-BMFF: 4-byte box size, then "ftyp", then the major brand.
    if data.len() >= 12 && &data[4..8] == b"ftyp" {
        let brand = &data[8..12];
        if [b"heic", b"heix", b"hevc", b"hevx", b"mif1", b"msf1"]
            .iter()
            .any(|b| b.as_slice() == brand)
        {
            return Some("heic");
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding() {
        assert_eq!(rounded_minutes(0), 0);
        assert_eq!(rounded_minutes(29), 0);
        assert_eq!(rounded_minutes(30), 1);
        assert_eq!(rounded_minutes(59), 1);
        assert_eq!(rounded_minutes(60), 1);
        assert_eq!(rounded_minutes(61), 1);
        assert_eq!(rounded_minutes(89), 1);
        assert_eq!(rounded_minutes(90), 2);
        // 1h43m31s = 6211s -> 104 min
        assert_eq!(rounded_minutes(6211), 104);
        // 1h43m29s = 6209s -> 103
        assert_eq!(rounded_minutes(6209), 103);
    }

    #[test]
    fn sniff_images() {
        assert_eq!(sniff_image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(sniff_image_ext(&[0x89, 0x50, 0x4E, 0x47, 0, 0, 0, 0]), Some("png"));
        assert_eq!(sniff_image_ext(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some("webp"));
        assert_eq!(sniff_image_ext(b"\x00\x00\x00\x18ftypheic\x00"), Some("heic"));
        assert_eq!(sniff_image_ext(b"\x00\x00\x00\x18ftypmif1\x00"), Some("heic"));
        assert_eq!(sniff_image_ext(b"\x00\x00\x00\x18ftypmp41\x00"), None);
        assert_eq!(sniff_image_ext(b"GIF89a"), None);
        assert_eq!(sniff_image_ext(b""), None);
    }

    #[test]
    fn credited_with_coefficient() {
        assert_eq!(credited_minutes(104, 1), 104);
        assert_eq!(credited_minutes(104, 2), 208);
    }

    #[test]
    fn allocation() {
        assert_eq!(
            allocate(100, 60),
            Allocation {
                to_debt: 60,
                to_balance: 40
            }
        );
        assert_eq!(
            allocate(100, 0),
            Allocation {
                to_debt: 0,
                to_balance: 100
            }
        );
        assert_eq!(
            allocate(50, 200),
            Allocation {
                to_debt: 50,
                to_balance: 0
            }
        );
    }

    #[test]
    fn revert_full_balance() {
        let r = revert(30, 70, 100);
        assert_eq!(r.from_balance, 70);
        assert_eq!(r.to_debt, 30);
    }

    #[test]
    fn revert_insufficient_balance_goes_to_debt() {
        // 70 was applied to balance but only 20 remains -> 50 goes back to debt
        let r = revert(30, 70, 20);
        assert_eq!(r.from_balance, 20);
        assert_eq!(r.to_debt, 30 + 50);
    }
}
