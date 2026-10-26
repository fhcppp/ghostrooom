# GhostRoom Step 2 security design

Status: implemented domain foundation only, **not production secure messaging**.

## Security boundaries

`RoomId` is a public routing identifier; `ParticipantId` is a public
room-local label. Neither grants access. `InvitationSecret` and
`ParticipantAccessCapability` are distinct 256-bit credential types.

A server creates a waiting room and returns the creator's capability plus a
single-use invite token. The server retains only a domain-separated SHA-256
verifier (not raw tokens). The invitee presents the invitation once to obtain
a newly generated participant label and access capability. Only a validated
credential permits participant lifecycle operations. The room admits no third
participant. Closing or expiring the room drops all stored verifiers.

Invitation authentication runs before deadline mutation. Therefore random
guesses cannot expire or close a room. For an authenticated request at
`now >= expires_at`, the room expires at the original configured deadline.
For invite redemption at `now >= invitation_expires_at`, redemption is
rejected. A creator can cancel a waiting room; after joining, either
authenticated participant can close an active room.

## Threat model

Assume a trusted monotonic server clock and the host OS random source.
The domain type system prevents substituting room IDs for capabilities.
Verifier comparisons use constant-time comparison after SHA-256 derivation.
An unauthenticated attacker cannot directly mutate a room using its ID.
A leaked invitation token permits a first redeemer to join, however; an
attacker who intercepts it may win the race. Sharing the invitation over a
trusted out-of-band channel is essential.

This module provides no network authentication, Tor routing, anti-DoS,
database integrity, E2EE, peer identity verification, or server anonymity.
Operators may observe connection timing and room membership. A malicious
server can issue false invitations or alter state; client E2EE must defend
message confidentiality against such a server. The server must not accept
network-supplied timestamps or call `expire_if_due` for unauthenticated
client commands.

## Resource controls and concurrency

Per-room storage remains bounded. The future room manager must enforce
caps on rooms, request rates, failed authorization attempts, invite attempts,
room lifetime, message size, queues, and memory. Exactly one mutex/serialized
actor must own each room for atomic invite redemption. Hash-verifier checks
are constant-size; membership is at most two slots. No global maps or
unbounded retries are implemented here. Detect room ID collisions before
insertion and retry with bounded attempts.

## Secrets

Raw secrets reside in creator/invitee-issued values and outside the server
on clients. On the server, only digest verifiers reside in memory; erasure
on closure and on Rust drop is best effort, not guaranteed physical erasure.
Credential bytes may be copied by client transport, OS, or compiler. No
plain tokens should appear in logs or crash reports. Android secret storage
should use platform-backed keystore facilities where suitable, with a clear
threat model for unlocked devices and backups.

## Persistence and platform compatibility

`Instant` values cannot be serialized across reboot. A future durable room
manager must define wall-clock recovery and monotonic re-establishment while
failing closed when deadline state is uncertain. Onion routing, Android
integration, API schema, invitation URLs and QR codes are deliberately
unimplemented. Link encoding must include authenticated instance address,
strict version and length validation, and a safe failure policy.

## Audit hardening: authorization and expiry

The public `SecureRoom::authorize_active(id, capability, now)` method now
combines bearer verification, active-state validation, and trusted monotonic
time. The former public credential-only `authenticate` method has been
removed to discourage misinterpretation. The separate `recorded_state()`
getter is explicitly not authorization and can show a stored state awaiting
an expiry poll. `state_at(now)` is time-aware observation, also not
authorization. A future server must hold an exclusive per-room owner through
authorization and the protected message/queue operation; otherwise a race
could invalidate a previously successful check.

The sole invitation is a waiting room's maximum waiting deadline, even if
the overall room has no deadline. The earliest applicable deadline wins:
if an invitation expires earlier than the room, the waiting room terminates
at the invitation deadline. At equal deadlines the room deadline wins.
Once the invitee joins, the invitation deadline no longer applies.
Both expiry paths remove all stored bearer verifiers and preserve closure
metadata. Creator cancellation and expiry are idempotent terminal actions.

Invitation validity is checked before time and state. Unknown/invalid,
consumed, revoked, or wrong-room invitation secrets get the same generic
`InvalidInvitation` return. A still-valid invitation can receive a
`BeforeCreation`, `InvitationExpired`, or `Expired` outcome.
The generic response is a domain-level error policy; network timing,
serialization, room lookup and error normalization remain unimplemented.
The removed `AlreadyJoined` variant avoids revealing participation status
to someone without an outstanding invitation.

The old ID-only `Room` is exposed through `ghostroom_core::legacy`
only. It has no remote-client authentication and is not the supported
application path. `SecureRoom` and credential-bearing values redact
diagnostic output, including room labels and timestamps.

The future server still needs bounded room and invitation counts, rate caps,
room TTL policy and periodic cleanup, and collision checks during registry
insertion. `SecureRoom` uses a three-attempt bound for participant-ID
generation but cannot ensure global room-ID uniqueness without a manager.

## Method-level contract

