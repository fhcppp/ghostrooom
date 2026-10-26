# RoomManager security contract and remaining server requirements

**Current status (2026-10-21):** In-memory synchronous `RoomManager`
**implemented and verified at Step 3 HEAD `83fa0a691bd7ac975fd1d7dfafd866b69e2c2871`**.
[Matching successful eight-check CI](https://github.com/fngvarlog/ghostroom/actions/runs/39515539280).
The following contract combines implemented guarantees and separate
**future server requirements**. A **loopback-only experimental HTTP room-control service now exists** in
`server/`, with a bounded periodic cleanup task and no messaging routes.
Transport authentication, ciphertext queues, Tor, durable restart recovery,
external-source rate limiting and production readiness are **not implemented**;
don't interpret future checklist items as completed functionality.

## Ownership and clock trust

- A single owner, lock, or actor must serialize every operation for one room:
  registry lookup, credential verification, invitation consumption, active
  authorization, queue action, closure, expiry, and credential invalidation.
- Do not split authorization from a protected mutation across different lock
  scopes. After room closure, no operation may enqueue, dequeue or transmit.
- Capture time from a trusted server-local monotonic clock **inside** the
  manager operation. Never accept a client-supplied `Instant` or timestamp.
- Store/deal with `Instant` only within a single process lifetime; explicitly
  fail closed on reboot/recovery uncertainty until durable TTL policy exists.
- `SecureRoom::expire_if_due` is crate-private. Implement any room-manager
  maintenance inside the library or design a restricted trusted API with a
  dedicated review. Never expose this mutator directly to network handlers.

## Resource bounds and expiry cleanup

- Set configurable, enforced maximum concurrent rooms, maximum waiting
  duration, maximum active duration, per-source creation/lookup/redemption
  rate limits, and budgeted cleanup work per tick.
- A waiting room's sole invitation expires at its configured deadline;
  it must not keep the room resident indefinitely. An expired terminal
  record must eventually be removed under a bounded retention policy.
- Enforce finite framed message sizes, bounded queues and backpressure
  *before* implementing any message transport.
- Never rely on nondeterministic garbage collection for expiry cleanup.

## Registry uniqueness

- Insert new 128-bit room identifiers atomically only when the key is absent.
- Detect collisions against the actual registry, and use a documented finite
  retry bound. Exhaustion must return a safe error without overwriting any
  existing room or credential verifiers.
- `RoomId::generate()` is cryptographically random, not a registry-level
  uniqueness guarantee. The new sampled ID unit test is not a substitute
  for an injected-collision registry test.

## Mandatory future manager tests

1. Inject repeat/collision room IDs: second insert never overwrites first;
   bounded retry exhaustion fails safely and frees temporary resources.
2. Race many valid redemptions of one invitation: exactly one joins and
   exactly one access capability is issued.
3. Race close and authorized queue action: no post-close operation succeeds.
4. Race expiry at exact deadline against join/close: one terminal outcome
   and original closure metadata are preserved.
5. Exhaust each configured room count, request rate and queue bound: no
   unbounded allocation or work occurs.
6. Verify cleanup reclaims room quota and invalidates access after expiration.
7. Simulate a restart and invalid/uncertain clocks: rooms do not reopen.
8. Verify external errors, timing and logging policy against malformed or
   guessed invitations; avoid disclosing whether a room exists.

The Step 3 manager tests cover bounded collision retries, one-time
redemption races, callback/close ordering, cleanup/poisoning, and quotas
within the in-memory manager. This historical checklist also includes
**future real-queue, restart, network-facing error, and rate-limit
properties**, which cannot be established by manager tests alone.

## Step 3 implementation design and scope

Selected design: a bounded registry mutex plus one mutex for each room.
Registry lookup returns only a private Arc, never a public handle. Room
operations take the per-room lock after releasing the registry lock; no path
holds entry and registry locks simultaneously. OS-backed ID and credential generation plus verifier construction occur
outside the registry lock; only capacity and collision validation followed
by atomic insertion use the lock. In a concurrent insertion race, the
losing candidate and its secret-bearing values are dropped without
exposing credentials. Collision retries remain finite. Room operations on unrelated rooms use separate locks.

Expiry cleanup uses a rotating cursor, selecting at most the configured
budget from the bounded registry, retiring a terminal entry under its mutex,
then removing it under the registry lock only when the instance pointer
still matches. A previously cloned entry cannot be resurrected after
retirement. Poisoned mutexes return `ManagerError::Poisoned`; a poisoned
entry can require administrative process restart rather than silently
discarding unknown room state. The code never returns room locks or Arc
handles publicly.

`with_active` keeps authorization and the caller's bounded synchronous
callback within the same room lock; this establishes only an **in-memory
serialization contract**, not encrypted message delivery. The callback must
not reenter the manager or perform blocking network operations. Message
queues and their atomic close-versus-send tests are future work.

Future transport still requires per-source rate limiting, trusted
timestamp capture, error normalization, queue budgets, authentication
bootstrap, persistence/restart decisions, Tor and client E2EE.

## Distinct waiting and active phases

The configured waiting TTL bounds pre-join admission alongside the invitation
TTL, using the earlier deadline. It must not shorten the post-join session.
After successful invitation redemption the manager installs a new active TTL
measured from the trusted join instant. Regression tests exercise remaining
active after the pre-join timeout and expiry at the exact active deadline.

## Step 3 acceptance disposition (2026-10-20)

Step 3 implements an in-memory bounded room registry, random ID generation
with atomic uniqueness checking and finite collision retries, per-room
synchronous ownership, creator cancellation, one-time redemption, close,
authorized in-memory callbacks and bounded rotating cleanup. It checks
trusted time inside every manager operation. The room entry and registry
mutexes are not exposed in the public interface. Deterministic ID sources
and fake clocks are only available in private unit-test code.

Exact code commit `8499dbb53a962e66c979ea726fc48a98462975df`
passed all eight GitHub Actions gates:
https://github.com/fngvarlog/ghostroom/actions/runs/39514957147

**Deferred:** real message queue enqueue/dequeue/clear semantics, external
rate limiting, persistence, reboot timestamp recovery, production
scheduled cleanup, browser/network response normalization and any Tor
or Android client work. A poisoned room lock fails closed but can occupy
capacity until operator intervention or process restart; do not claim
automatic recovery of poisoned entries. No latency benchmark, concurrency
model checker, property fuzzing or independent review has been run.

## Final targeted race-test additions

The 28 manager tests include targeted barriers for cleanup racing an
already-authorized callback and concurrent creation with deliberately
colliding room IDs. The expanded 61-library-test suite passed all eight
gates at commit `707f83db32d4f74e380ec942086aa8f36f0fbd00`:
https://github.com/fngvarlog/ghostroom/actions/runs/39515394180

Remaining server-stage acceptance criteria still include a real,
bounded ciphertext queue under the same serialization boundary,
network-input validation/error normalization, transport-source rate
limits, scheduler deployment and fail-closed restart semantics.

## Verified current Step 3 head, reconfirmed 2026-10-21

Commit `83fa0a691bd7ac975fd1d7dfafd866b69e2c2871`, runner
https://github.com/fngvarlog/ghostroom/actions/runs/39515539280:
8/8 quality checks passed with stable Rust 1.99.1. Each of four test
configurations passed 61 library tests (28 manager, 31 participation,
2 internal maintenance), 30 Step 1 integration tests, and 1 compile-fail
Rustdoc test. This proves the named checked code's automated suite
completed; not server readiness or an independent audit. No local Cargo
run occurred because the assistant environment lacks `cargo` and `rustc`.

## Backend v0.1 room-control integration (2026-10-21)

The separate `ghostroom-server` binary now calls `RoomManager` for create,
single-use join, credential-gated cancel/close/status and expiration cleanup.
It binds to loopback only, rejects public listeners, sets bounded request
bodies and immediate-concurrency admission, and does not log credentials
or provide message-transfer routes. The authorized `RoomManager::status`
method returns only an ephemeral lifecycle enum, never an authorization
snapshot or lock. General-purpose `with_active` callbacks were removed
to prevent reentrant or unbounded code while the room mutex is held.

This is a **testable development control API**, not a hardened remote
service. There is no TLS, Tor forwarding, E2EE, persistent state or
authenticated encrypted-message queue. Source rate limits and safe
deployment remain future security gates. See README for exact routes.
