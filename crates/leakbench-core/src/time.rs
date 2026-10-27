//! Minimal, dependency-free civil-time helpers.
//!
//! LeakBench intentionally avoids a chrono/time dependency: the only thing we
//! need is "epoch seconds -> RFC 3339 UTC", which is thirty lines of Hinnant's
//! civil-from-days algorithm. Fewer dependencies, faster build, smaller attack
//! surface for a tool that is supposed to be auditable.

/// Convert a Unix timestamp to an RFC 3339 UTC string (`2026-10-27T09:52:17Z`).
pub fn to_iso8601(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let secs_of_day = epoch_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Howard Hinnant's `civil_from_days`, valid for the whole range we care about.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Epoch seconds for "now", without pulling in a clock crate.
pub fn now_epoch_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Human-readable span, e.g. `3 blocks / 1m 15s`.
pub fn describe_span(block_interval_secs: u64, blocks: u64) -> String {
    let secs = block_interval_secs.saturating_mul(blocks);
    let unit = if blocks == 1 { "block" } else { "blocks" };
    if secs < 120 {
        format!("{blocks} {unit} / {secs}s")
    } else if secs < 3600 {
        format!("{blocks} {unit} / {}m {}s", secs / 60, secs % 60)
    } else {
        format!("{blocks} {unit} / {}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_instants() {
        assert_eq!(to_iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(to_iso8601(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(to_iso8601(1_795_000_000), "2026-11-18T11:06:40Z");
    }

    #[test]
    fn handles_leap_day() {
        assert_eq!(to_iso8601(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn describes_spans() {
        assert_eq!(describe_span(75, 1), "1 block / 75s");
        assert_eq!(describe_span(25, 4), "4 blocks / 100s");
        assert_eq!(describe_span(75, 12), "12 blocks / 15m 0s");
        assert_eq!(describe_span(25, 200), "200 blocks / 1h 23m");
    }
}
