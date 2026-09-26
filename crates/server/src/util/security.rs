use std::path::{Path, PathBuf};
use sha2::Sha256;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub device_id: String,
    pub iat: u64,
    pub exp: u64,
}

pub struct TokenManager {
    secret: Vec<u8>,
}

impl TokenManager {
    pub fn new(secret: Vec<u8>) -> Self {
        Self { secret }
    }

    pub fn generate_token(&self, device_id: &str, ttl_secs: u64) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let claims = AuthClaims {
            device_id: device_id.to_string(),
            iat: now,
            exp: now + ttl_secs,
        };

        let json = serde_json::to_string(&claims).unwrap_or_default();
        let payload_b64 = URL_SAFE_NO_PAD.encode(json.as_bytes());

        let mut mac = HmacSha256::new_from_slice(&self.secret).expect("HMAC init");
        mac.update(payload_b64.as_bytes());
        let sig = mac.finalize().into_bytes();
        let sig_b64 = URL_SAFE_NO_PAD.encode(sig);

        format!("v4.local.{}.{}", payload_b64, sig_b64)
    }

    pub fn verify_token(&self, token: &str) -> Result<AuthClaims, &'static str> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 4 || parts[0] != "v4" || parts[1] != "local" {
            return Err("Invalid token format");
        }

        let payload_b64 = parts[2];
        let sig_b64 = parts[3];

        let mut mac = HmacSha256::new_from_slice(&self.secret).expect("HMAC init");
        mac.update(payload_b64.as_bytes());

        let actual_sig = URL_SAFE_NO_PAD
            .decode(sig_b64)
            .map_err(|_| "Invalid signature encoding")?;

        mac.verify_slice(&actual_sig)
            .map_err(|_| "Invalid token signature")?;

        let payload_bytes = URL_SAFE_NO_PAD
            .decode(payload_b64)
            .map_err(|_| "Invalid payload encoding")?;

        let claims: AuthClaims = serde_json::from_slice(&payload_bytes)
            .map_err(|_| "Invalid payload JSON")?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now > claims.exp {
            return Err("Token expired");
        }

        Ok(claims)
    }
}

/// Sanitizes relative_path and verifies it resides strictly inside base_root.
/// Prevents path traversal vulnerabilities.
pub fn sanitize_and_resolve_path(base_root: &Path, relative_path: &str) -> Option<PathBuf> {
    if relative_path.contains('\0') || relative_path.contains('\\') {
        return None;
    }

    let target = base_root.join(relative_path);
    let canonical_root = base_root.canonicalize().ok()?;
    let canonical_target = target.canonicalize().ok()?;

    if canonical_target.starts_with(&canonical_root) {
        Some(canonical_target)
    } else {
        None
    }
}

/// Searches through all configured root paths to resolve a relative track path safely.
pub fn resolve_path_in_roots(roots: &[PathBuf], relative_path: &str) -> Option<PathBuf> {
    for root in roots {
        if let Some(resolved) = sanitize_and_resolve_path(root, relative_path) {
            return Some(resolved);
        }
    }
    None
}

/// Generates a 6-digit numeric pairing code.
pub fn generate_pairing_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let code: u32 = rng.gen_range(100_000..999_999);
    code.to_string()
}
