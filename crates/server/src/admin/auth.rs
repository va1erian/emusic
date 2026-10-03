//! Admin listener access control.
//!
//! - With `admin.token` set, every request needs HTTP Basic auth whose
//!   password equals the token (any user name). The comparison is constant
//!   time over SHA-256 digests, so neither content nor length leaks. Wrong or
//!   malformed credentials are audited and count against a per-IP limit;
//!   past the limit every request gets `429` until the window slides.
//! - Without a token (only allowed on loopback), the `Host` header must name
//!   an IP literal or `localhost`. That defeats DNS rebinding, where a web
//!   page re-points its own hostname at 127.0.0.1 to read the admin page
//!   from the operator's browser.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::AdminState;

/// Failed admin logins allowed per IP per [`AUTH_FAILURE_WINDOW`].
pub const AUTH_FAILURE_LIMIT: u32 = 10;

/// Window for [`AUTH_FAILURE_LIMIT`].
pub const AUTH_FAILURE_WINDOW: Duration = Duration::from_secs(60);

/// Outcome of checking an `Authorization` header against the token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicAuth {
    /// No credentials were sent (the browser has not prompted yet).
    Missing,
    /// The header is not valid Basic credentials.
    Malformed,
    /// Well-formed credentials with the wrong password.
    Wrong,
    /// The password matches the token.
    Valid,
}

impl BasicAuth {
    /// Stable label for the audit `reason` field.
    pub fn label(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Malformed => "malformed",
            Self::Wrong => "wrong",
            Self::Valid => "valid",
        }
    }
}

/// Checks `headers` for Basic credentials whose password is `token`.
pub fn check_basic(headers: &HeaderMap, token: &str) -> BasicAuth {
    let Some(value) = headers.get(header::AUTHORIZATION) else {
        return BasicAuth::Missing;
    };
    let Some((scheme, encoded)) = value.to_str().ok().and_then(|v| v.split_once(' ')) else {
        return BasicAuth::Malformed;
    };
    if !scheme.eq_ignore_ascii_case("basic") {
        return BasicAuth::Malformed;
    }
    let Ok(decoded) = STANDARD.decode(encoded.trim()) else {
        return BasicAuth::Malformed;
    };
    let Some(colon) = decoded.iter().position(|byte| *byte == b':') else {
        return BasicAuth::Malformed;
    };
    let password = &decoded[colon + 1..];
    let presented = Sha256::digest(password);
    let expected = Sha256::digest(token.as_bytes());
    if bool::from(presented.ct_eq(&expected)) {
        BasicAuth::Valid
    } else {
        BasicAuth::Wrong
    }
}

/// Whether `Host` names an IP literal or `localhost` (any port).
pub fn is_local_host_header(headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        // HTTP/1.0 without Host cannot be a rebinding attack from a browser.
        return true;
    };
    let name = if let Some(rest) = host.strip_prefix('[') {
        // IPv6 literal: `[::1]:8081`.
        return rest
            .split(']')
            .next()
            .is_some_and(|ip| ip.parse::<IpAddr>().is_ok());
    } else {
        host.rsplit_once(':').map_or(host, |(name, _port)| name)
    };
    name.eq_ignore_ascii_case("localhost")
        || name.to_ascii_lowercase().ends_with(".localhost")
        || name.parse::<IpAddr>().is_ok()
}

/// Middleware enforcing the rules in the module docs.
pub async fn require_admin(
    State(admin): State<AdminState>,
    request: Request,
    next: Next,
) -> Response {
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
        .to_string();
    let state = &admin.app;
    let config = &state.config.admin;

    if !config.requires_auth() {
        if is_local_host_header(request.headers()) {
            return next.run(request).await;
        }
        state.audit.admin_auth_failed(&ip, "host_not_local");
        return (StatusCode::FORBIDDEN, "admin page: Host must be local").into_response();
    }

    let limit_key = format!("admin-auth:{ip}");
    if state
        .rate
        .exceeded(&limit_key, AUTH_FAILURE_LIMIT, AUTH_FAILURE_WINDOW)
    {
        state.audit.rate_limited(&ip, "admin");
        return (StatusCode::TOO_MANY_REQUESTS, "too many failed logins").into_response();
    }
    match check_basic(request.headers(), &config.token) {
        BasicAuth::Valid => next.run(request).await,
        BasicAuth::Missing => unauthorized(),
        failure => {
            state
                .rate
                .check(&limit_key, AUTH_FAILURE_LIMIT, AUTH_FAILURE_WINDOW);
            state.audit.admin_auth_failed(&ip, failure.label());
            unauthorized()
        }
    }
}

fn unauthorized() -> Response {
    let mut response = (StatusCode::UNAUTHORIZED, "authentication required").into_response();
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Basic realm=\"emusic-server admin\", charset=\"UTF-8\""),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_auth(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, value.parse().unwrap());
        headers
    }

    fn basic(user_pass: &str) -> HeaderMap {
        with_auth(&format!("Basic {}", STANDARD.encode(user_pass)))
    }

    const TOKEN: &str = "correct-horse-battery-staple";

    #[test]
    fn basic_auth_accepts_any_user_with_the_token() {
        assert_eq!(
            check_basic(&basic(&format!("admin:{TOKEN}")), TOKEN),
            BasicAuth::Valid
        );
        assert_eq!(
            check_basic(&basic(&format!(":{TOKEN}")), TOKEN),
            BasicAuth::Valid
        );
    }

    #[test]
    fn basic_auth_rejects_wrong_and_malformed_credentials() {
        assert_eq!(check_basic(&HeaderMap::new(), TOKEN), BasicAuth::Missing);
        assert_eq!(check_basic(&basic("admin:nope"), TOKEN), BasicAuth::Wrong);
        assert_eq!(
            check_basic(&basic(&format!("admin:{TOKEN}x")), TOKEN),
            BasicAuth::Wrong
        );
        assert_eq!(check_basic(&basic(TOKEN), TOKEN), BasicAuth::Malformed);
        assert_eq!(
            check_basic(&with_auth("Basic !!!"), TOKEN),
            BasicAuth::Malformed
        );
        assert_eq!(
            check_basic(&with_auth(&format!("Bearer {TOKEN}")), TOKEN),
            BasicAuth::Malformed
        );
    }

    fn host(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, value.parse().unwrap());
        headers
    }

    #[test]
    fn only_local_host_headers_pass_without_a_token() {
        for ok in [
            "127.0.0.1:8081",
            "localhost:8081",
            "localhost",
            "[::1]:8081",
            "10.0.0.2",
        ] {
            assert!(is_local_host_header(&host(ok)), "{ok}");
        }
        for bad in [
            "evil.example:8081",
            "music.chaton.lol",
            "127.0.0.1.nip.io:8081",
        ] {
            assert!(!is_local_host_header(&host(bad)), "{bad}");
        }
        assert!(is_local_host_header(&HeaderMap::new()));
    }
}
