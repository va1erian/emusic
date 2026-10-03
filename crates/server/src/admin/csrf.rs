//! CSRF protection for the admin page's POST forms.
//!
//! Browsers attach cached Basic credentials to cross-site form posts, so
//! every state-changing form carries a token: `<issued_at>.<hmac>` keyed by a
//! per-process random secret, valid for [`TOKEN_TTL_SECS`]. As a second
//! layer, POSTs whose `Origin` or `Sec-Fetch-Site` header shows another site
//! are refused before the token is even checked.

use axum::http::{HeaderMap, header};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// How long a form token stays valid, in seconds.
pub const TOKEN_TTL_SECS: i64 = 3600;

/// Name of the hidden form field carrying the token.
pub const FIELD: &str = "csrf";

/// Per-process HMAC key for form tokens.
pub struct CsrfKey([u8; 32]);

impl CsrfKey {
    /// A fresh random key. Tokens do not survive a restart, by design.
    pub fn generate() -> Self {
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        Self(key)
    }

    /// A token issued at `now`.
    pub fn token(&self, now: i64) -> String {
        format!("{now}.{}", hex::encode(self.mac(now)))
    }

    /// Whether `token` was issued by this key within the validity window.
    pub fn verify(&self, token: &str, now: i64) -> bool {
        let Some((issued, mac)) = token.split_once('.') else {
            return false;
        };
        let (Ok(issued), Ok(mac)) = (issued.parse::<i64>(), hex::decode(mac)) else {
            return false;
        };
        let age = now.saturating_sub(issued);
        if !(-60..=TOKEN_TTL_SECS).contains(&age) {
            return false;
        }
        let expected = self.mac(issued);
        mac.len() == expected.len() && bool::from(mac.ct_eq(&expected))
    }

    /// A hidden `<input>` carrying a fresh token.
    pub fn hidden_field(&self, now: i64) -> String {
        format!(
            "<input type=\"hidden\" name=\"{FIELD}\" value=\"{}\">",
            self.token(now)
        )
    }

    fn mac(&self, issued: i64) -> Vec<u8> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.0)
            .expect("HMAC-SHA256 accepts keys of any length");
        mac.update(b"emusic-server/admin-csrf/v1:");
        mac.update(issued.to_string().as_bytes());
        mac.finalize().into_bytes().to_vec()
    }
}

/// Whether the request's fetch metadata and `Origin` (when sent) say it came
/// from the admin page itself.
pub fn is_same_origin(headers: &HeaderMap) -> bool {
    let header_str = |name| headers.get(name).and_then(|v| v.to_str().ok());
    if let Some(site) = header_str("sec-fetch-site")
        && !matches!(site, "same-origin" | "none")
    {
        return false;
    }
    match (
        header_str(header::ORIGIN.as_str()),
        header_str(header::HOST.as_str()),
    ) {
        (None, _) => true,
        (Some(origin), Some(host)) => origin
            .split_once("://")
            .is_some_and(|(_, authority)| authority.eq_ignore_ascii_case(host)),
        (Some(_), None) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_verify_within_their_window_only() {
        let key = CsrfKey::generate();
        let token = key.token(1_000);
        assert!(key.verify(&token, 1_000));
        assert!(key.verify(&token, 1_000 + TOKEN_TTL_SECS));
        assert!(!key.verify(&token, 1_001 + TOKEN_TTL_SECS));
        assert!(!CsrfKey::generate().verify(&token, 1_000));
        assert!(!key.verify("1000.00", 1_000));
        assert!(!key.verify("garbage", 1_000));
        let forged = format!("2000{}", &token[4..]);
        assert!(!key.verify(&forged, 2_000));
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, value.parse().unwrap());
        }
        map
    }

    #[test]
    fn cross_site_posts_are_refused() {
        assert!(is_same_origin(&headers(&[("host", "127.0.0.1:8081")])));
        assert!(is_same_origin(&headers(&[
            ("host", "127.0.0.1:8081"),
            ("origin", "http://127.0.0.1:8081"),
            ("sec-fetch-site", "same-origin"),
        ])));
        assert!(!is_same_origin(&headers(&[
            ("host", "127.0.0.1:8081"),
            ("origin", "https://evil.example"),
        ])));
        assert!(!is_same_origin(&headers(&[
            ("host", "127.0.0.1:8081"),
            ("sec-fetch-site", "cross-site"),
        ])));
        assert!(!is_same_origin(&headers(&[
            ("host", "127.0.0.1:8081"),
            ("origin", "null"),
        ])));
    }
}
