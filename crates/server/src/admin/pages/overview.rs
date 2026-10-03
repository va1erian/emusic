//! Overview page.

use std::fmt::Write as _;

use axum::extract::State;
use axum::response::{Html, Response};

use super::render;
use crate::admin::AdminState;
use crate::admin::data::{self, Overview};
use crate::admin::html::{self, PageOptions, escape};
use crate::error::Result;

/// `GET /admin/`
pub async fn page(State(admin): State<AdminState>) -> Response {
    render(build(&admin).await)
}

async fn build(admin: &AdminState) -> Result<Html<String>> {
    let overview = data::overview(&admin.app).await?;
    Ok(html::page(
        "Overview",
        "/admin/",
        PageOptions::default(),
        &body(&overview),
    ))
}

fn body(o: &Overview) -> String {
    let scan = &o.scan;
    let scan_state = if scan.running {
        format!("running ({} files so far)", scan.files_found)
    } else {
        "idle".to_string()
    };
    let last_scan = match &scan.last_report {
        Some(last) => format!(
            "{} · {} files, {} changed, {} deleted, {}{}",
            html::time(last.finished_at),
            last.files_found,
            last.changed,
            last.deleted,
            html::duration((last.elapsed_ms / 1000) as i64),
            if last.partial {
                " · <span class=\"warn\">partial</span>"
            } else {
                ""
            }
        ),
        None => "none since startup".to_string(),
    };
    let render_cache = match (o.render_cache_bytes, o.render_cache_max_bytes) {
        (Some(used), Some(max)) => format!("{} of {}", html::bytes(used), html::bytes(max)),
        _ => "<span class=\"muted\">rendering disabled</span>".to_string(),
    };
    let dropped = if o.audit_events_dropped == 0 {
        "0".to_string()
    } else {
        format!("<span class=\"warn\">{}</span>", o.audit_events_dropped)
    };
    let mut out = html::key_values(&[
        ("Version", escape(o.version)),
        (
            "Uptime",
            format!(
                "{} (since {})",
                html::duration(o.uptime_secs),
                html::time(o.started_at)
            ),
        ),
        ("Schema version", o.schema_version.to_string()),
        ("Library version", o.library_version.to_string()),
        (
            "Database size",
            format!(
                "{} + {} WAL",
                html::bytes(o.files.db_bytes),
                html::bytes(o.files.wal_bytes)
            ),
        ),
        ("Scan", scan_state),
        ("Last scan", last_scan),
        ("Render cache", render_cache),
        (
            "Active devices",
            format!("<a href=\"/admin/active\">{}</a>", o.active_devices),
        ),
        ("Audit events dropped", dropped),
        (
            "Audit events throttled",
            o.audit_events_throttled.to_string(),
        ),
    ]);

    out.push_str("<h2>Library roots</h2><table><tr><th class=\"num\">#</th><th>Path</th></tr>");
    for (index, root) in o.roots.iter().enumerate() {
        let _ = write!(
            out,
            "<tr><td class=\"num\">{index}</td><td class=\"mono\">{}</td></tr>",
            escape(root)
        );
    }
    out.push_str(
        "</table><h2>Rows per table</h2><table><tr><th>Table</th><th class=\"num\">Rows</th></tr>",
    );
    for table in &o.tables {
        let _ = write!(
            out,
            "<tr><td class=\"mono\">{}</td><td class=\"num\">{}</td></tr>",
            escape(&table.table),
            table.rows
        );
    }
    out.push_str("</table>");
    out
}
