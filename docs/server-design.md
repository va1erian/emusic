# emusic Server Architecture & Design Specification

This document presents the complete architecture and design for **`emusic-server`**, a high-performance, secure homelab music server designed to interface with the **`emusic`** Windows desktop application.

---

## 1. Executive Summary & Core Objectives

### Objectives
1. **Homelab Self-Hosting**: Run `emusic-server` on home servers (Linux / Docker / Windows) with minimal memory footprint and fast library scanning.
2. **Support for Specialized & Tracker Formats**: Native streaming and playback support for formats that are uncommon in standard streaming systems, such as Commodore 64 SID files (`.sid`, `.psid`), Tracker Modules (`.mod`, `.s3m`, `.xm`, `.it`, `.mo3`), and MIDI (`.mid`).
3. **Hardened Security**: Zero-Trust design preventing unauthorized access to the host machine or media library over public or private networks.
4. **Minimal Client Changes**: Integrate with existing `emusic` player logic (`crates/player`, `crates/library`, Win32 UI) without rewriting the core audio pipeline or Win32 user interface.

---

## 2. High-Level Architecture

```
 +-------------------------------------------------------------------------+
 |                          Homelab / Host Server                          |
 |                                                                         |
 |  +-------------------------------------------------------------------+  |
 |  |            Cosmos Cloud / Reverse Proxy (Let's Encrypt TLS)       |  |
 |  +------------------------------------+------------------------------+  |
 |                                       | HTTP/2 (Internal Net)           |
 |  +------------------------------------v------------------------------+  |
 |  |                          emusic-server                            |  |
 |  |                                                                   |  |
 |  |  +--------------------+  +--------------------+  +-------------+  |  |
 |  |  | Trusted Proxy Auth |  | Path Sanitizer &   |  | SQLite      |  |  |
 |  |  | & Token Validation |  | Jail (Chroot/Safe) |  | Meta Store  |  |  |
 |  |  +---------+----------+  +---------+----------+  +------+------+  |  |
 |  |            |                       |                    |         |  |
 |  |  +---------v-----------------------v--------------------v------+  |  |
 |  |  |             Axum / Tokio HTTP/2 REST & WS API              |  |  |
 |  |  +---------------------------------+---------------------------+  |  |
 |  +------------------------------------|------------------------------+  |
 |                                       | HTTPS (Port 443 / Let's Encrypt)|
 +---------------------------------------|---------------------------------+
                                         |
                            (Cosmos Cloud HTTPS Domain)
                                         |
 +---------------------------------------v---------------------------------+
 |                           Client Workstation                            |
 |                                                                         |
 |  +-------------------------------------------------------------------+  |
 |  |                            emusic                                 |  |
 |  |                                                                   |  |
 |  |  +--------------------+  +--------------------+  +-------------+  |  |
 |  |  | Remote Library Sync|  | Stream Cache Mgr   |  | Player Engine|  |  |
 |  |  | (Background)       |  | (Local Disk / RAM) |  | (BASS / SID)|  |  |
 |  |  +--------------------+  +---------+----------+  +------+------+  |  |
 |  |                                    |                     |        |  |
 |  |                                    +---------> + <-------+        |  |
 |  |                                                |                  |  |
 |  |                                          Native Audio             |  |
 |  +-------------------------------------------------------------------+  |
 +-------------------------------------------------------------------------+
```

---

## 3. Security Architecture

Security is paramount. The server exposes media and metadata to remote clients, making it a potential target if exposed to the internet. `emusic-server` enforces multiple defense-in-depth layers.

### 3.1 Authentication & Device Pairing
- **PASETO (Platform-Agnostic Security Tokens) / Ed25519**:
  - `emusic-server` avoids raw session cookies or long-lived static API keys.
  - Device pairing uses a short-lived **One-Time Pairing Code (6-digit / QR Code)** generated via CLI (`emusic-server pair`) or admin console.
  - Upon pairing, client and server exchange asymmetric keys (Ed25519 keypair).
  - Subsequent requests require a signed PASETO token (v4.public or v4.local) containing the `device_id`, timestamp, and scope.
- **Client Revocation**:
  - The server maintains a persistent list of paired `device_id` records in SQLite. Revoking a device immediately invalidates all associated tokens.

