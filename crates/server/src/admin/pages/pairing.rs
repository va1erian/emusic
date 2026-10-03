//! Pairing page and the generate-code action.

use std::sync::Arc;

use axum::Form;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, Response};

use super::{CsrfForm, PeerIp, reject_forged, render};
use crate::admin::AdminState;
use crate::admin::data::{self, PairingStatus, blocking};
use crate::admin::html::{self, PageOptions, escape};
use crate::audit::Actor;
use crate::auth::pairing::generate_pairing_code;
use crate::error::Result;
use crate::util::unix_now;

/// `GET /admin/pairing`
pub async fn page(State(admin): State<AdminState>) -> Response {
    render(build(&admin, None).await)
}

/// `POST /admin/pairing/codes`: mints a code and shows it once. The response
/// is `no-store` like every admin response, so the code is not cached.
pub async fn generate(
    State(admin): State<AdminState>,
    PeerIp(ip): PeerIp,
    headers: HeaderMap,
    Form(form): Form<CsrfForm>,
) -> Response {
    if let Some(rejection) = reject_forged(&admin, &headers, &form.csrf) {
        return rejection;
    }
    render(mint(&admin, &ip).await)
}

async fn mint(admin: &AdminState, ip: &str) -> Result<Html<String>> {
    let db = admin.app.db.clone();
    let keys = Arc::clone(&admin.app.keys);
    let ttl = admin.app.config.security.pairing_code_ttl_secs;
    let code = blocking(move || generate_pairing_code(&db, &keys, ttl, unix_now())).await?;
    admin.app.audit.pairing_code_created(ip, &Actor::Admin, ttl);
    build(admin, Some(code)).await
}

async fn build(admin: &AdminState, code: Option<String>) -> Result<Html<String>> {
    let status = data::pairing(&admin.app).await?;
    let csrf = admin.csrf.hidden_field(unix_now());
    Ok(html::page(
        "Pairing",
        "/admin/pairing",
        PageOptions::default(),
        &body(&status, code.as_deref(), &csrf),
    ))
}

fn body(status: &PairingStatus, code: Option<&str>, csrf: &str) -> String {
    let mut out = String::new();
    if let Some(code) = code {
        out.push_str(&format!(
            "<h2>New pairing code</h2><div class=\"code\">{}</div>\
             <p>Enter it in the emusic client within {}. It works once and is not \
             shown again.</p>",
            escape(code),
            html::duration(status.code_ttl_secs as i64)
        ));
    }
    out.push_str(&html::key_values(&[
        ("Outstanding codes", status.codes.outstanding.to_string()),
        ("Used codes", status.codes.used.to_string()),
        ("Expired codes", status.codes.expired.to_string()),
        (
            "New code lifetime",
            html::duration(status.code_ttl_secs as i64),
        ),
    ]));
    out.push_str(&format!(
        "<form method=\"post\" action=\"/admin/pairing/codes\">{csrf}\
         <button type=\"submit\">Generate code</button></form>"
    ));
    out
}
