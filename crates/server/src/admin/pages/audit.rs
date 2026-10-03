//! Audit log page: paginated, filterable by event, device and time range.

use std::fmt::Write as _;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use time::{Date, Month, PrimitiveDateTime, Time};

use super::render;
use crate::admin::AdminState;
use crate::admin::data::{self, AuditPage};
use crate::admin::html::{self, PageOptions, escape, url_encode};
use crate::audit::{AuditQuery, KNOWN_EVENTS};
use crate::error::Result;

/// Rows per page by default.
const DEFAULT_PAGE: u32 = 100;

/// Query-string filters. Everything is a string so an empty form field means
/// "no filter" rather than a parse error.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct AuditFilter {
    /// Exact event name.
    pub event: Option<String>,
    /// Exact device id.
    pub device: Option<String>,
    /// Start (inclusive): `YYYY-MM-DD`, `YYYY-MM-DDTHH:MM[:SS]` (UTC) or Unix seconds.
    pub since: Option<String>,
    /// End: same formats; a bare date includes that whole day.
    pub until: Option<String>,
    /// Keyset cursor from the previous page.
    pub before: Option<String>,
    /// Page size (1–500).
    pub limit: Option<String>,
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

impl AuditFilter {
    /// Converts the form into a store query, or a message for a `400`.
    pub fn to_query(&self) -> std::result::Result<AuditQuery, String> {
        let number = |value: Option<&str>, name: &str| {
            value
                .map(|v| v.parse::<i64>().map_err(|_| format!("invalid {name}")))
                .transpose()
        };
        Ok(AuditQuery {
            event: non_empty(&self.event).map(str::to_string),
            device_id: non_empty(&self.device).map(str::to_string),
            since: non_empty(&self.since)
                .map(|v| parse_time(v, false).ok_or("invalid since"))
                .transpose()?,
            until: non_empty(&self.until)
                .map(|v| parse_time(v, true).ok_or("invalid until"))
                .transpose()?,
            before_id: number(non_empty(&self.before), "before")?,
            limit: number(non_empty(&self.limit), "limit")?
                .map_or(DEFAULT_PAGE, |n| n.clamp(1, 500) as u32),
        })
    }

    /// The filter as a query string without the cursor.
    fn query_string(&self) -> String {
        [
            ("event", &self.event),
            ("device", &self.device),
            ("since", &self.since),
            ("until", &self.until),
            ("limit", &self.limit),
        ]
        .iter()
        .filter_map(|(key, value)| non_empty(value).map(|v| format!("{key}={}", url_encode(v))))
        .collect::<Vec<_>>()
        .join("&")
    }
}

