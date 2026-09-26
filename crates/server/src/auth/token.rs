//! PASETO v4 local token generation and validation.

use pasetors::Local;
use pasetors::claims::{Claims, ClaimsValidationRules};
use pasetors::keys::{Generate, SymmetricKey};
use pasetors::local;
use pasetors::token::UntrustedToken;
use pasetors::version4::V4;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("PASETO token error: {0}")]
    Paseto(#[from] pasetors::errors::Error),
    #[error("Device is revoked or unknown")]
    DeviceRevoked,
    #[error("Missing or invalid Authorization header")]
    InvalidHeader,
}

#[derive(Clone)]
pub struct TokenManager {
    key: SymmetricKey<V4>,
}

impl TokenManager {
    pub fn new(key_bytes: &[u8; 32]) -> Result<Self, AuthError> {
        let key = SymmetricKey::<V4>::from(key_bytes)?;
        Ok(Self { key })
    }

    pub fn generate_random() -> (Self, [u8; 32]) {
        let key = SymmetricKey::<V4>::generate().expect("generate key");
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(key.as_bytes());
        (Self { key }, bytes)
    }

    pub fn issue_token(&self, device_id: &str, ttl_hours: u64) -> Result<String, AuthError> {
        let mut claims = Claims::new()?;
        claims.issuer("emusic-server")?;
        claims.subject(device_id)?;

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let exp_secs = now_secs + (ttl_hours * 3600);

        let exp_iso = format_iso8601(exp_secs);
        claims.expiration(&exp_iso)?;

        let token = local::encrypt(&self.key, &claims, None, None)?;
        Ok(token)
    }

    pub fn verify_token(&self, token_str: &str) -> Result<String, AuthError> {
        let untrusted = UntrustedToken::<Local, V4>::try_from(token_str)?;
        let validation_rules = ClaimsValidationRules::new();
        let trusted = local::decrypt(&self.key, &untrusted, &validation_rules, None, None)?;

        let payload_json = trusted.payload();
        let val: serde_json::Value = serde_json::from_str(payload_json)
            .map_err(|_| pasetors::errors::Error::InvalidClaim)?;

        let sub = val
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or(pasetors::errors::Error::InvalidClaim)?;

        Ok(sub.to_string())
    }
}

fn format_iso8601(secs: u64) -> String {
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = rem / 3600;
    let mins = (rem % 3600) / 60;
    let seconds = rem % 60;

    let (year, month, day) = days_to_date(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{mins:02}:{seconds:02}Z")
}

fn days_to_date(days_since_epoch: u64) -> (u64, u64, u64) {
    let z = days_since_epoch + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_issue_and_verify() {
        let (tm, _) = TokenManager::generate_random();
        let token = tm.issue_token("device-abc", 24).unwrap();
        assert!(token.starts_with("v4.local."));

        let verified_device_id = tm.verify_token(&token).unwrap();
        assert_eq!(verified_device_id, "device-abc");
    }
}
