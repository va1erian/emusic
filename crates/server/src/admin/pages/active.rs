//! Active users page.

use std::fmt::Write as _;

use axum::extract::State;
use axum::response::{Html, Response};

use super::render;
use crate::activity::ACTIVE_WINDOW_SECS;
use crate::admin::AdminState;
use crate::admin::data::{self, ActiveDevice};
use crate::admin::html::{self, PageOptions, escape};
use crate::error::Result;
use crate::util::unix_now;

/// Seconds between automatic reloads.
const REFRESH_SECS: u32 = 10;

/// `GET /admin/active`
pub async fn page(State(admin): State<AdminState>) -> Response {
    render(build(&admin).await)
}

async fn build(admin: &AdminState) -> Result<Html<String>> {
    let active = data::active(&admin.app).await?;
    Ok(html::page(
        "Active users",
        "/admin/active",
        PageOptions {
            refresh_secs: Some(REFRESH_SECS),
        },
        &body(&active, unix_now()),
    ))
}

fn body(active: &[ActiveDevice], now: i64) -> String {
    let mut out = format!(
        "<p class=\"muted\">Devices with a request in the last {} minutes or an open \
         WebSocket. Refreshes every {REFRESH_SECS} s.</p>",
        ACTIVE_WINDOW_SECS / 60
    );
    if active.is_empty() {
        out.push_str("<p>No active devices.</p>");
        return out;
    }
    out.push_str(
        "<div class=\"wrap\"><table><tr><th>Device</th><th>Last request</th><th>Client IP</th>\
         <th>User agent</th><th class=\"num\">WebSockets</th><th>Last track</th></tr>",
    );
    for device in active {
        let a = &device.activity;
        let track = match (&a.last_track_id, &device.last_track) {
            (Some(_), Some(label)) => format!(
                "{}<br><span class=\"muted\">{} ago</span>",
                escape(label),
                html::duration(now.saturating_sub(a.last_track_at.unwrap_or(now)))
            ),
            (Some(id), None) => format!("<span class=\"mono\">{}</span>", escape(id)),
            _ => "<span class=\"muted\">none</span>".to_string(),
        };
        let _ = write!(
            out,
            "<tr><td>{name}<br><span class=\"mono muted\">{id}</span></td>\
             <td class=\"nowrap\">{ago} ago</td><td class=\"mono nowrap\">{ip}</td><td>{ua}</td>\
             <td class=\"num\">{ws}</td><td>{track}</td></tr>",
            name = escape(&a.device_name),
            id = escape(&a.device_id),
            ago = html::duration(now.saturating_sub(a.last_request_at)),
            ip = escape(&a.client_ip),
            ua = escape(&a.user_agent),
            ws = a.open_websockets,
        );
    }
    out.push_str("</table></div>");
    out
}
