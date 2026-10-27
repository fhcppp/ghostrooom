# Backend v0.2 — transport hardening candidate

**Status: NOT ACCEPTED / NOT VERIFIED.** Source patch only. This environment has
no `cargo`/`rustc`, direct GitHub cloning fails DNS resolution, and no compiled
TCP tests, performance baseline, or Rust quality gate was possible. This patch
must pass all requested checks on a machine with the pinned Rust toolchain.

## Scope

Connection ownership moves from `axum::serve` to one explicit Hyper HTTP/1
connection loop. Hyper is already in the lockfile at 1.12.0; Hyper-Util is
already present at 0.1.21. No HTTP parser or crypto implementation is added.

The accept loop waits for a connection semaphore permit *before* `accept`.
Saturation therefore pauses the accept loop and leaves new connections in the
OS listener backlog. This is intentional backpressure; the OS backlog is finite
but its precise size is platform-defined. A `JoinSet` is reaped continually,
including successful/failed socket completions; the number of live connection
workers never exceeds the number of permits. Worker drop releases its permit.

Hyper enforces an absolute header-read deadline on HTTP/1 request headers
(including silent, incomplete and trickled headers); it is not middleware.
Idle HTTP keep-alives awaiting a subsequent request are subject to the same
header deadline. Independently, a hard maximum connection age aborts sockets
whose total lifetime expires, even if a client keeps sending requests.

Hyper explicitly enforces header bytes/count and its buffer size, responding
431 for oversize requests where possible. Malformed parser input and peer
failures are connection-local and do not expose request contents in logs.

On shutdown the accept loop closes; workers ask Hyper to drain current HTTP/1
requests and are cancelled after a ten-second shared grace deadline.

## Defaults and ranges

| Environment variable | Default | Supported |
|---|---:|---:|
| `GHOSTROOM_MAX_CONNECTIONS` | 64 | 1–4096 |
| `GHOSTROOM_HEADER_TIMEOUT_MS` | 5000 ms | 100–30000 ms |
| `GHOSTROOM_MAX_CONNECTION_AGE_S` | 60 s | 2–3600 s; must be >= header timeout |
| `GHOSTROOM_MAX_HEADER_BYTES` | 8192 bytes | 8192–32768 bytes |
| `GHOSTROOM_MAX_HEADERS` | 64 | 1–100 |

Existing request-body limit, handler timeout, in-flight request semaphore,
rate limits, bearer authentication, credentials and loopback-only bind
restriction are unchanged. The existing SIGINT/SIGTERM and cleanup-triggered
shutdown logic drives the transport's new bounded drain.

## Performance/limits and future testing

No comparable measurements exist. `max_headers` explicitly configures Hyper's
parser and may introduce a small per-request allocation (per Hyper docs);
there is no lock on socket reads. Tests must measure memory and throughput
under realistic local load before any performance conclusion.

The kernel's accept backlog/file-descriptor accounting is outside the
in-process semaphore. Under extreme file-descriptor exhaustion, the transport
backs off on accept errors but does not implement reserve-FD exhaustion
recovery. HTTP/1 keep-alive reuses connections; a finite age bounds each one.
No TLS, Tor, public exposure, E2EE, relay, WebSocket or persistent state.
ADR 0002 remains NO-GO / BLOCKED.