| Method | Required authorization | Transition | Time and expiry |
|---|---|---|---|
| `create` | Trusted server creates | New Waiting | Validate future deadlines |
| `join` | Valid outstanding invitation | Waiting → Active | Earliest waiting deadline wins; single use |
| `authorize_active` | Valid room-scoped access capability | Active stays Active or expires | At/after room deadline rejects |
| `cancel` | Creator capability | Waiting → Closed | Earliest waiting deadline wins |
| `close` | Either active participant capability | Active → Closed | Room deadline wins at exact instant |
| `expire_if_due` | Trusted server only | Waiting/Active → Closed | Record original earliest deadline |
| `state_at` | Trusted internal status reader | None | Time-aware logical state; not authorization |
| `recorded_state` | Trusted internal status reader | None | Can be stale; not authorization |
| `participants`, `closure` | Trusted internal metadata consumer | None | Do not return without network authorization |

All synchronous mutations are bounded and perform constant-sized
membership and verifier checks. No measured latency, memory profile, or
thread-safety guarantee is asserted.

## 2026-10-20 status audit follow-up: trusted polling and identifier tests

The attached repository status audit was based on commit
`a451810`, before the verified `e242de0` security remediation.
The actual branch source was rechecked before applying new changes.

`SecureRoom::expire_if_due` now has `pub(crate)` visibility. An
external crate cannot directly run the trusted expiration mutator.
Authorization paths use the same internal transition during deadline
rejection; they still require a valid room-scoped capability before
changing state. The crate includes a Rustdoc compile-fail guardrail and
private unit tests for polling, deadline precedence and terminal
idempotence. This narrows the API surface, but is NOT a proof that a future
transport cannot misuse timestamps: a separate room manager must capture
time internally and avoid accepting clock values from untrusted clients.

The tautological `assert_eq!(r.id(), r.id())` pattern was removed. A
new test creates independent rooms and checks that generated room IDs do
not repeat across a small sample. This is a probabilistic RNG sanity
check only; it does not prove global uniqueness or a cryptographic
property. The future registry must enforce uniqueness with bounded retry,
capacity limits and collision-injection tests.

See `docs/ROOM_MANAGER_REQUIREMENTS.md` for the deferred registry and
concurrent-operation acceptance criteria. Neither transport nor manager
was implemented as part of this follow-up.

## Step 4: client-held cryptography remains research only

The Step 3 `RoomManager` now implements bounded in-memory room
registration and per-room lifecycle serialization, but there is
still no transport, actual ciphertext queue or persistence. Earlier
statements here that a manager is entirely absent describe the
earlier Step 2-era source, not the current HEAD. Actual manager
boundaries and deferred service requirements are recorded in
[`ROOM_MANAGER_REQUIREMENTS.md`](ROOM_MANAGER_REQUIREMENTS.md).

Step 4 **does not add encryption**. The proposed client E2EE
architecture is documented in [ADR 0002](adr/0002-ephemeral-two-party-e2ee.md)
and [E2EE design](E2EE_ARCHITECTURE.md).
A room ID, invite or bearer capability is not an E2EE peer identity.
The self-hosted server is treated as a potential key-substituting
adversary. Both clients need fresh per-room keys, authenticated
out-of-band identity/QR verification, room/role/session transcript
binding and mutual confirmation before user messages are exchanged.
If no authentic OOB channel exists, impersonation cannot be ruled out.
The former conditional Olm profile has **no verified post-quantum
ratchet and is rejected for mandatory first-release PQ confidentiality**.
PQ is **mandatory**, not an open product preference. Android/client API
compatibility, third-party support, anti-downgrade enforcement and
qualified licensing review remain open gates. No client private keys
should enter server memory.

## Step 4B final protocol decision (2026-10-21)

**ADR 0002 is NO-GO / BLOCKED**. Mandatory first-release post-quantum
confidentiality means the former conditional vodozemac/Olm candidate
cannot be used for production. Signal's PQXDH + Triple Ratchet
implementation is the nearest research target, but unsupported external
use, exact Android API/profile behavior and AGPL licensing are unresolved.
The manager remains an in-memory room/capability authority, not an E2EE
endpoint; neither the server nor the current code encrypts user messages.
Peer authentication still requires fresh room-scoped identities and
mutually confirmed authenticated OOB transcript verification, independent
of invitation and bearer capability possession. No claims of PQ
post-compromise recovery, secure deletion or endpoint compromise
resistance are made without evidence and the later review gate.

## Backend v0.1 foundation clarification

The in-memory RoomManager is the supported application-facing
room-lifecycle boundary. SecureRoom mutations remain crate-private.
Room and participant IDs are labels, **not** authorization tokens;
participant capabilities and server-owned time protect operations.
A loopback-only experimental HTTP room-control server now exists in
`server/`. This does **not** imply Tor, production encrypted messaging,
a ciphertext queue, Android UI, durable persistence or approved E2EE. ADR 0002 remains NO-GO.

## Backend v0.1 control-plane boundary

The HTTP server accepts invitation and participant capabilities as
non-URL fields, delegates state transitions to the in-memory manager,
and returns only minimal status to authorized participants. Invalid
bearers and unknown room IDs have the same external error response.
Responses containing newly created credentials are marked
`Cache-Control: no-store`. A fail-fast semaphore bounds in-flight
application requests; a body limit restricts each request. A loopback
listener is enforced at startup. A background task performs bounded
cleanup and stops on graceful server shutdown.

**Not yet guaranteed:** TLS, Tor-only transport, protection from local
process compromise, distributed rate limiting, durable restart recovery,
secure client key storage, E2EE, or production-grade DoS resilience.
ADR 0002 stays **NO-GO / BLOCKED** for encryption.
