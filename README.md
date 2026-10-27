# GhostRoom — Backend v0.1 (room control only)

The **`ghostroom-server`** executable exposes loopback-only HTTP room
**control** operations backed by the synchronous, bounded
`ghostroom-core::RoomManager`. It is experimental. **No ciphertext
messages, E2EE, Tor deployment, Android application or persistence**
exist. ADR 0002 remains **NO-GO / BLOCKED**.

## Run locally

Install Rust 1.99.1 with Cargo, Clippy and rustfmt. From repository root:

```sh
cargo run -p ghostroom-server
```

Default listener: `127.0.0.1:38421`; any non-loopback address is
rejected. A future Tor onion service may forward to this loopback
listener but Tor is **not configured or verified** here.

## HTTP v1 routes

| Method | Path | Behavior |
|---|---|---|
| GET | `/health/live` | Only `{"status":"ok"}` |
| POST | `/v1/rooms` | Empty body, returns 201 with `room_id`, `creator_id`, `creator_access`, `invitation` |
| POST | `/v1/rooms/{room_id}/join` | JSON `{"invitation":"<64 hex>"}`, returns 201 with `participant_id` and `access` |
| POST | `/v1/rooms/{room_id}/cancel` | Creator credential required; 204 on success |
| POST | `/v1/rooms/{room_id}/close` | Either active participant credential; 204 on success |
| GET | `/v1/rooms/{room_id}/status` | Credential required; JSON `{"state":"waiting"}` or `{"state":"active"}` |

Room/participant IDs are 32 hexadecimal characters, credentials are
64 hexadecimal characters. For cancel/close/status set
`x-ghostroom-participant-id: <32 hex>` and
`Authorization: Bearer <64 hex>`. Credentials never go into URLs.
Create, cancel, close, status and liveness require empty request bodies;
join requires JSON. Secret responses are labeled `Cache-Control: no-store`.
The in-flight semaphore refuses excess work with 503 `{"code":"busy"}`
without waiting on an application permit queue. Responses from create/join reveal new secrets **once only** and must
be transferred to the intended client over a protected channel.
**This HTTP API currently has no TLS or Tor: do not expose it publicly.**

Missing/invalid/closed room credentials share the generic 404
`{"code":"room_unavailable"}` response. Malformed bodies return 400,
unsupported join media types 415, excessive payloads 413,
capacity exhaustion 503, and unexpected manager failures 500.
Responses carry fixed error codes without private state or tokens.
No request-body logging is configured.

Limits: `GHOSTROOM_MAX_ROOMS` (default 128, max 4096),
`GHOSTROOM_MAX_BODY_BYTES` (default 1024, max 8192),
`GHOSTROOM_MAX_IN_FLIGHT` (default 32, max 1024),
`GHOSTROOM_CLEANUP_BUDGET` (default 16, max 4096), and
`GHOSTROOM_BIND` (default 127.0.0.1:38421).
Room waiting lifetime is 600 seconds; invitation lifetime 300 seconds;
active lifetime 3600 seconds. The bounded cleanup task scans up to the configured budget each second.
An unrecoverable cleanup failure triggers controlled shutdown with a
nonzero process exit code. The server handles SIGINT and Unix SIGTERM
and drains for at most 10 seconds; outstanding requests after the grace
period are interrupted. All state
is lost on restart, deliberately failing closed.

## Verify

```sh
cargo fmt --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --release
cargo test --locked
cargo test --locked --release
cargo doc --no-deps
```

For HTTP tests: `cargo test -p ghostroom-server`.

Core IDs do **not** authenticate a user. Bearer secrets and the
manager-owned monotonic clock enforce lifecycle operations; the server
never returns a room lock, verifier or reusable authorization token.
The core no longer offers `with_active` to run arbitrary callbacks
under the per-room lock. No message transfer or account endpoints
exist, and this software is **not production hardened or an encrypted messenger**.

