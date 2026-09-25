//! Toolkit-agnostic content of File -> Database info (#193): the labelled
//! fields the app shows, and the formatting behind them.

use std::time::SystemTime;

use crate::library_api::{DatabaseInfo, LibraryDataSource};

/// The dialog's labelled fields, top to bottom, formatted for display.
pub fn fields(library: &dyn LibraryDataSource) -> Vec<(&'static str, String)> {
    let info = library.database_info();
    vec![
        ("Tracks", library.track_count().to_string()),
        ("Albums", library.albums().len().to_string()),
        ("Artists", library.artists().len().to_string()),
        ("Total duration", duration_text(library)),
        ("Database file", path_text(&info)),
        ("Database size", size_text(info.size_bytes)),
        ("Last scan", last_scan_text(info.last_scan)),
    ]
}

fn duration_text(library: &dyn LibraryDataSource) -> String {
    let secs = library.total_duration().as_secs();
    format!("{}h {:02}m {:02}s", secs / 3600, secs / 60 % 60, secs % 60)
}

fn path_text(info: &DatabaseInfo) -> String {
    info.path.as_ref().map_or_else(
        || "In memory (not saved)".to_owned(),
        |path| path.display().to_string(),
    )
}

/// Formats a byte count with a binary unit, e.g. `17.5 MiB`.
fn size_text(bytes: Option<u64>) -> String {
    let Some(bytes) = bytes else {
        return "Unknown".to_owned();
    };
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// Formats the last scan time in UTC, or a note that none ran this session.
fn last_scan_text(time: Option<SystemTime>) -> String {
    let Some(secs) = time
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
    else {
        return "No scan yet this session".to_owned();
    };
    let (year, month, day) = civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// Converts days since 1970-01-01 to a (year, month, day) civil date
/// (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_use_binary_units() {
        assert_eq!(size_text(Some(512)), "512 B");
        assert_eq!(size_text(Some(1536)), "1.5 KiB");
        assert_eq!(size_text(Some(18_350_080)), "17.5 MiB");
        assert_eq!(size_text(None), "Unknown");
    }

    #[test]
    fn civil_dates_convert() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_454), (2026, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn last_scan_formats_in_utc() {
        let time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_767_225_600 + 3_661);
        assert_eq!(last_scan_text(Some(time)), "2026-01-01 01:01:01 UTC");
        assert_eq!(last_scan_text(None), "No scan yet this session");
    }
}