### 3.2 Network Layer & Reverse Proxy Integration (Cosmos Cloud)
- **Cosmos Cloud / Reverse Proxy Friendly**:
  - In homelabs running **Cosmos Cloud** (or Traefik/Nginx/Caddy), Cosmos Cloud handles public exposure and manages **Let's Encrypt** SSL/TLS certificates automatically.
  - `emusic-server` can run in plain HTTP mode internally within the Docker bridge network while Cosmos Cloud terminates TLS 1.3 at the perimeter.
  - **Trusted Proxy Header Handling**: `emusic-server` inspects `X-Forwarded-For`, `X-Forwarded-Proto`, and `Host` headers provided by Cosmos Cloud when `trusted_proxies` is configured in `server.toml`.
  - **WebSocket & Range Pass-Through**: Cosmos Cloud transparently forwards WebSocket connections (`/api/v1/ws`) and HTTP `206 Partial Content`Range requests without buffering or chunk truncation.
- **Direct TLS Mode**: If run without a reverse proxy, `emusic-server` uses built-in `rustls` with user-supplied certificates.

### 3.3 Storage Access & Path Traversal Prevention
- **Strict Path Canonicalization**:
  - The server isolates audio files within configured base paths (`/music/library_1`, etc.).
  - Every file request canonicalizes requested relative paths using `std::fs::canonicalize` and verifies that the resulting `Path` starts with an authorized root directory.
  - Rejects null bytes, UNC path overrides, symlinks pointing outside the root directory, or relative navigation (`..`).
- **Sandboxed File Handles**:
  - File reading is constrained by a strict safe I/O layer that returns structured streams rather than exposing direct path handles.

### 3.4 Rate Limiting & Audit Logging
- **Adaptive Rate Limiting**:
  - Brute-force protection on authentication and pairing endpoints (e.g., maximum 3 pairing attempts per minute per IP), utilizing real client IPs extracted from proxy headers when behind Cosmos Cloud.
- **Structured Audit Logs**:
  - Security events (failed auth, device paired, device revoked, path access violations) are written to structured audit logs (`tracing` + JSON file target).

---

## 4. Format Handling & Streaming Strategy

### 4.1 Standard Audio Formats (FLAC, MP3, WAV, OGG, Opus, AAC)
- **HTTP/2 Range Requests (`206 Partial Content`)**:
  - Standard streams utilize chunked streaming with `Range` header support to allow instant seeking without downloading the full file.
- **Client Cache**:
  - `emusic` pre-buffers audio chunks locally in an encrypted or restricted temporary cache directory.

### 4.2 Specialized Formats (SID, Tracker Modules MOD/S3M/XM/IT, MIDI)

#### The Problem
Standard streaming protocols (like HLS or Icecast) transcode audio server-side into Lossy PCM/AAC. For demoscene formats, tracker modules, and C64 SID files, server-side transcoding destroys essential client features:
- Inability to inspect tracker channels, instruments, orders, and rows in `emusic`'s Tracker view.
- Loss of real-time oscilloscope/FFT visualization capability directly from synthesized audio channels.
- Inability to customize MIDI soundfonts or SID subtune selection locally on the client.
- Unnecessary server CPU load for rendering music that is naturally compact in raw format.

#### The Solution: Native Direct-Format Delivery
- **Raw File Delivery**: Tracker modules (`.mod`, `.s3m`, `.xm`, `.it`, `.mo3`), SID files (`.sid`, `.psid`), and MIDI (`.mid`) are transferred in their original raw binary format.
- **Extremely Low Bandwidth**: A SID file is typically ~2KB to 10KB; a MOD/XM module is ~100KB to 2MB. Transferring raw files consumes significantly less bandwidth than streaming a 320kbps MP3 or lossless FLAC stream!
- **Accurate Local Rendering**: The client uses its existing native rendering engines:
  - **SID**: `crates/sid` (cRSID cycle-exact emulation).
  - **Tracker**: `crates/player` via `BASS_MusicLoad`.
  - **MIDI**: `crates/player` via `BASSMIDI` and local SoundFonts (`.sf2`/`.sf3`).
