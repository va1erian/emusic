# emusic-server

`emusic-server` is the hardened homelab server that lets the `emusic` desktop
client stream and sync a music library over the internet. It is a single
cross-platform binary (Linux, macOS, Windows) written in Rust, backed by
SQLite, and designed to run behind a TLS-terminating reverse proxy such as
**Cosmos Cloud**. The full architecture rationale lives in
[server-design.md](server-design.md); this document is the operator and client
guide.

## Contents

- [Running](#running)
- [Configuration](#configuration)
- [Security model](#security-model)
- [HTTP API](#http-api)
- [Streaming & specialized formats](#streaming--specialized-formats)
- [Deployment](#deployment)
- [Operations](#operations)

## Running

```sh
# Build
cargo build --release -p emusic-server

# Generate the first pairing code (writes/uses the data directory)
emusic-server --config deploy/server.example.toml pair

# Run
emusic-server --config /etc/emusic-server/server.toml
```

The binary has four subcommands:

| Command | Purpose |
| --- | --- |
| `serve` (default) | Run the HTTP(S) server. |
| `pair [--ttl SECS]` | Generate and print a one-time pairing code. |
| `devices` | List paired devices and their state. |
| `revoke <device_id>` | Revoke a device immediately. |

## Configuration

Configuration is TOML, loaded from `--config`, else `EMUSIC_SERVER_CONFIG`,
else built-in defaults (which require a library path). Every key has an
environment override, which makes container configuration file-free.

| TOML key | Environment | Default | Notes |
| --- | --- | --- | --- |
| `server.host` | `EMUSIC_SERVER_HOST` | `0.0.0.0` | Bind address. |
| `server.port` | `EMUSIC_SERVER_PORT` | `8080` | Internal port. |
| `server.data_dir` | `EMUSIC_SERVER_DATA_DIR` | `/var/lib/emusic-server`¹ | DB, server key, audit log. |
| `server.trusted_proxies` | `EMUSIC_SERVER_TRUSTED_PROXIES` | empty | CIDR/IP list, comma or `;` separated. |
| `security.tls_cert` | `EMUSIC_SERVER_TLS_CERT` | empty | PEM chain for direct TLS. |
| `security.tls_key` | `EMUSIC_SERVER_TLS_KEY` | empty | PEM key for direct TLS. |
| `security.token_ttl_hours` | `EMUSIC_SERVER_TOKEN_TTL_HOURS` | `168` | Access-token lifetime. |
| `security.max_pairing_attempts_per_min` | `EMUSIC_SERVER_MAX_PAIRING_ATTEMPTS` | `3` | Per client IP. |
| `security.pairing_code_ttl_secs` | — | `600` | Code lifetime (max 1 hour). |
| `security.max_body_bytes` | — | `65536` | Request body limit (max 1 MiB). |
| `library.paths` | `EMUSIC_SERVER_LIBRARY_PATHS` | required | Read-only roots. |
| `library.hvsc_songlengths_path` | `EMUSIC_SERVER_HVSC_SONGLENGTHS` | none | HVSC `Songlengths.md5`. |
| `library.scan_interval_secs` | `EMUSIC_SERVER_SCAN_INTERVAL` | `3600` | `0` disables periodic scans. |

¹ `%LOCALAPPDATA%\emusic-server` on Windows.

Startup is **fail-closed**: an inconsistent configuration (TLS cert without
key, zero TTL, empty library) is refused rather than silently weakened.
Unknown TOML keys are rejected so typos cannot disable a security control.

## Security model

### Device pairing

1. The operator runs `emusic-server pair` (or a paired device calls
   `POST /api/v1/devices/pairing-codes`) to mint a random six-digit code.
   Only a keyed HMAC of the code is stored — the HMAC key is the server
   secret, which lives in `server.key` rather than the database — so a leak
   of the database alone does not yield a brute-forceable code. The code
   expires and is single-use, enforced atomically in one transaction.
2. The client generates an Ed25519 keypair and calls
   `POST /api/v1/auth/pair` with the code, a device name and its public key in
   PASERK form (`k4.public.…`).
3. The server registers the device and returns a PASETO v4.public access token
   signed by the server key.

Pairing is rate limited per real client IP; wrong, expired and already-used
codes all return the same `401` so they cannot be distinguished.

### Tokens and refresh

Every protected endpoint requires `Authorization: Bearer <token>`. The token
is a PASETO v4.public token containing the device id (`sub`), a unique `jti`,
a scope and an expiry. On each request the server verifies the signature and
expiry and confirms the device is not revoked.

To extend a session, the client calls `POST /api/v1/auth/refresh` with the
current token **and a device-signed proof**:

```jsonc
// body
{ "proof": "v4.public.…" }
```

The proof is itself a short-lived PASETO v4.public token signed with the
device's private key, carrying an additional `refresh_for` claim equal to
`token_fingerprint(access_token)`, where

```
token_fingerprint(t) = hex(SHA-256("emusic-server/refresh-token/v1:" || t))
```

Because the proof is bound to that exact token and verified against the stored
device public key, a stolen access token alone cannot be refreshed — the
attacker also needs the device's private key.

### Trusted proxies and client IPs

`X-Forwarded-For` is honoured **only** when the immediate peer is inside
`trusted_proxies`. The header is walked from right to left, skipping further
trusted hops; the first untrusted address is the client. A forged prefix added
by an attacker is ignored because the rightmost untrusted hop is what the last
trusted proxy actually saw. Without a trusted peer, the header is ignored
entirely.

### Path jail

Clients only ever name an opaque track id. The server resolves the stored
relative path under its configured root with `canonicalize` and verifies the
result starts with that root using component-wise comparison. Absolute paths,
`..`, NUL bytes, drive/UNC prefixes and symlinks that escape the root are all
rejected. Violations are audited and surfaced as `404` so no path is disclosed.

### Audit log

Security events are written as JSON lines to a daily-rotated
`<data_dir>/audit.log.<date>` file, in addition to the human-readable stdout
log. Events include `auth_failed`,
`pair_failed`, `device_paired`, `device_revoked`, `token_refreshed`,
`path_violation`, `rate_limited` and `scan_finished`.

## HTTP API

All routes are under `/api/v1` and require a bearer token except `health` and
`auth/pair`.

| Method & path | Description |
| --- | --- |
| `GET /health` | Unauthenticated liveness: `status` and `started_at` only. |
| `POST /auth/pair` | Pair a device. Body: `pairing_code`, `device_name`, `public_key`. |
| `POST /auth/refresh` | Extend a session. Body: `proof`. |
| `GET /devices` | List paired devices (no public keys). |
| `POST /devices/pairing-codes` | Mint a pairing code while authenticated. |
| `DELETE /devices/{id}` | Revoke a device. |
| `GET /library/sync?since_version=N` | Delta sync: changed tracks and tombstones. |
| `GET /tracks/{id}/meta` | Full metadata for one track. |
| `GET /albums/{id}/art` | Cover art bytes. |
| `GET /sid/songlengths` | HVSC song lengths by MD5. |
| `GET /tracks/{id}/stream` | Audio bytes, `Range` supported. |
| `GET /ws` | WebSocket: scan progress and library-version events. |

`GET /library/sync` returns a monotonically increasing `version`; every row
carries the `sync_version` at which it last changed, and deletions appear in
`deleted`. A client that stores `version` only ever fetches the difference.

## Streaming & specialized formats

Standard audio (FLAC, MP3, WAV, OGG, Opus, AAC, …) is served with HTTP `Range`
support: a `Range: bytes=start-end` request yields `206 Partial Content` with
`Content-Range`, `Accept-Ranges` and an `ETag`. Only a single range is
supported; multi-range requests are rejected.

SID (`.sid`/`.psid`/`.rsid`), tracker modules (`.mod`, `.xm`, `.it`, `.s3m`,
`.mo3`, …) and MIDI are transferred **raw**, with their specialized content
type (`audio/x-sid`, `audio/x-mod`, `audio/midi`). The client renders them with
its native engines (cRSID, BASS, BASSMIDI), preserving channel inspection,
subtune selection, soundfont choice and visualisation. SID subtune counts and
song lengths are indexed server-side and exposed through the API.

## Deployment

### Docker + Cosmos Cloud

Use [deploy/docker-compose.yml](../deploy/docker-compose.yml) and
[deploy/Dockerfile](../deploy/Dockerfile). A prebuilt image is published to the
GitHub Container Registry by
[.github/workflows/server-image.yml](../.github/workflows/server-image.yml):
`ghcr.io/va1erian/emusic-server:latest`, plus `sha-<short>` and version tags.
Cosmos Cloud terminates TLS and
forwards `https://music.homelab.net` to the internal port `8080`; the server
runs plain HTTP inside the bridge network. Set `trusted_proxies` to the Cosmos
Cloud bridge subnet, for example:

```toml
trusted_proxies = ["172.16.0.0/12", "127.0.0.1"]
```

The container runs as a non-root user with a read-only root filesystem, all
capabilities dropped and `no-new-privileges`; only the data directory is
writable and the library is mounted read-only.

### systemd

Use [deploy/emusic-server.service](../deploy/emusic-server.service). It runs
under a dedicated user with `ProtectSystem=strict`, `ProtectHome=true`,
`MemoryDenyWriteExecute=true` and explicit `ReadWritePaths` / `ReadOnlyPaths`.
Open only the internal port; keep the public TLS endpoint at the proxy.

### Direct TLS

If no reverse proxy is used, set both `security.tls_cert` and
`security.tls_key`; the server then speaks TLS directly via `rustls`.

## Operations

- **Scanning** runs on startup and then every `scan_interval_secs`. A scan that
  cannot see part of the library (unreachable root, unreadable directory or a
  root that suddenly yields zero files) is marked partial and never deletes
  rows it could not verify, so an unmounted share cannot wipe the index.
- **Pairing** a new client: `emusic-server pair`, enter the code in the desktop
  app's *Homelab Server* settings.
- **Revoking** a lost device: `emusic-server revoke <device_id>`; its tokens
  stop working on the next request.
- **Back up** `<data_dir>/emusic-server.db` (library index and devices) and
  `<data_dir>/server.key` (token signing key). Losing `server.key` invalidates
  existing tokens but not device records.

### Client integration

The desktop client:

1. Syncs metadata from `GET /library/sync`, caching rows locally.
2. Streams standard audio straight to the audio backend with `Range`.
3. Fetches specialized formats to a local cache file first, then hands the
   path to its existing SID/tracker/MIDI decoders.
4. Stores its Ed25519 private key and its last `version`, and refreshes its
   token with a proof before expiry.
