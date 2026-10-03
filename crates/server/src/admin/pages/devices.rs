//! Devices page and the revoke action.

use std::fmt::Write as _;

use axum::Form;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};

use super::{CsrfForm, PeerIp, reject_forged, render};
use crate::admin::data::{self, blocking};
use crate::admin::html::{self, PageOptions, escape, url_encode};
use crate::admin::{AdminState, internal_error};
use crate::api::auth_routes::DeviceView;
use crate::audit::Actor;
use crate::error::Result;
use crate::util::unix_now;

/// `GET /admin/devices`
pub async fn page(State(admin): State<AdminState>) -> Response {
    render(build(&admin).await)
}

/// `POST /admin/devices/{id}/revoke`
pub async fn revoke(
    State(admin): State<AdminState>,
    PeerIp(ip): PeerIp,
    Path(id): Path<String>,
    headers: HeaderMap,
    Form(form): Form<CsrfForm>,
) -> Response {
    if let Some(rejection) = reject_forged(&admin, &headers, &form.csrf) {
        return rejection;
    }
    let db = admin.app.db.clone();
    let target = id.clone();
    match blocking(move || db.revoke_device(&target)).await {
        Ok(true) => {
            admin.app.audit.device_revoked(&ip, &id, &Actor::Admin);
            Redirect::to("/admin/devices").into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, "no such active device").into_response(),
        Err(error) => internal_error(&error),
    }
}

async fn build(admin: &AdminState) -> Result<Html<String>> {
    let devices = data::devices(&admin.app).await?;
    let csrf = admin.csrf.hidden_field(unix_now());
    Ok(html::page(
        "Devices",
        "/admin/devices",
        PageOptions::default(),
        &body(&devices, &csrf),
    ))
}

fn body(devices: &[DeviceView], csrf: &str) -> String {
    if devices.is_empty() {
        return "<p>No paired devices yet. Generate a code on the \
                <a href=\"/admin/pairing\">Pairing</a> page."
            .to_string();
    }
    let mut out = String::from(
        "<div class=\"wrap\"><table><tr><th>Name</th><th>Id</th><th>Paired</th>\
         <th>Last seen</th><th>Status</th><th></th></tr>",
    );
    for device in devices {
        let (status, action) = if device.is_revoked {
            ("<span class=\"warn\">revoked</span>", String::new())
        } else {
            (
                "<span class=\"ok\">active</span>",
                format!(
                    "<form class=\"inline\" method=\"post\" action=\"/admin/devices/{}/revoke\">\
                     {csrf}<button class=\"danger\" type=\"submit\">Revoke</button></form>",
                    url_encode(&device.id)
                ),
            )
        };
        let _ = write!(
            out,
            "<tr><td>{name}</td><td class=\"mono\"><a href=\"/admin/audit?device={id_q}\">{id}</a></td>\
             <td>{paired}</td><td>{seen}</td><td>{status}</td><td>{action}</td></tr>",
            name = escape(&device.name),
            id_q = url_encode(&device.id),
            id = escape(&device.id),
            paired = html::time(device.paired_at),
            seen = html::time_or_never(device.last_seen),
        );
    }
    out.push_str("</table></div>");
    out
}
