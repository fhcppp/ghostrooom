# Disposition: GhostRoom Repository Status Audit (2026-10-20)

Source: user-supplied 8-page `GhostRoom_Repository_Status_Audit_2026-10-20.pdf`.
The report observed commit `a451810`; direct inspection of the branch
after earlier remediation found current commit `e242de0`. The old PDF
correctly identified historical hazards but its commit/CI/test counts
became stale. This disposition refers to actual source at the later head.

| Finding | Current disposition | Evidence / next action |
|---|---|---|
| F-01: ID-only `Room` exported | Fixed previously; verified | Root exports only `SecureRoom`; ID-only `Room` sits under `legacy` |
| F-02: authentication without deadline | Fixed previously; verified | `authorize_active` checks capability, state, time |
| F-03: expired invitation strands room | Fixed previously; verified | Earliest waiting-room deadline closes and revokes |
| F-04: invitation lifecycle leak | Fixed previously; verified | Invalid/consumed tokens give generic `InvalidInvitation` |
| F-05: public trusted poll | Fixed and verified on GitHub CI | `expire_if_due` is now `pub(crate)`; compile-fail guardrail and direct private tests |
| F-06: generated-ID collision | Deferred to room manager | Registry uniqueness and bounded retry cannot be guaranteed inside one room object |
| F-07: tautological ID test | Fixed and verified on GitHub CI | Removed self-comparison; sampled independent room IDs |
| F-08: manager/atomicity/quotas | Deferred to next server phase | No server exists; see `docs/ROOM_MANAGER_REQUIREMENTS.md` |

F-01–F-04 were already in the earlier branch and had passed the mandatory
eight-command Linux CI gate with 60 integration tests. F-05/F-07 results
must be recorded from the final updated CI run. The source-level
restriction on internal maintenance is not a substitute for review of
actual network adapters. There is no Tor client, E2EE, Android app,
persistent registry, or concurrent server in this repository.

Review the latest branch and pull request at:
https://github.com/fngvarlog/ghostroom/pull/1

## Verified follow-up

Source commit `1e2a34bd30d5ab7871b4d524b05a055cf1d13a28`
passed all eight gates on GitHub Actions:
https://github.com/fngvarlog/ghostroom/actions/runs/39507838360

The `compile_fail` Rustdoc example rejects access to the private
maintenance method from outside the crate. Two private maintenance tests
and 31 integration participation tests completed successfully. Remaining
manager-level collision, request/room capacity, atomicity, and cleanup
requirements are explicitly deferred.

## Step 3 follow-up

An in-memory registry/room manager is now under implementation on a
separate Step 3 review branch. It aims to address earlier deferred F-06
(global room-ID collision checking) and F-08 (bounded per-room ownership,
race-safe cleanup and quotas), but **do not mark either verified** until
the exact Step 3 commit passes concurrency and cleanup tests. Transport
rate limiting, message queue atomicity and durable restart semantics
remain explicitly outside Step 3.

## Implemented Step 3 manager boundary

F-06: the in-memory RoomManager now performs registry-level atomic
collision checks, finite retries, and deterministic collision tests;
this addresses the specific in-process uniqueness issue. It is not a
persistence or cross-restart uniqueness guarantee.

F-08: the manager now includes per-room serialization, room limits,
bounded cleanup and deterministic concurrency tests, but the complete
real-service F-08 requirement is **partially addressed**, not closed:
there are no network identities, external rate limits, persisted TTL
recovery, actual queues or transport integration.

Code commit `8499dbb53a962e66c979ea726fc48a98462975df`
passed the eight-gate workflow:
https://github.com/fngvarlog/ghostroom/actions/runs/39514957147

## Step 4 reconciliation (2026-10-21)

Step 3 implementation and tests address the in-process F-06 collision
problem and selected F-08 bounded/concurrent in-memory manager work,
with matching successful HEAD commit `83fa0a691bd7ac975fd1d7dfafd866b69e2c2871`
at https://github.com/fngvarlog/ghostroom/actions/runs/39515539280.
F-08 remains **partially resolved** until the real server enforces
network quotas, queue close-versus-send ordering, scheduler lifecycle,
and durable restart semantics. No independent security audit is implied.
