//! HTML building blocks: escaping, the page layout and value formatting.
//!
//! Pages are plain server-rendered HTML with no script at all; the stylesheet
//! is served from `/admin/style.css` so the CSP can forbid inline styles.

use std::fmt::Write as _;

use axum::response::Html;

/// Navigation entries: path and label.
const NAV: &[(&str, &str)] = &[
    ("/admin/", "Overview"),
    ("/admin/active", "Active users"),
    ("/admin/devices", "Devices"),
    ("/admin/pairing", "Pairing"),
    ("/admin/audit", "Audit log"),
    ("/admin/database", "Database"),
];

/// Escapes text for use in HTML element content and quoted attributes.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Percent-encodes a query-string value.
pub fn url_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// Options for [`page`].
#[derive(Debug, Default, Clone, Copy)]
pub struct PageOptions {
    /// Reload the page every this many seconds.
    pub refresh_secs: Option<u32>,
}

/// Wraps `body` in the admin layout. `current` is the nav path to highlight.
pub fn page(title: &str, current: &str, options: PageOptions, body: &str) -> Html<String> {
    let mut nav = String::new();
    for (path, label) in NAV {
        let class = if *path == current {
            " class=\"current\""
        } else {
            ""
        };
        let _ = write!(nav, "<a href=\"{path}\"{class}>{label}</a>");
    }
    let refresh = options
        .refresh_secs
        .map(|secs| format!("<meta http-equiv=\"refresh\" content=\"{secs}\">"))
        .unwrap_or_default();
    Html(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <meta name=\"referrer\" content=\"no-referrer\">{refresh}\
         <title>{title} · emusic-server admin</title>\
         <link rel=\"stylesheet\" href=\"/admin/style.css\"></head><body>\
         <header><span class=\"brand\">emusic-server admin</span><nav>{nav}</nav></header>\
         <main><h1>{title}</h1>{body}</main></body></html>",
        title = escape(title),
    ))
}

/// A two-column key/value table. Values are inserted as-is (pre-escaped).
pub fn key_values(rows: &[(&str, String)]) -> String {
    let mut out = String::from("<table class=\"kv\">");
    for (key, value) in rows {
        let _ = write!(out, "<tr><th>{}</th><td>{value}</td></tr>", escape(key));
    }
    out.push_str("</table>");
    out
}

/// Formats Unix seconds as `YYYY-MM-DD HH:MM:SS UTC`.
pub fn time(unix: i64) -> String {
    crate::util::format_utc(unix)
}

/// Formats an optional timestamp, `never` when absent.
pub fn time_or_never(unix: Option<i64>) -> String {
    unix.map(time).unwrap_or_else(|| "never".to_string())
}

/// Formats a duration in seconds as `1d 2h 3m 4s` (largest units only).
pub fn duration(secs: i64) -> String {
    let secs = secs.max(0);
    let (days, hours, minutes, seconds) =
        (secs / 86_400, secs / 3600 % 24, secs / 60 % 60, secs % 60);
    match (days, hours, minutes) {
        (0, 0, 0) => format!("{seconds}s"),
        (0, 0, _) => format!("{minutes}m {seconds}s"),
        (0, _, _) => format!("{hours}h {minutes}m"),
        _ => format!("{days}d {hours}h"),
    }
}

/// Formats a byte count with a binary unit.
pub fn bytes(value: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut amount = value as f64;
    let mut unit = 0;
    while amount >= 1024.0 && unit + 1 < UNITS.len() {
        amount /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value} B")
    } else {
        format!("{amount:.1} {}", UNITS[unit])
    }
}

/// The admin stylesheet: small, light/dark via `prefers-color-scheme`.
pub const STYLESHEET: &str = r#":root{color-scheme:light dark;--bg:#f7f7f8;--fg:#1d1d22;--muted:#61616b;--card:#fff;--line:#dcdce2;--accent:#3550c8;--warn:#a3341f;--ok:#25723a}
@media (prefers-color-scheme:dark){:root{--bg:#141418;--fg:#e8e8ee;--muted:#9a9aa6;--card:#1d1d23;--line:#33333d;--accent:#8ea2ff;--warn:#ff8a73;--ok:#7bd88f}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.45 system-ui,-apple-system,"Segoe UI",sans-serif}
header{display:flex;flex-wrap:wrap;gap:12px 24px;align-items:center;padding:12px 24px;background:var(--card);border-bottom:1px solid var(--line)}
.brand{font-weight:600}nav{display:flex;flex-wrap:wrap;gap:4px}nav a{padding:4px 10px;border-radius:6px;color:var(--fg);text-decoration:none}
nav a:hover{background:var(--bg)}nav a.current{background:var(--accent);color:var(--card)}
main{max-width:1200px;margin:0 auto;padding:16px 24px 48px}h1{font-size:22px;margin:8px 0 16px}h2{font-size:17px;margin:24px 0 8px}
table{border-collapse:collapse;width:100%;background:var(--card);border:1px solid var(--line);border-radius:8px;overflow:hidden;margin-bottom:12px}
th,td{text-align:left;padding:6px 10px;border-bottom:1px solid var(--line);vertical-align:top}th{color:var(--muted);font-weight:600}
table.kv th{width:14em}.num{text-align:right;font-variant-numeric:tabular-nums}code,.mono{font-family:ui-monospace,Consolas,monospace;font-size:13px;word-break:break-all}
.muted{color:var(--muted)}.nowrap{white-space:nowrap;word-break:normal}.warn{color:var(--warn)}.ok{color:var(--ok)}.wrap{overflow-x:auto}
form.inline{display:inline}button{font:inherit;padding:4px 12px;border:1px solid var(--line);border-radius:6px;background:var(--card);color:var(--fg);cursor:pointer}
button:hover{border-color:var(--accent)}button.danger{color:var(--warn)}
form.filters{display:flex;flex-wrap:wrap;gap:8px 12px;align-items:end;margin-bottom:12px}form.filters label{display:flex;flex-direction:column;font-size:13px;color:var(--muted)}
input,select{font:inherit;padding:4px 6px;border:1px solid var(--line);border-radius:6px;background:var(--card);color:var(--fg)}
.code{font:600 40px/1.2 ui-monospace,Consolas,monospace;letter-spacing:.2em;padding:16px 24px;background:var(--card);border:1px solid var(--line);border-radius:8px;display:inline-block;margin:8px 0}
.pager{display:flex;gap:12px;margin-top:8px}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_neutralizes_markup() {
        assert_eq!(
            escape("<a href=\"x\">'&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;"
        );
    }

    #[test]
    fn url_encode_keeps_unreserved_characters() {
        assert_eq!(url_encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
        assert_eq!(url_encode("dev-1_x.y~"), "dev-1_x.y~");
    }

    #[test]
    fn formatting_helpers() {
        assert_eq!(time(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(duration(59), "59s");
        assert_eq!(duration(3_725), "1h 2m");
        assert_eq!(duration(90_000), "1d 1h");
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KiB");
    }

    #[test]
    fn page_escapes_the_title_and_marks_the_current_entry() {
        let Html(html) = page("<x>", "/admin/devices", PageOptions::default(), "");
        assert!(html.contains("&lt;x&gt;"));
        assert!(html.contains("<a href=\"/admin/devices\" class=\"current\">"));
        assert!(!html.contains("http-equiv"));
        let Html(html) = page(
            "t",
            "/",
            PageOptions {
                refresh_secs: Some(10),
            },
            "",
        );
        assert!(html.contains("<meta http-equiv=\"refresh\" content=\"10\">"));
    }
}
