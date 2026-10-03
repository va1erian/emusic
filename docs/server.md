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
- [Admin page](#admin-page)
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

The binary has five subcommands:

| Command | Purpose |
| --- | --- |
| `serve` (default) | Run the HTTP(S) server. |
| `pair [--ttl SECS]` | Generate and print a one-time pairing code. |
| `devices` | List paired devices and their state. |
| `revoke <device_id>` | Revoke a device immediately. |
| `audit [--limit N] [--event X]` | Print recent audit events, newest first. |

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
| `render.enabled` | `EMUSIC_SERVER_RENDER_ENABLED` | `false` | Enable server-side rendering. |
| `render.codec` | `EMUSIC_SERVER_RENDER_CODEC` | `flac` | Output codec (only `flac`). |
| `render.sample_rate` | `EMUSIC_SERVER_RENDER_SAMPLE_RATE` | `44100` | Rendering sample rate, Hz. |
| `render.soundfont_path` | `EMUSIC_SERVER_RENDER_SOUNDFONT` | none | MIDI SoundFont (reserved). |
| `render.cache_max_bytes` | `EMUSIC_SERVER_RENDER_CACHE_MAX_BYTES` | `2147483648` | Rendition cache cap. |
| `render.max_concurrent` | `EMUSIC_SERVER_RENDER_MAX_CONCURRENT` | `2` | Concurrent renders. |
| `admin.enabled` | `EMUSIC_SERVER_ADMIN_ENABLED` | `true` | Run the [admin page](#admin-page) listener. |
| `admin.host` | `EMUSIC_SERVER_ADMIN_HOST` | `127.0.0.1` | IP literal or `localhost`. Non-loopback requires `admin.token`. |
| `admin.port` | `EMUSIC_SERVER_ADMIN_PORT` | `8081` | Must differ from `server.port`. |
| `admin.token` | `EMUSIC_SERVER_ADMIN_TOKEN` | empty | HTTP Basic password (24+ chars). Empty = no auth, loopback only. |
| `audit.retention_days` | `EMUSIC_SERVER_AUDIT_RETENTION_DAYS` | `90` | Days kept in the `audit_log` table (1-3650). |

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

Security and operational events go to three places: the human-readable stdout
log, a daily-rotated JSON-lines file `<data_dir>/audit.log.<date>`, and the
`audit_log` table in the database, which the [admin page](#admin-page) and
`emusic-server audit` query. Each row has a time, an event name, the real
client IP (or `system` / `cli` for events the server or the CLI raises), the
device concerned if any, and a JSON `detail` object (in the JSON-lines file,
`detail` is a JSON string field).

| Area | Events (`detail` fields) |
| --- | --- |
| Pairing | `pairing_code_created` (`by`: `cli`/`admin`/`device`, `ttl_secs`), `device_paired` (`device_name`), `pair_failed` (`reason`: `wrong`, `expired`, `reused`, `rate_limited`, `invalid_request`) |
| Sessions | `token_refreshed`, `refresh_failed` (`reason`), `auth_failed` (`reason`), `device_revoked` (`by`, `by_device`) |
| Library | `scan_started` (`roots`), `scan_finished` (`files_found`, `changed`, `deleted`, `skipped`, `partial`, `elapsed_ms`), `scan_failed`, `library_version_changed` (`from`, `to`), `tombstones_pruned` (`count`) |
| Database | `migration_applied` (`from`, `to`), `audit_pruned` (`removed`, `retention_days`), `admin_integrity_check` (`ok`) |
| Library state | `starred_changed` (`starred`, `unstarred`) |
| Defences | `path_violation` (`track_id`), `rate_limited` (`scope`), `admin_auth_failed` (`reason`), `audit_events_dropped` (`count`) |

Recording an event never blocks or fails a request: handlers push it onto a
bounded in-memory queue (4096 events) drained by one background writer that
inserts in batches. If the queue is ever full the event is dropped, counted,
logged as a warning, and an `audit_events_dropped` row records how many were
lost. Events an anonymous client can trigger at will (`auth_failed`,
`pair_failed`, `rate_limited`, `admin_auth_failed`) are persisted at most 30
times per minute per client address, so a flood cannot grow the table without
bound; the rest still reach the log file, and the admin overview counts them.
Rows older than `audit.retention_days` are pruned at startup and daily.

```sh
emusic-server audit --limit 20
emusic-server audit --event pair_failed
```

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
| `GET /tracks/{id}/render?subtune=N&codec=flac` | Server-rendered rendition of a specialized track. |
| `GET /starred` | Server-wide starred tracks, newest first; `If-None-Match` → `304`. |
| `PUT /starred/{track_id}` | Star a track (idempotent; unknown track → `404`). |
| `DELETE /starred/{track_id}` | Unstar a track (idempotent). |
| `POST /starred/batch` | Star/unstar many tracks in one transaction. |
| `GET /ws` | WebSocket: scan progress, library-version and starred-version events. |

`GET /library/sync` returns a monotonically increasing `version`; every row
carries the `sync_version` at which it last changed, and deletions appear in
`deleted`. A client that stores `version` only ever fetches the difference.

### Starred tracks

The server is single-user, so there is one starred set shared by every paired
device. Rows are keyed by the server track id and survive a scan deleting the
track (the file may come back), but responses only list ids that currently
exist. Every effective change bumps a starred-set `version`; no-op requests
(starring a starred track, unstarring an unstarred one) leave it unchanged.

- `GET /starred` →
  `{ "version": 3, "tracks": [{ "id": "…", "starred_at": 1730000000 }] }`,
  newest first, with `ETag: "3"`. Sending `If-None-Match: "3"` returns `304`
  while the version is unchanged.
- `PUT /starred/{track_id}` and `DELETE /starred/{track_id}` →
  `{ "version": 4 }`. Starring an id that is not in the library is a `404`;
  unstarring never fails.
- `POST /starred/batch` with `{ "star": ["…"], "unstar": ["…"] }` applies both
  lists in one transaction (stars first, then unstars, so an id in both ends up
  unstarred) and returns `{ "version": 5, "unknown": ["…"] }`, where `unknown`
  lists ids in `star` that are not in the library (skipped). At most 5,000 ids
  in total, otherwise `400`. This route accepts bodies up to 1 MiB regardless
  of `security.max_body_bytes`.

Each change is written to the audit log (`starred_changed`, with the device id
and how many ids changed) and broadcast on `GET /ws` as
`{ "type": "starred_changed", "version": 5 }`.

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

### Server-side rendering

Clients that lack the desktop's native engines — notably Android — cannot play
those raw formats. With `render.enabled = true` the server renders them to FLAC
and serves the rendition from
`GET /api/v1/tracks/{id}/render?subtune=N&codec=flac`:

- **SID** is rendered by the in-repo cRSID engine, clamped to the tune's HVSC
  song length when `library.hvsc_songlengths_path` is set (otherwise a bounded
  default).
- **Tracker modules and MIDI** are not renderable yet; their renderers are
  follow-ups. Requests for them return `404`, exactly like an unsupported or
  unknown track.
- `subtune` is 1-based; omitting it renders the file's default subtune.
- `codec` defaults to `render.codec`; any other value is a `400`.

The first request for a rendition renders it, which can take seconds; subsequent
requests are served from the cache with full `Range`/`ETag`/seek support.
Renditions are written atomically to `<data_dir>/renditions/` and evicted LRU
when `render.cache_max_bytes` is exceeded. Renders run on the blocking pool and
are bounded by `render.max_concurrent`, so they never stall the async runtime.

Every `TrackView` carries `renderable` and `renditions` (`["flac"]`) so a client
can tell whether to call `/render` before trying. With `render.enabled = false`
the endpoint returns `404` and no track is reported renderable; `specialized`
is unchanged for the desktop's raw delivery path.

## Admin page

A small, server-rendered admin page runs on a **second listener** with its
own router (`admin.host:admin.port`, default `127.0.0.1:8081`). It is never
mounted on the public router: `/admin` on the public port is a `404`.

| Page | Shows |
| --- | --- |
| **Overview** (`/admin/`) | Version, uptime, schema and library versions, DB + WAL size, rows per table, scan status, render cache size, library roots. |
| **Active users** (`/admin/active`) | Devices with a request in the last 5 minutes or an open WebSocket: last request, client IP, user agent, WebSocket count, last streamed/rendered track. Refreshes every 10 s. |
| **Devices** (`/admin/devices`) | Every paired device with last seen, and a *Revoke* button. |
| **Pairing** (`/admin/pairing`) | Outstanding code count and a *Generate code* button that shows a one-time code. |
| **Audit log** (`/admin/audit`) | Newest first, paginated, filterable by event, device and UTC time range. |
| **Database** (`/admin/database`) | Per-table counts, tombstones, pairing-code rows, page stats, `PRAGMA quick_check`, and `PRAGMA integrity_check` on demand. |

The same data is available as JSON from `GET /admin/api/overview`, `active`,
`devices`, `pairing`, `audit` (same query parameters as the page: `event`,
`device`, `since`, `until`, `before`, `limit`) and `database`.

**Access control.**

- Startup refuses an admin `host` that is not loopback unless `admin.token`
  is set (at least 24 characters). With a token, every request needs HTTP
  Basic auth: any user name, password = token, compared in constant time.
  Wrong or malformed credentials are audited (`admin_auth_failed`), and after
  10 failures in a minute an IP gets `429` until the window slides.
- Without a token (loopback only), requests must carry a `Host` header that is
  an IP literal or `localhost`, which defeats DNS-rebinding attacks from a web
  page open in a browser on the server machine.
- Revoke, generate-code and integrity-check are `POST` forms with a per-process
  CSRF token (valid one hour); cross-site `Origin` / `Sec-Fetch-Site` are
  refused too. These actions are audited with `by: admin`.
- Every response has `Cache-Control: no-store`, `X-Frame-Options: DENY`,
  `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer` and a CSP
  of `default-src 'none'; style-src 'self'; form-action 'self';
  frame-ancestors 'none'`. Pages contain no scripts and load nothing external.
- The listener speaks plain HTTP. Do not expose it to the internet or add it
  to the reverse proxy; reach it over an SSH tunnel instead:

  ```sh
  ssh -L 8081:127.0.0.1:8081 you@homelab
  # then browse http://localhost:8081/
  ```

Set `admin.enabled = false` to turn the listener off entirely.

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

The compose file also publishes the [admin page](#admin-page) on the host's
**loopback only** (`127.0.0.1:8081:8081`). Inside the container the listener
binds `0.0.0.0` (`EMUSIC_SERVER_ADMIN_HOST`) so the mapping can reach it, which
requires `EMUSIC_SERVER_ADMIN_TOKEN`: replace the `CHANGE-ME` placeholder with
a random token (`openssl rand -hex 24`); the server refuses to start until you
do. Do not add port 8081 to the Cosmos Cloud labels; use an SSH tunnel.

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

6. **Pair a device.** Once the desktop client integration lands, enter the
   code in the app's *Homelab Server* settings. Until then the `emusic-remote`
   CLI (built from the same repository, `cargo build -p emusic-client`) can
   pair and pull right away:

   ```sh
   emusic-remote pair https://music.example.com --code 123456
   emusic-remote status https://music.example.com
   emusic-remote pull   https://music.example.com --out ~/Music/remote
   ```

   Credentials are stored per server under the user's config directory
   (`<config>/emusic/servers/<id>.json`); downloads go to the cache directory
   or `EMUSIC_REMOTE_CACHE`.

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
- **Revoking** a lost device: `emusic-server revoke <device_id>` or the admin
  page's *Devices* tab; its tokens stop working on the next request.
- **Auditing**: `emusic-server audit` or the admin page's *Audit log* tab.
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

**Can it be done from the web?** Yes, from the [admin page](#admin-page)'s
*Pairing* tab (over the SSH tunnel), which also works for the first device.
The public HTTP endpoint that mints codes requires an already-paired device:

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

> **Status:** the server and the cross-platform `emusic-client` core + the
> `emusic-remote` CLI are available; the `emusic` desktop app's *Homelab
> Server* UI is not wired up yet.

The `emusic-remote` CLI (crate `emusic-client`) already supports:

```sh
emusic-remote pair   <url> --code 123456      # one-time pairing code
emusic-remote status <url>                    # health + token expiry
emusic-remote list   <url> [--limit 100]      # delta-sync and list tracks
emusic-remote fetch  <url> <track-id> <file>  # one track
emusic-remote pull   <url> --out <dir>        # mirror the library (removes deleted)
emusic-remote songlengths <url> <file>        # HVSC Songlengths.md5 text
```

`--credentials-dir` overrides where the device keypair and token are stored
(`<config>/emusic/servers/<id>.json` by default) and `EMUSIC_REMOTE_CACHE`
overrides the track cache root. The client library is blocking and free of
Win32/BASS/SQLite so the same code can drive the desktop app later.

The desktop app integration (issue #385) will then:

1. Sync metadata from `GET /library/sync`, caching rows locally.
2. Fetch tracks into a local cache and hand the cached path to the existing
   SID/tracker/MIDI decoders (standard audio lands fully first; true Range
   streaming via a BASS URL stream is a follow-up).
3. Store its Ed25519 private key and its last `version`, and refresh its
   token with a proof before expiry.
4. Expose a *Homelab Server* settings page for the URL and pairing code.