**Security notice:** Backend v0.1 is a *local development* control plane,
not a complete messenger. A raw HTTP bearer-capability API is unsuitable
for public exposure even behind a network tunnel without additional
transport authentication, lifecycle monitoring, quotas, and a separate
security assessment. Room state is process-local and vanishes on restart.

## Hardening policy and remaining transport risks

Backend v0.1 accepts a configurable `GHOSTROOM_REQUEST_TIMEOUT_MS`
(default **5000**, range 50–30000 ms). This deadline covers the
handler including body-reading and releases the in-flight permit on
timeout, responding `408 {"code":"request_timeout"}` where possible.
An incomplete HTTP header that has not become an application request
is **outside this handler timeout**; connection/header deadlines and
keep-alive bounds remain server-layer DoS work before onion exposure.

Business operations share a bounded, fail-fast in-flight semaphore;
`GET /health/live` is separate, never consumes that permit, and is
only liveness (not readiness or an assertion about room cleanup).
Creation uses an instance-wide `GHOSTROOM_CREATES_PER_MINUTE`
bucket (default 30, range 1–600). Other POST room operations use
`GHOSTROOM_MUTATIONS_PER_MINUTE` (default 120, range 1–600).
Exhaustion returns `429 {"code":"rate_limited"}` with
`Retry-After`. These limits are intentionally **global**, not
per-IP: Tor circuits, local processes, and source addresses are not
identities. Capacity exhaustion and anonymous local-process abuse
remain possible, and token buckets do not guarantee fairness.

Cross-site browser-style mutation requests are rejected on untrusted
`Origin` or `Sec-Fetch-Site`. Native clients without those headers
are accepted; Host/Origin are not authentication. Bearer schemes are
case-insensitive, duplicate authorization/participant headers are
rejected, and all responses (including errors) use `Cache-Control:
no-store`. Decoded capability wrapper bytes are zeroized on drop;
invitation JSON strings are best-effort zeroized, but this cannot erase
all HTTP/parser/allocator/OS copies.

The server is still **loopback-only HTTP**, with no Tor or TLS, E2EE,
persistent state, messaging, per-client identity, production readiness
or complete socket-level DoS resistance. No performance/load benchmark
or penetration test was conducted.

## Backend v0.2 TCP/HTTP transport limits (development only)

The HTTP/1 listener now enforces connection admission **before** socket
acceptance and parsing, rather than relying on application middleware.
When the connection limit is reached, the accept loop pauses and the
operating system's finite listen backlog provides backpressure. Hyper
limits header read time (including incomplete and trickled headers),
header bytes/count, and idle keep-alive header waits. A separate absolute
connection-age deadline terminates even an active keep-alive peer.

| Environment variable | Default | Valid range |
|---|---:|---:|
| `GHOSTROOM_MAX_CONNECTIONS` | 64 | 1–4096 |
| `GHOSTROOM_HEADER_TIMEOUT_MS` | 5000 | 100–30000 |
| `GHOSTROOM_MAX_CONNECTION_AGE_S` | 60 | 2–3600; must be >= header timeout |
| `GHOSTROOM_MAX_HEADER_BYTES` | 8192 | 8192–32768 |
| `GHOSTROOM_MAX_HEADERS` | 64 | 1–100 |

The transport allows up to ten seconds to drain on SIGINT, SIGTERM or
fatal maintenance failure before aborting remaining connection workers.
Existing body limits, request timeouts, authentication, concealment,
rate limits, no-store responses and loopback-only binding are preserved.

Hyper 1.12.0/Hyper-Util 0.1.21 were already present in the lockfile; this
step uses them directly and does not update their versions. The header
count setting can incur a small allocation per request. No comparable
load, throughput, memory or benchmark measurements are available.
The kernel's finite backlog/FD limits and OS-level local-process abuse
remain outside the in-process cap. Never expose this HTTP listener
publicly. Tor, messaging and E2EE are still absent; **ADR 0002 remains
NO-GO / BLOCKED**. See `docs/TRANSPORT_V0_2.md` for test caveats.
