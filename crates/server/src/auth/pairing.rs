//! Device pairing: one-time codes and key registration.

use std::convert::TryFrom;
use std::time::Duration;

use pasetors::keys::AsymmetricPublicKey;
use pasetors::version4::V4;
use rand::RngCore;

use crate::auth::keys::ServerKey;
use crate::auth::paseto::{IssuedToken, issue_access_token};
use crate::db::Db;
use crate::db::devices::{hash_pairing_code, is_valid_pairing_code_format};
use crate::db::models::Device;
use crate::error::{Result, ServerError};

/// Successful pairing result.
#[derive(Debug, Clone)]
pub struct PairOutcome {
    /// The newly registered device.
    pub device: Device,
    /// Its first access token.
    pub token: IssuedToken,
}

/// Generates, stores and returns a fresh one-time pairing code.
///
/// Only the SHA-256 of the code is persisted, so a database leak does not
/// reveal usable codes.
pub fn generate_pairing_code(db: &Db, ttl_secs: u64, now: i64) -> Result<String> {
    let mut rng = rand::rngs::OsRng;
    let code = format!("{:06}", rng.next_u32() % 1_000_000);
    let hash = hash_pairing_code(&code);
    db.insert_pairing_code(&hash, now, now.saturating_add(ttl_secs as i64))?;
    Ok(code)
}

/// Validates a pairing request and, on success, registers the device.
///
/// A wrong, expired or already-used code all yield [`ServerError::InvalidPairingCode`]
/// so callers cannot distinguish them.
pub fn pair(
    db: &Db,
    key: &ServerKey,
    token_ttl: Duration,
    pairing_code: &str,
    device_name: &str,
    public_key: &str,
    now: i64,
) -> Result<PairOutcome> {
    let name = validate_device_name(device_name)?;
    if !is_valid_pairing_code_format(pairing_code) {
        return Err(ServerError::InvalidPairingCode);
    }
    let parsed_key = parse_public_key(public_key)?;

    if !db.consume_pairing_code(&hash_pairing_code(pairing_code), now)? {
        return Err(ServerError::InvalidPairingCode);
    }

    let canonical_key = crate::auth::paseto::public_key_paserk(&parsed_key)?;
    let device = Device {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        public_key: canonical_key,
        paired_at: now,
        last_seen: None,
        is_revoked: false,
    };
    db.insert_device(&device)?;
    let token = issue_access_token(key, &device.id, token_ttl)?;
    Ok(PairOutcome { device, token })
}

/// Ensures the device name is a short, printable, non-empty string.
pub fn validate_device_name(name: &str) -> Result<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ServerError::Unauthorized("device name is empty".into()));
    }
    if trimmed.chars().count() > 64 {
        return Err(ServerError::Unauthorized(
            "device name is longer than 64 characters".into(),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(ServerError::Unauthorized(
            "device name contains control characters".into(),
        ));
    }
    Ok(trimmed)
}

fn parse_public_key(public_key: &str) -> Result<AsymmetricPublicKey<V4>> {
    AsymmetricPublicKey::<V4>::try_from(public_key.trim()).map_err(|_| {
        ServerError::Unauthorized("device public key is not a valid PASERK key".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_codes_are_six_digits() {
        for _ in 0..50 {
            let mut rng = rand::rngs::OsRng;
            let code = format!("{:06}", rng.next_u32() % 1_000_000);
            assert_eq!(code.len(), 6);
            assert!(code.bytes().all(|byte| byte.is_ascii_digit()));
        }
    }

    #[test]
    fn device_name_validation() {
        assert!(validate_device_name("Living Room").is_ok());
        assert!(validate_device_name("  ").is_err());
        assert!(validate_device_name(&"x".repeat(65)).is_err());
        assert!(validate_device_name("bad\u{7}name").is_err());
    }
}