- **Sidecar Metadata**:
  - High Voltage SID Collection (HVSC) songlength details (`Songlengths.md5`) and subtunes count are sent as JSON headers alongside the stream or queried via the server's metadata API.

---

## 5. Server Architecture & Data Model

### 5.1 Crate Structure (`crates/server`)
A new crate `crates/server` is added to the `emusic` workspace, or compiled as an independent binary `emusic-server`.

```
crates/server/
├── Cargo.toml
├── src/
│   ├── main.rs                 # CLI entry point and server startup
│   ├── config.rs               # Server configuration loader (TOML/env)
│   ├── db/                     # SQLite database migrations and queries
│   │   ├── mod.rs
│   │   ├── schema.rs
│   │   └── models.rs
│   ├── auth/                   # PASETO tokens, pairing, mTLS helpers
│   │   ├── mod.rs
│   │   ├── paseto.rs
│   │   └── pairing.rs
│   ├── scanner/                # Library file crawler & tag extractor
│   │   └── mod.rs
│   ├── api/                    # Axum web endpoints
│   │   ├── mod.rs
│   │   ├── auth_routes.rs
│   │   ├── library_routes.rs
│   │   ├── stream_routes.rs
│   │   └── ws.rs               # WebSocket for live sync
│   └── util/                   # Path sanitization, security helpers
│       └── security.rs
```

### 5.2 SQLite Schema (`emusic-server.db`)
Reuses and extends tag metadata models from `crates/library`:

```sql
CREATE TABLE IF NOT EXISTS devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    public_key TEXT NOT NULL,
    paired_at DATETIME NOT NULL,
    last_seen DATETIME,
    is_revoked INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS tracks (
    id TEXT PRIMARY KEY,            -- SHA256 of canonical relative path
    relative_path TEXT NOT NULL,
    format TEXT NOT NULL,           -- 'flac', 'mp3', 'sid', 'xm', 'mod', 'mid', etc.
    title TEXT,
    artist TEXT,
    album TEXT,
    duration_secs REAL,
    subtunes INTEGER DEFAULT 1,      -- For SID files
    file_size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    hash TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_tracks_path ON tracks(relative_path);
```

---

## 6. API Specification

All routes require a valid PASETO token in the `Authorization: Bearer <token>` header (except initial pairing).

### Authentication & Pairing Endpoints
- `POST /api/v1/auth/pair`
  - Body: `{ "pairing_code": "123456", "device_name": "LivingRoom-PC", "public_key": "..." }`
  - Response: `{ "device_id": "...", "auth_token": "..." }`
- `POST /api/v1/auth/refresh`
  - Refreshes expired PASETO tokens.

### Library & Metadata Endpoints
- `GET /api/v1/library/sync?since_version=102`
  - Returns updated, added, or deleted track metadata since `since_version` for efficient delta syncing.
- `GET /api/v1/tracks/{id}/meta`
  - Fetches detailed track info (including SID subtunes, songlengths, or module channel counts).
- `GET /api/v1/albums/{id}/art`
  - Retrieves cover art image bytes (JPEG/PNG/WebP).

### Streaming & File Fetch Endpoints
- `GET /api/v1/tracks/{id}/stream`
  - Headers: `Range: bytes=0-` (for audio streams).
  - For standard streams: Returns `206 Partial Content` audio bytes.
  - For tracker/SID/MIDI: Returns complete file contents (`application/x-sid`, `audio/x-mod`, `audio/midi`).
- `GET /api/v1/sid/songlengths`
  - Returns `Songlengths.md5` entries for SID hash lookups.

### Real-Time WebSocket Endpoint
- `GET /api/v1/ws`
  - Bi-directional status updates (e.g., server library scan progress, newly added albums).

---

## 7. Minimal Client Changes in `emusic`

To keep changes minimal and isolated in `emusic`:

1. **Remote Library Provider (`crates/library`)**:
   - Introduce a `RemoteLibraryProvider` struct that implements the existing library storage/index interfaces.
   - On application startup, if remote server connection is configured, `RemoteLibraryProvider` performs a background sync with `GET /api/v1/library/sync`.
   - Stores remote metadata in `emusic`'s local SQLite cache, tagging entries with `is_remote = 1` and `remote_track_id`.

