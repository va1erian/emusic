//! Device and pairing-code persistence.

use rusqlite::OptionalExtension;
use subtle::ConstantTimeEq;

use crate::db::Db;
use crate::db::models::Device;
use crate::error::{Result, ServerError};

impl Db {
    /// Inserts a newly paired device.
    pub fn insert_device(&self, device: &Device) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO devices (id, name, public_key, paired_at, last_seen, is_revoked)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                device.id,
                device.name,
                device.public_key,
                device.paired_at,
                device.last_seen,
                device.is_revoked as i64,
            ],
        )?;
        Ok(())
    }

    /// Fetches a device by id.
    pub fn device_by_id(&self, id: &str) -> Result<Option<Device>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, public_key, paired_at, last_seen, is_revoked
             FROM devices WHERE id = ?1",
        )?;
        let device = stmt.query_row([id], row_to_device).optional()?;
        Ok(device)
    }

    /// Lists every paired device, newest first.
    pub fn list_devices(&self) -> Result<Vec<Device>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, public_key, paired_at, last_seen, is_revoked
             FROM devices ORDER BY paired_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_device)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Revokes a device. Returns `true` when a row was affected.
    pub fn revoke_device(&self, id: &str) -> Result<bool> {
        let conn = self.conn()?;
        let affected = conn.execute(
            "UPDATE devices SET is_revoked = 1 WHERE id = ?1 AND is_revoked = 0",
            [id],
        )?;
        Ok(affected > 0)
    }

    /// Records that a device authenticated successfully at `now`.
    pub fn touch_device(&self, id: &str, now: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE devices SET last_seen = ?2 WHERE id = ?1",
            rusqlite::params![id, now],
        )?;
        Ok(())
    }

    /// Stores the hash of a freshly generated pairing code.
    pub fn insert_pairing_code(
        &self,
        code_hash: &str,
        created_at: i64,
        expires_at: i64,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO pairing_codes (code_hash, created_at, expires_at, used)
             VALUES (?1, ?2, ?3, 0)",
            rusqlite::params![code_hash, created_at, expires_at],
        )?;
        Ok(())
    }

    /// Number of pairing codes that are neither used nor expired.
    pub fn active_pairing_codes(&self, now: i64) -> Result<u32> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pairing_codes WHERE used = 0 AND expires_at > ?1",
            [now],
            |row| row.get(0),
        )?;
        Ok(count as u32)
    }

    /// Consumes a valid pairing code, comparing hashes in constant time.
    ///
    /// Returns `false` for unknown, expired or already-used codes without
    /// revealing which of those applies.
    pub fn consume_pairing_code(&self, code_hash: &str, now: i64) -> Result<bool> {
        let expected = decode_hash(code_hash);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, code_hash FROM pairing_codes WHERE used = 0 AND expires_at > ?1",
        )?;
        let candidates = stmt
            .query_map([now], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);

        let mut matched: Option<i64> = None;
        for (id, stored) in candidates {
            if constant_time_eq(expected.as_deref(), decode_hash(&stored).as_deref()) {
                matched = Some(id);
            }
        }
        match matched {
            Some(id) => {
                conn.execute("UPDATE pairing_codes SET used = 1 WHERE id = ?1", [id])?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Removes pairing codes whose expiry has passed.
    pub fn purge_expired_pairing_codes(&self, now: i64) -> Result<usize> {
        let conn = self.conn()?;
        let removed = conn.execute("DELETE FROM pairing_codes WHERE expires_at <= ?1", [now])?;
        Ok(removed)
    }
}

fn decode_hash(value: &str) -> Option<Vec<u8>> {
    hex::decode(value).ok()
}

fn constant_time_eq(a: Option<&[u8]>, b: Option<&[u8]>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) if a.len() == b.len() => a.ct_eq(b).into(),
        _ => false,
    }
}

fn row_to_device(row: &rusqlite::Row<'_>) -> rusqlite::Result<Device> {
    Ok(Device {
        id: row.get("id")?,
        name: row.get("name")?,
        public_key: row.get("public_key")?,
        paired_at: row.get("paired_at")?,
        last_seen: row.get("last_seen")?,
        is_revoked: row.get::<_, i64>("is_revoked")? != 0,
    })
}

/// Hashes a plaintext pairing code for storage and comparison.
pub fn hash_pairing_code(code: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"emusic-server/pairing-code/v1:");
    hasher.update(code.as_bytes());
    hex::encode(hasher.finalize())
}

/// Validates the shape of a generated code (exactly six ASCII digits).
pub fn is_valid_pairing_code_format(code: &str) -> bool {
    code.len() == 6 && code.bytes().all(|byte| byte.is_ascii_digit())
}

/// Converts a device's stored public key into a typed key, or a token error.
pub fn parse_device_public_key(
    public_key: &str,
) -> Result<pasetors::keys::AsymmetricPublicKey<pasetors::version4::V4>> {
    use std::convert::TryFrom;
    pasetors::keys::AsymmetricPublicKey::<pasetors::version4::V4>::try_from(public_key)
        .map_err(|_| ServerError::Token("device public key is not a valid PASERK key".into()))
}
