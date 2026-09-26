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
| `security.pairing_code_ttl_secs` | `EMUSIC_SERVER_PAIRING_CODE_TTL` | `600` | Code lifetime (max 1 hour). |
| `security.max_body_bytes` | `EMUSIC_SERVER_MAX_BODY_BYTES` | `65536` | Request body limit (max 1 MiB). |
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

Security events raised while serving are written as JSON lines to a
daily-rotated `<data_dir>/audit.log.<date>` file, in addition to the
human-readable stdout log. Events include `auth_failed`,
`pair_failed`, `device_paired`, `device_revoked`, `token_refreshed`,
`path_violation`, `rate_limited` and `scan_finished`.

## HTTP API

All routes are under `/api/v1` and require a bearer token except `health` and
`auth/pair`.

| Method & path | Description |
| --- | --- |
| `GET /api/v1/health` | Unauthenticated liveness: `status` and `started_at` only. |
| `POST /auth/pair` | Pair a device. Body: `pairing_code`, `device_name`, `public_key`. |
| `POST /auth/refresh` | Extend a session. Body: `proof`. |
| `GET /devices` | List paired devices (no public keys). |
| `POST /devices/pairing-codes` | Mint a pairing code while authenticated. |
| `DELETE /devices/{id}` | Revoke a device. |
| `GET /library/sync?since_version=N` | Delta sync: changed tracks and tombstones. |
| `GET /tracks/{id}/meta` | Full metadata for one track. |
| `GET /albums/{id}/art` | Cover art bytes. |
| `GET /sid/songlengths` | HVSC `Songlengths.md5` text (parsed by the client's existing parser). |
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
type (`audio/x-sid`, `audio/x-mod`, `audio/midi`). Every `TrackView` carries a
`specialized` flag so a client knows to fetch the whole file and render it
natively, rather than stream it for seeking. The client renders with its
native engines (cRSID, BASS, BASSMIDI), preserving channel inspection, subtune
selection, soundfont choice and visualisation. `subtunes` gives the count and
`duration_secs` the tune's default (start) subtune; the full
`Songlengths.md5` text is available from `GET /sid/songlengths` for
per-subtune lengths.

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

#### Cosmos Cloud quickstart

1. **Make the image pullable.** The GHCR package is private by default:
   GitHub → your profile → **Packages** → `emusic-server` → *Package settings*
   → *Change visibility* → **Public** (or configure registry credentials in
   Cosmos).
2. **Add a stack.** In Cosmos Cloud, open **Stacks** (Docker Compose) → *Add
   stack* and paste [deploy/docker-compose.yml](../deploy/docker-compose.yml),
   then edit:
   - `image:` to the tag you want (`:latest`, or a specific version),
   - the `/path/to/music:/media/music:ro` line to your collection,
   - `EMUSIC_SERVER_TRUSTED_PROXIES` to Cosmos's proxy subnet (default
     `172.16.0.0/12` covers Docker's usual `172.17`–`172.31`),
   - `cosmos-cloud.domain` to your public hostname.
3. **Deploy.** Cosmos obtains the Let's Encrypt certificate and routes
   `https://<domain>` to the container's port `8080`; the server itself speaks
   plain HTTP internally. Watch the logs for `listening on plain HTTP` and a
   `scan_finished` line.
4. **Verify** through the public URL:

   ```sh
   curl https://music.example.com/api/v1/health
   # {"status":"ok","started_at":...}
   ```

5. **Generate the first pairing code** inside the running container (see
   [Pairing](#pairing) below):

   ```sh
   docker exec -it emusic-server emusic-server pair
   ```

6. **Pair a device** with the code from step 5.

> The compose file uses a **named volume** (`emusic-data`) for the data
> directory; a fresh volume inherits the image's ownership (uid 10001), so no
> `chown` is needed. If you prefer a bind mount, first run
> `mkdir -p /path/data && chown 10001:10001 /path/data`.

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
- **Revoking** a lost device: `emusic-server revoke <device_id>`; its tokens
  stop working on the next request.
- **Back up** `<data_dir>/emusic-server.db` (library index and devices) and
  `<data_dir>/server.key` (token signing key). The database runs in WAL mode,
  so stop the server first or copy the `-wal`/`-shm` files too (or use
  `sqlite3 .backup`). Losing `server.key` invalidates existing tokens but not
  device records.

### Pairing

A **pairing code is generated by the server**, not the client, and it is
single-use with a short lifetime (10 minutes by default). The code is the only
secret needed to enrol a new device; it is stored only as a keyed HMAC.

**Where to run it.** Run the binary against the same data directory the server
uses. With Docker that means inside the running container, which already has
the right environment and volume:

```sh
# Docker / Cosmos Cloud
docker exec -it emusic-server emusic-server pair
# 123456
# Pairing code valid for 600s; enter it in the emusic client.

# Native / systemd
sudo -u emusic emusic-server --config /etc/emusic-server/server.toml pair
```

The server does not need to be stopped; SQLite runs in WAL mode, so the CLI and
the server share the database safely. The command prints the code on its own
line and then a human-readable hint.

**Can it be done from the web?** For the *first* device, no. There is no admin
web UI, and the HTTP endpoint that mints codes requires an already-paired
device. Later codes can be minted over the API by a paired device:

```http
POST /api/v1/devices/pairing-codes
Authorization: Bearer <token>
```

so a client app can offer a "pair another device" button without shell access.

**Enrolling a device** (what the client does):

```http
POST /api/v1/auth/pair
{ "pairing_code": "123456", "device_name": "LivingRoom-PC",
  "public_key": "k4.public...." }
```

The device generates an Ed25519 keypair, sends the public key in PASERK form,
and stores the returned access token plus its own private key. Sessions are
extended with `POST /api/v1/auth/refresh` using a device-signed proof bound to
the current token (see [Security model](#security-model)).

### Client integration

> **Status:** the server is complete and deployable; the `emusic` desktop
> client does not speak to it yet. Until the client integration lands the
> server can be exercised with `curl` and the pairing/revocation CLIs.

The desktop client is planned to:

1. Sync metadata from `GET /library/sync`, caching rows locally.
2. Fetch tracks (standard audio with `Range` seeking; specialized formats
   whole) into a local cache and hand the cached path to the existing
   SID/tracker/MIDI decoders.
3. Store its Ed25519 private key and its last `version`, and refresh its
   token with a proof before expiry.
4. Expose a *Homelab Server* settings page for the URL and pairing code.