/// Parses a UTC date, date-time or Unix timestamp. With `end_of_day`, a bare
/// date maps to the start of the following day.
pub fn parse_time(value: &str, end_of_day: bool) -> Option<i64> {
    if let Ok(unix) = value.parse::<i64>() {
        return Some(unix);
    }
    let (date, time) = match value.split_once(['T', ' ']) {
        Some((date, time)) => (date, Some(time)),
        None => (value, None),
    };
    let mut parts = date.splitn(3, '-').map(str::parse::<i64>);
    let (Some(Ok(year)), Some(Ok(month)), Some(Ok(day))) =
        (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let date = Date::from_calendar_date(
        i32::try_from(year).ok()?,
        Month::try_from(u8::try_from(month).ok()?).ok()?,
        u8::try_from(day).ok()?,
    )
    .ok()?;
    let clock = match time {
        Some(time) => {
            let mut hms = time.splitn(3, ':').map(str::parse::<u8>);
            let hour = hms.next()?.ok()?;
            let minute = hms.next().unwrap_or(Ok(0)).ok()?;
            let second = hms.next().unwrap_or(Ok(0)).ok()?;
            Time::from_hms(hour, minute, second).ok()?
        }
        None => Time::MIDNIGHT,
    };
    let at = PrimitiveDateTime::new(date, clock)
        .assume_utc()
        .unix_timestamp();
    Some(if end_of_day && time.is_none() {
        at + 86_400
    } else {
        at
    })
}

/// `GET /admin/audit`
pub async fn page(State(admin): State<AdminState>, Query(filter): Query<AuditFilter>) -> Response {
    let query = match filter.to_query() {
        Ok(query) => query,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };
    render(build(&admin, &filter, query).await)
}

async fn build(
    admin: &AdminState,
    filter: &AuditFilter,
    query: AuditQuery,
) -> Result<Html<String>> {
    let page = data::audit(&admin.app, query).await?;
    Ok(html::page(
        "Audit log",
        "/admin/audit",
        PageOptions::default(),
        &body(filter, &page),
    ))
}

fn filter_form(filter: &AuditFilter, events: &[String]) -> String {
    let selected = non_empty(&filter.event).unwrap_or_default();
    let mut names: Vec<&str> = KNOWN_EVENTS.to_vec();
    names.extend(events.iter().map(String::as_str));
    names.sort_unstable();
    names.dedup();
    let mut options = String::from("<option value=\"\">any</option>");
    for name in names {
        let mark = if name == selected { " selected" } else { "" };
        let _ = write!(options, "<option{mark}>{}</option>", escape(name));
    }
    let field = |value: &Option<String>| escape(non_empty(value).unwrap_or_default());
    format!(
        "<form class=\"filters\" method=\"get\" action=\"/admin/audit\">\
         <label>Event<select name=\"event\">{options}</select></label>\
         <label>Device id<input name=\"device\" value=\"{device}\" size=\"38\"></label>\
         <label>Since (UTC)<input name=\"since\" value=\"{since}\" placeholder=\"2026-01-31 or 2026-01-31T08:00\"></label>\
         <label>Until (UTC)<input name=\"until\" value=\"{until}\" placeholder=\"2026-02-01\"></label>\
         <button type=\"submit\">Filter</button> <a href=\"/admin/audit\">Reset</a></form>",
        device = field(&filter.device),
        since = field(&filter.since),
        until = field(&filter.until),
    )
}

fn body(filter: &AuditFilter, page: &AuditPage) -> String {
    let mut out = filter_form(filter, &page.events);
    if page.rows.is_empty() {
        out.push_str("<p>No matching events.</p>");
        return out;
    }
    out.push_str(
        "<div class=\"wrap\"><table><tr><th>Time</th><th>Event</th><th>Client</th>\
         <th>Device</th><th>Detail</th></tr>",
    );
    for row in &page.rows {
        let device = match &row.device_id {
            Some(id) => format!(
                "<a class=\"mono\" href=\"/admin/audit?device={}\">{}</a>",
                url_encode(id),
                escape(id)
            ),
            None => String::new(),
        };
        let _ = write!(
            out,
            "<tr><td class=\"nowrap\">{time}</td>\
             <td><a href=\"/admin/audit?event={event_q}\">{event}</a></td>\
             <td class=\"mono nowrap\">{ip}</td><td>{device}</td><td class=\"mono\">{detail}</td></tr>",
            time = html::time(row.at),
            event_q = url_encode(&row.event),
            event = escape(&row.event),
            ip = escape(&row.client_ip),
            detail = escape(&row.detail.to_string()),
        );
    }
    out.push_str("</table></div><div class=\"pager\">");
    let base = filter.query_string();
    let sep = if base.is_empty() { "" } else { "&" };
    if filter.before.is_some() {
        let _ = write!(out, "<a href=\"/admin/audit?{base}\">« Newest</a>");
    }
    if let Some(before) = page.next_before_id {
        let _ = write!(
            out,
            "<a href=\"/admin/audit?{base}{sep}before={before}\">Older »</a>"
        );
    }
    out.push_str("</div>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_parse_as_utc() {
        assert_eq!(parse_time("1970-01-02", false), Some(86_400));
        assert_eq!(parse_time("1970-01-02", true), Some(2 * 86_400));
        assert_eq!(parse_time("1970-01-01T01:02", true), Some(3_720));
        assert_eq!(parse_time("1970-01-01 00:00:05", false), Some(5));
        assert_eq!(parse_time("12345", false), Some(12_345));
        assert_eq!(parse_time("2026-13-01", false), None);
        assert_eq!(parse_time("yesterday", false), None);
    }

    #[test]
    fn empty_fields_mean_no_filter() {
        let filter = AuditFilter {
            event: Some(String::new()),
            device: Some("  ".into()),
            before: Some(String::new()),
            ..AuditFilter::default()
        };
        let query = filter.to_query().unwrap();
        assert_eq!(query.event, None);
        assert_eq!(query.device_id, None);
        assert_eq!(query.limit, DEFAULT_PAGE);
        assert!(
            AuditFilter {
                before: Some("x".into()),
                ..AuditFilter::default()
            }
            .to_query()
            .is_err()
        );
    }
}
