//! UTC timestamp formatting without chrono (std has no calendar).

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_utc() -> String {
    // A clock before 1970 is a broken machine; report the epoch rather than fail.
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    utc_stamp(secs)
}

pub fn utc_stamp(secs: u64) -> String {
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Howard Hinnant's days-since-epoch to civil date algorithm (proleptic Gregorian).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::utc_stamp;

    #[test]
    fn known_dates() {
        assert_eq!(utc_stamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc_stamp(951_782_400), "2000-02-29 00:00:00 UTC"); // leap day
        assert_eq!(utc_stamp(1_700_000_000), "2023-11-14 22:13:20 UTC");
    }
}