2. **Stream / Cache Manager (`crates/player`)**:
   - When a remote track is queued for playback, the URL/track ID is resolved through `StreamCacheManager`.
   - For standard audio: Streams chunks directly to BASS via push stream or HTTP stream handle.
   - For `.sid`, `.mod`, `.xm`, `.mid`: Fetches the file to a secure temporary directory (`%LOCALAPPDATA%\emusic\cache\tracks\<hash>.<ext>`) and passes the local cached file path to existing `AudioBackend` decoders (`SidChannel`, `BassChannel::Music`, `BassBackend::open_stream`).
   - Reuses existing audio playback pipelines without changing `SidPlayer`, `BassBackend`, or Win32 UI views!

3. **Settings UI (`crates/app`)**:
   - Add a "Homelab Server" section in `emusic` settings:
     - Server URL (e.g. `https://music.myhomelab.com` served via Cosmos Cloud or local URL).
     - "Pair New Server" button prompting for the 6-digit pairing code.
     - Sync status indicator.

---

## 8. Server Configuration Reference (`server.toml`)

```toml
[server]
host = "0.0.0.0"
port = 8080 # Run on internal port when behind Cosmos Cloud / proxy
data_dir = "/var/lib/emusic-server"
trusted_proxies = ["172.16.0.0/12", "127.0.0.1"] # Cosmos Cloud container subnet

[security]
# Set tls_cert/tls_key if running standalone; leave blank when behind Cosmos Cloud
tls_cert = ""
tls_key = ""
token_ttl_hours = 168 # 7 days
max_pairing_attempts_per_min = 3

[library]
paths = [
    "/media/music",
    "/media/tracker_modules",
    "/media/hvsc_sid"
]
hvsc_songlengths_path = "/media/hvsc_sid/C64Music/DOCUMENTS/Songlengths.md5"
scan_interval_secs = 3600
```

---

## 9. Deployment Strategy

### 9.1 Cosmos Cloud / Docker Compose Deployment
`emusic-server` integrates seamlessly into Cosmos Cloud as a managed container with automated Let's Encrypt SSL:

```yaml
version: '3.8'
services:
  emusic-server:
    image: va1erian/emusic-server:latest
    container_name: emusic-server
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - /path/to/music:/media/music:ro
      - /path/to/config:/etc/emusic-server
      - /path/to/data:/var/lib/emusic-server
    environment:
      - RUST_LOG=info,emusic_server=debug
    # Cosmos Cloud automatically routes HTTPS traffic from your domain
    # (e.g., https://music.homelab.net) to port 8080 and manages Let's Encrypt certificates.
    labels:
      - "cosmos-cloud.enabled=true"
      - "cosmos-cloud.domain=music.homelab.net"
      - "cosmos-cloud.target-port=8080"
```

### 9.2 Native systemd Service
```ini
[Unit]
Description=emusic Homelab Music Server
After=network.target

[Service]
Type=simple
User=emusic
ExecStart=/usr/local/bin/emusic-server --config /etc/emusic-server/server.toml
Restart=on-failure
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/emusic-server
ReadOnlyPaths=/media/music

[Install]
WantedBy=multi-user.target
```

---

## 10. Summary & Verification Matrix

| Requirement | Proposed Solution | Implementation Area |
| :--- | :--- | :--- |
| **Security** | TLS 1.3 via Cosmos Cloud / Let's Encrypt, PASETO v4 tokens, device pairing, strict path canonicalization. | `crates/server/src/auth`, `util/security.rs` |
| **Cosmos Cloud Integration** | Automatic Let's Encrypt TLS termination, trusted proxy headers (`X-Forwarded-For`), WebSocket & Range pass-through. | `crates/server/src/config.rs`, Cosmos Docker labels |
| **Specialized Formats** | Direct raw binary file transfer for `.sid`, `.mod`, `.xm`, `.it`, `.mid`. | `crates/server/src/api/stream_routes.rs` |
| **Minimal Client Changes** | Transferred files saved to local temp cache; existing `SidChannel` & `BassBackend` handles playback. | `crates/player`, `crates/library` |
| **Homelab Deployment** | Cosmos Cloud Docker Compose template, single binary, systemd integration. | `crates/server`, `installer/` |
