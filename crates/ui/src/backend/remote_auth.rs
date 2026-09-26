//! Shared remote-token refresh helper (#391).
//!
//! Both the background sync worker and the playback backend need a valid
//! token for a server; this refreshes it with the device-signed proof shortly
//! before it expires and persists the result.

use std::sync::Mutex;
use std::time::Duration;

use emusic_client::auth::{issue_refresh_proof, token_fingerprint};
use emusic_client::{CredentialStore, RemoteClient};

/// How close to expiry a token is refreshed.
pub(crate) const REFRESH_MARGIN_SECS: i64 = 24 * 3600;

/// Serialises credential load/refresh/save across the sync worker and the
/// playback workers, so two threads cannot race the credential file.
static REFRESH_LOCK: Mutex<()> = Mutex::new(());

/// Loads the stored token for `server_id`, refreshing it if it is close to
/// expiry. Errors are strings so callers can surface them as status/playback
/// errors without depending on the client error type.
pub(crate) fn ensure_token(
    client: &RemoteClient,
    credentials: &CredentialStore,
    server_id: &str,
) -> Result<String, String> {
    let _guard = REFRESH_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut creds = credentials
        .load(server_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "not paired".to_string())?;
    if creds.expiring_within(REFRESH_MARGIN_SECS, emusic_client::unix_now()) {
        let secret = creds.secret_key().map_err(|error| error.to_string())?;
        let fingerprint = token_fingerprint(&creds.token);
        let proof = issue_refresh_proof(
            &secret,
            &fingerprint,
            Duration::from_secs(120),
            Duration::from_secs(30),
        )
        .map_err(|error| error.to_string())?;
        let response = client
            .refresh(&creds.token, &proof)
            .map_err(|error| error.to_string())?;
        creds.token = response.auth_token;
        creds.expires_at = response.expires_at;
        credentials
            .save(server_id, &creds)
            .map_err(|error| error.to_string())?;
    }
    Ok(creds.token)
}
