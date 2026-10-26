# GhostRoom Step 3 — RoomManager implementation record

## Architecture (2026-10-20)

Selected: synchronous per-room mutexes in a capacity-bounded
`BTreeMap<RoomId, Arc<Mutex<Entry>>>` protected by a separate global
registry mutex. This enables independent room operations without a global
lock for every cryptographic verification or lifecycle transition.
No Tokio, additional dependencies, database or socket is added.

Trade-offs: `BTreeMap` has O(log N) lookup, insertion and deletion;
entry operations are constant-sized, excluding operating-system lock
contention. Creation generates candidate IDs, secrets and SHA-256 verifiers outside
the registry lock. The lock is held only for the authoritative capacity
and collision check followed by atomic insertion. A concurrent capacity
race or ID collision drops the unsuccessful secret-bearing candidate,
without ever publishing it. Creation may repeat this work up to the
configured finite collision-retry bound.

Lock order: the manager clones a private Arc under registry lock,
releases it, and only then takes the entry lock. Cleanup retires under
entry lock, releases it, and then removes under registry lock if the
Arc pointer still matches. No entry-to-registry lock acquisition occurs
under a held entry lock; no private handle/guard is returned publicly.
Poisoned locks fail closed; they are not blindly recovered.

## Limits

All configuration values are validated nonzero. Waiting TTL and
invitation TTL derive from a manager-owned clock; the earlier deadline
wins. The waiting deadline stops applying on a successful single-use join;
the active phase has a separate maximum lifetime measured from the join.
Room count and insertion retry loops are bounded. Cleanup scans at most
the configured number of rooms in a pass, with round-robin progress.
Room counts can remain at capacity until the next scheduled cleanup;
the future scheduler must call cleanup regularly.

## Collision and lifecycle

Room ID generation is OS-backed; a private deterministic source is
injected only for unit tests. The registry checks and inserts atomically,
never overwrites existing entries, and has finite collision retries.
Failed retries/failures do not insert partial rooms. A retired entry cannot
authorize a late operation even if a stale private Arc survives. The
public `with_active` callback runs under the same lock as close/join and
must be short, bounded, synchronous, non-reentrant and in-memory.
This is **not** a message queue, not a send operation.

## Tests and evidence

Existing 30 Step 1 tests and 31 Step 2 participation tests are kept.
Step 2 mutation methods now require crate access; their regression
suite moved to an internal Rust test module. Step 3 adds registry,
collision, expiration, capacity, callback ordering and concurrency tests.
Final observed CI result and test counts are recorded in `VERIFICATION.md`
after the actual final run. Do not infer CI success from an earlier commit.

## Deferred boundaries

No network transport, Tor, E2EE, real encrypted queue, Android app,
database, persisted clock, transport per-IP/source rate limiting,
performance benchmark or external security audit is implemented.
A manager cannot enforce privacy guarantees of an unbuilt network.

## Final observed test and verification results

The reviewed code commit
`8499dbb53a962e66c979ea726fc48a98462975df` was verified by
[GitHub Actions](https://github.com/fngvarlog/ghostroom/actions/runs/39514957147)
using stable Rust 1.99.1. All eight required gates passed. The four
debug/release (ordinary/locked) test configurations each passed 59
library tests, 30 Step 1 lifecycle integration tests, and one Rustdoc
compile-fail API-visibility test.

The 59 library tests consist of 26 Step 3 room-manager tests, 31 retained
Step 2 participation tests relocated to in-crate scope after mutating
`SecureRoom` APIs became crate-private, and 2 trusted-expiration tests.
The suite includes deterministic injected ID collisions and exhaustion,
ID-source failure, concurrent room-capacity attempts, 12-thread races
redeeming the same invitation, join/cancel races, same-lock
callback/close ordering, deadlines at their exact boundary, separate
waiting-vs-active TTL behavior, bounded rotating cleanup, retained
stale-entry invalidation, ID reuse after cleanup, and poisoned locks.
Callback panic tests verify fail-closed poisoned room behavior.
No arbitrary sleeps were used to synchronize the thread races.

**Implementation scope:** this step delivers an in-memory registry and
serialization contract only. It does not deliver a network server or real
ciphertext queue. Queue-level atomic close-versus-send remains untested
because no real queue exists; a future queue must be integrated with
the same room serialization boundary. Other missing work includes
periodic scheduler integration, per-transport-source rate limits,
persistent state/restart recovery, Tor-only transport, E2EE/crypto
protocol choice, Kotlin Android clients, benchmarks, fuzzing,
vulnerability scans and independent security review.

## CI maintenance

The workflow uses Ubuntu 24.04 explicitly and pins
`actions/checkout` v6.1.0 to immutable upstream Git commit
`d23441a48e516b6c34aea4fa41551a30e30af803`, verified against
[GitHub's official action tag](https://github.com/actions/checkout/releases/tag/v6.1.0).
It retains stable Rust and all eight checks.

No benchmark was run: O(log N) registry updates plus bounded cleanup
are reasoned complexity bounds, **not measured throughput**. The
registry insert phase is short-held; room entry operations use
independent locks. Secret creation executes outside the registry lock
and failed race candidates are discarded.

## Additional targeted concurrency regressions

Two focused tests extend the manager suite beyond the first verified code
baseline: one holds an authorized callback under the per-room mutex while
cleanup races after the deadline, proving cleanup waits for the callback
and ultimately retires/removes the room. The other starts two concurrent
creators with an injected sequence of colliding IDs and verifies that
the final registered IDs are distinct and neither room is overwritten.
Results for these tests must be taken from the subsequent CI run.

## Final expanded concurrency-suite evidence

Code commit `707f83db32d4f74e380ec942086aa8f36f0fbd00` passed
[all eight CI checks](https://github.com/fngvarlog/ghostroom/actions/runs/39515394180).
This run supersedes the earlier 59-library-test baseline after two further
manager regressions were added.

**Observed:** 61 passing library tests per configuration (28 Step 3
manager, 31 Step 2 participation, 2 internal maintenance), 30 passing
Step 1 lifecycle integration tests, and 1 passing Rustdoc compile-fail
test. The suite passed in debug, release, debug-locked, and release-locked
configurations, all on stable Rust 1.99.1.

Additional manager tests use barriers, not sleeps, to demonstrate
cleanup waiting for an in-flight protected callback and two concurrent
creators safely handling repeated injected room IDs. No mutable room
reference is returned to external callers. No queue-level acceptance
or deletion was tested because there is no queue implementation.

The final documentation-only commit must itself pass CI before any
merge; CI success is not an independent security or performance audit.

## Status reconciliation, 2026-10-21

The latest observed matching successful code+documentation HEAD is
`83fa0a691bd7ac975fd1d7dfafd866b69e2c2871`,
[Actions run 39515539280](https://github.com/fngvarlog/ghostroom/actions/runs/39515539280).
The previously recorded 61 + 30 + 1 suite, including 28 Step 3
manager tests, passed all eight gates. The implementation is an
in-memory manager only, not a running server or message queue.
The new Step 4 ADR describes potential future **client** E2EE
integration; it does not alter this manager's security contract.
