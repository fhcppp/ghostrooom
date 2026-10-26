# GhostRoom Step 1 implementation report

## Architecture / exact source tree

```text
ghostroom-core/
├── .gitignore
├── Cargo.toml
├── Cargo.lock
├── LICENSE
├── README.md
├── RESEARCH.md
├── STEP1_REPORT.md
├── VERIFICATION.md
├── check_output.txt
├── run-verification.sh
├── src/
│   ├── error.rs
│   ├── ids.rs
│   ├── lib.rs
│   └── room.rs
└── tests/
    └── lifecycle.rs
```

* Synchronous library only; no runtime, network, Tor, database, crypto or build
  script. Zero Cargo dependencies.
* `RoomId` and `ParticipantId`: distinct `u128` newtypes with explicit
  deterministic constructors (NOT credential proof / NOT production RNG).
* `Room` owns its two participants as `[ParticipantId; 2]`, creation instant,
  optional deadline and state; all fields private.
* `RoomState`: `Active` or `Closed(Closure)`. Closure metadata is a single
  value, so there is no separate mutable closure reason/time to diverge.
* `Room::new`: validates distinct members and strictly future expiration.
* `Room::is_active_at`: pure observation; accounts for time crossing a deadline
  even if no transition has been recorded yet. `state()` returns stored state.
* `Room::require_active`: first checks membership, then validates temporal
  activity and records expiry. `close` follows the same ordering.
* `Room::expire_if_due`: documented trusted server-only path, idempotent and
  records the deadline (rather than delayed polling time).
* All transition methods return errors or explicit poll outcome; terminal
  closure fields can be inspected but never modified.

## Complexity and resource use

Membership checks two `u128` values: O(1) time and O(1) space. Creation,
activity check, expiration and closure are O(1), with no scans or dynamic
collections. The `Room` domain representation has fixed-size members and
fixed-size enums; it holds neither messages nor heap-backed strings/vectors.
There are no explicit allocations in transition code. Actual byte size and
runtime throughput were not benchmarked; they depend on compiler layout and
future server implementation.

## Coverage checklist

- [x] Two distinct participants / fixed-size storage / compact IDs
- [x] No deadline and strictly valid optional deadline
- [x] Active / terminal Closed with reason and logical timestamp
- [x] Participant 1 or 2 closure; outsiders rejected before mutation
- [x] Inclusive expiry `now >= deadline`, with deadline priority
- [x] Delayed detection records original deadline
- [x] Active operations reject expired/closed and never reopen a room
- [x] Repeated closes and repeated expiration polls preserve metadata
- [x] Explicit supplied times; no test sleeps or timers
- [x] Private fields / read-only closure metadata access
- [x] Domain errors / documentation / deterministic test cases
- [x] Sync-only Rust design and dependency-free Cargo manifest
- [x] Repository research and deferment analysis
- [ ] Compiled, linted, documented, and tested using stable Rust: blocked by missing toolchain and DNS failures in execution environment; see VERIFICATION.md
- [ ] CSPRNG room ID, secret invitations, authenticated capabilities: deferred
- [ ] Network, Tor, cryptography, Android, storage, cleanup worker: deferred

## 2026-10-20 Step 1 audit against the actual GitHub `main` tree

GitHub `main` at commit `702ad60423d6f53fad0ad4111149f40ae1902ad3`
contained the exact previously packaged Step 1 source, confirmed by matching
Git blob IDs for all thirteen common source/documentation files. The remote
repository also tracked `Cargo.lock` and **70 files under `target/`**, but did
not track `.gitignore`. This is a repository hygiene issue: generated build
outputs should be untracked using `git rm -r --cached target`, without deleting
local build artifacts, and `/target/` should be ignored. Retain `Cargo.lock`.

Source review found the core state machine consistent with the documented
single-process Step 1 model. This review does not establish runtime correctness.
The available local changes add four deterministic lifecycle boundary tests,
prohibit unsafe code at crate scope, add `.gitignore`, expand the verification
script to all six mandatory commands, and ensure that script signals failure.
The test count is now **30 written tests; none has been executed** here.

**Step 1 acceptance gate is not satisfied.** The Rust toolchain could not be
installed because package servers were unreachable (DNS failure); do not
transition to Step 2. This report is an audit/preparation record, not evidence
of successful compilation or a production security audit. No GitHub branch,
commit, or push was made as part of this review.

## Security / correctness boundaries

A `ParticipantId` does not attest to the identity of a network caller.
Similarly, `now` must be captured by the trusted server clock; allowing a remote
client to backdate time would bypass the intended deadline semantics. The
transport **MUST** validate a secret room-scoped credential before invoking
participant transitions. `expire_if_due` is trusted infrastructure only; the
future transport must never offer it as an unauthorized client endpoint.

`Instant` is monotonic for in-process lifetime calculations but not a durable
serialization format. Future durable deadlines should define how clock changes,
restart downtime and expiry are handled; do not cast `Instant` to wall-clock
seconds. Server-side state removal is not implemented: marking a room closed
is not equivalent to deleting messages or erasing all copies, snapshots,
logs or client history.

A future room manager must limit creation rate, active room count, retained
closure count and memory. All access and mutations for a room should be
serialized per-room to establish exactly one terminal outcome. The auth check
must be performed before state access when it can cause mutation. An operator
will need explicit logging rules that avoid storing message plaintext and
identifiable metadata.

## Next-step API contracts to preserve

1. Keep `RoomId` and `ParticipantId` transport-independent; generate real IDs
   and validate room-scoped capabilities outside the domain core.
2. Preserve the Step 1 `Active -> Closed` terminal transition, immutable original
   closure metadata and expiration-wins-at-deadline semantics. Step 2 may add a
   `WaitingForParticipant` state before `Active`, including waiting-room closure.
3. Serialize transition attempts with exclusive room access; avoid race-prone
   checks that happen before acquiring room-manager synchronization.
4. Reuse `require_active` before any later message enqueue/dequeue command,
   and couple closure to queue deletion so no post-close operations succeed.
5. Keep the in-process monotonic-clock model explicit until an independently
   reviewed persistence/restart strategy is developed.
