//! Database page and the on-demand integrity check.

use std::fmt::Write as _;

use axum::Form;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, Response};

use super::{CsrfForm, PeerIp, reject_forged, render};
use crate::admin::AdminState;
use crate::admin::data::{self, DatabaseReport, blocking};
use crate::admin::html::{self, PageOptions, escape};
use crate::admin::stats;
use crate::error::Result;
use crate::util::unix_now;

/// `GET /admin/database`
pub async fn page(State(admin): State<AdminState>) -> Response {
    render(build(&admin, None).await)
}

/// `POST /admin/database/integrity-check`: runs the full (slow) check.
pub async fn integrity_check(
    State(admin): State<AdminState>,
    PeerIp(ip): PeerIp,
    headers: HeaderMap,
    Form(form): Form<CsrfForm>,
) -> Response {
    if let Some(rejection) = reject_forged(&admin, &headers, &form.csrf) {
        return rejection;
    }
    render(run_check(&admin, &ip).await)
}

async fn run_check(admin: &AdminState, ip: &str) -> Result<Html<String>> {
    let db = admin.app.db.clone();
    let lines = blocking(move || stats::check_integrity(&db, true)).await?;
    let ok = lines == ["ok"];
    admin.app.audit.admin_integrity_check(ip, ok);
    build(admin, Some(lines)).await
}

async fn build(admin: &AdminState, integrity: Option<Vec<String>>) -> Result<Html<String>> {
    let report = data::database(&admin.app).await?;
    let csrf = admin.csrf.hidden_field(unix_now());
    Ok(html::page(
        "Database",
        "/admin/database",
        PageOptions::default(),
        &body(&report, integrity.as_deref(), &csrf),
    ))
}

fn check_result(lines: &[String]) -> String {
    if lines == ["ok"] {
        return "<span class=\"ok\">ok</span>".to_string();
    }
    let mut out = String::from("<span class=\"warn\">problems found:</span><br>");
    for line in lines {
        let _ = write!(out, "<code>{}</code><br>", escape(line));
    }
    out
}

fn body(report: &DatabaseReport, integrity: Option<&[String]>, csrf: &str) -> String {
    let pages = &report.pages;
    let integrity_row = match integrity {
        Some(lines) => check_result(lines),
        None => "<span class=\"muted\">not run</span>".to_string(),
    };
    let mut out = String::from("<h2>Health</h2>");
    out.push_str(&html::key_values(&[
        ("quick_check", check_result(&report.quick_check)),
        ("integrity_check", integrity_row),
        (
            "Files",
            format!(
                "{} database + {} WAL",
                html::bytes(report.files.db_bytes),
                html::bytes(report.files.wal_bytes)
            ),
        ),
        (
            "Pages",
            format!(
                "{} × {} ({} free)",
                pages.page_count,
                html::bytes(pages.page_size.max(0) as u64),
                pages.freelist_count
            ),
        ),
    ]));
    out.push_str(&format!(
        "<form method=\"post\" action=\"/admin/database/integrity-check\">{csrf}\
         <button type=\"submit\">Run full integrity check</button> \
         <span class=\"muted\">reads the whole database; may take a while</span></form>"
    ));

    out.push_str("<h2>Tables</h2><table><tr><th>Table</th><th class=\"num\">Rows</th></tr>");
    for table in &report.tables {
        let _ = write!(
            out,
            "<tr><td class=\"mono\">{}</td><td class=\"num\">{}</td></tr>",
            escape(&table.table),
            table.rows
        );
    }
    out.push_str("</table><h2>Tombstones and pairing codes</h2>");
    let codes = &report.pairing_codes;
    out.push_str(&html::key_values(&[
        ("Tombstones", report.tombstones.count.to_string()),
        (
            "Oldest tombstone",
            report
                .tombstones
                .oldest_deleted_at
                .map_or_else(|| "none".to_string(), html::time),
        ),
        (
            "Pairing code rows",
            format!(
                "{} total · {} outstanding · {} used · {} expired",
                codes.total, codes.outstanding, codes.used, codes.expired
            ),
        ),
    ]));
    out
}
