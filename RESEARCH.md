# GhostRoom Step 1 — concise reference review

Reviewed on 2026-10-20 from public repository pages and documentation. These
are design references, **not** code dependencies or claims of security audits.
No source code from these repositories is copied into the room engine.

| Reference | What was supported by reviewed sources | Lesson for Step 1 | License / caveat |
| --- | --- | --- | --- |
| [SimpleX Chat](https://github.com/simplex-chat/simplex-chat), [architecture](https://github.com/simplex-chat/simplex-chat/blob/stable/docs/SIMPLEX.md) | Avoids global user identifiers, puts durable message data on clients, uses temporary relay handling and supports independently run infrastructure; Android client exists. | Keep domain participant IDs room-scoped; do not build a user directory, treat deletion of room control state separately from client message retention. | AGPLv3 listed in repo README; branding/assets carry separate conditions. Upstream has published assessments, not an audit of GhostRoom. |
| [SimpleXMQ](https://github.com/simplex-chat/simplexmq), [protocol v20](https://github.com/simplex-chat/simplexmq/blob/stable/protocol/simplex-messaging.md) | Distinct queue addresses/roles, versioned wire protocol, authenticated commands, `ACK` and `DEL`, storage-duration limits and in-memory persistence recommendations. Explicit delete must clear queued content before success. | Distinguish authorized access, logical closure, message-queue deletion and eventual physical pruning. Reject expiry at use time rather than trusting periodic cleanup. | AGPLv3. Its queue and protocol complexity are outside Step 1. |
| [chattor](https://github.com/jmsingleton/chattor), [manifest](https://github.com/jmsingleton/chattor/blob/main/Cargo.toml) | Rust terminal P2P messenger using embedded Arti onion services, local SQLCipher, Signal-style ratchet via `libsignal-dezire`, backoff and connection pools. | Small domain boundaries integrate into richer Rust applications, but none of its P2P networking, identity or cryptography belongs here. | MIT OR Apache-2.0 as its README states. It lists `libsignal-dezire`, which is **not** equivalent to the official libsignal implementation merely because of its name; cryptographic correctness, maintenance and independent audit status were **not established** and must be assessed before adoption. |
| [Orbot Android](https://github.com/guardianproject/orbot-android), [service implementation](https://github.com/guardianproject/orbot-android/blob/master/app/src/main/java/org/torproject/android/service/OrbotService.java) | Real Android VPN/foreground-service lifecycle, Tor status reporting and SOCKS endpoint information; startup/shutdown and platform-service behavior need explicit handling. | Later Android client should depend on a known Tor state and fail closed on Tor loss, rather than allowing an implicit direct fallback. | Root license identifies Orbot as 3-clause BSD; bundled Tor and other components have their own notices. Android platform permissions/background restrictions require later testing. |
| [Ricochet Refresh](https://github.com/blueprint-freespeech/ricochet-refresh), [design](https://github.com/blueprint-freespeech/ricochet-refresh/blob/main/doc/design.md) | Peer-to-peer Tor onion services, contact identities at onion addresses, contact acceptance, authentication, and protocol negotiation; no store-and-forward relay in the basic connection. | Shows a different trust model: no relay operator, but each peer must host/be reachable and coordinate online availability. GhostRoom instead plans self-hosted relay instances and transient rooms. | Checked root LICENSE contains 3-clause BSD text and third-party notices. An old issue alleges a Commons Clause change; treat branch/history and reuse rights as needing separate verification before copying any code. |
| [Arti](https://gitlab.torproject.org/tpo/core/arti), [official site](https://arti.torproject.org/), [FAQ](https://arti.torproject.org/FAQs/) | Official site reports proxy and onion service support ready for use as of 2026; still lacks some C Tor features and developer APIs may evolve. | Reassess embedded Arti against externally managed Tor later; avoid coupling the core domain to a particular Tor library. | GitLab source repository returned HTTP 403 in this environment, so source-tree details and its precise repository license were **not directly verified** here. Official website was accessible. |
| [Tokio](https://github.com/tokio-rs/tokio), [bounded channels guide](https://tokio.rs/tokio/tutorial/channels) | Async I/O/runtime concerns, task cancellation and backpressure; Tokio warns unbounded queues can exhaust memory. | Keep the domain library synchronous; manager later applies synchronization, bounded channels and concurrency quotas. | MIT listed in upstream README. No async runtime needed for deterministic state transitions. |
| [Rust ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html) | Ownership, borrowing and drop-based resource management allow safe layouts without a garbage collector. | Represent participant pair as an array and IDs as `u128`; borrow `&mut Room` for transitions, return immutable copied metadata. | Official documentation; generic design principles only. |
| [Criterion.rs](https://github.com/criterion-rs/criterion.rs) | Statistics-based microbenchmarking on stable Rust. | Do not confuse O(1) structural reasoning with measured throughput; benchmark only when real workloads exist. | MIT OR Apache-2.0. Not added as a dependency. |

## Peer-to-peer versus server-relayed

* In **P2P onion service** applications (Ricochet, chattor), a peer operates a
  service and hosts state; establishing a connection depends on peer reachability.
  There is no independent rendezvous/storage relay, but identity availability,
  reconnections and operational load move to the peer.
* In **relay-based** systems (SimpleXMQ and planned GhostRoom), the server can
  remain reachable independently of the peer and temporarily store encrypted
  payloads, but its operator can observe operational metadata unless carefully
  minimized. Even with E2EE, a malicious relay can delay, drop or replay traffic.
* GhostRoom's Step 1 implements **neither** transport model. Its room state
  machine merely says whether later components may perform an operation.

## Risks and decisions deferred

1. The engine knows participant **identifiers**, not authenticated identities.
   Later clients must prove possession of room-scoped capabilities; a request
   must never be authorized simply by supplying an arbitrary `ParticipantId`.
2. IDs need a CSPRNG, collision checks, secrecy where appropriate, and strict
   room scoping. Deterministic constructor values are **test-only**.
3. Room closure must be coupled in a later transactional/serialized manager to
   message queue deletion, no further network acceptance and timely map eviction.
   Zeroization does not follow automatically from `RoomState::Closed`.
4. `Instant` is process-relative and cannot be reconstructed after a restart.
   Persistence requires an explicitly specified deadline restoration policy.
5. Concurrent close-versus-expiry events must be serialized per room. Background
   cleanup alone cannot enforce expiry; every authenticated operation checks it.
6. Onion-only routing/fail-closed enforcement, E2EE protocol and crypto review,
   Android lifecycle, replay protection, transport limits, privacy logging and
   self-hosting operations are deferred in full to later development steps.
7. Security claims and the current maintenance status of all upstream projects
   are not implied by code availability or this documentation-level review.
