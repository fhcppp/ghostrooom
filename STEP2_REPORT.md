# GhostRoom Step 2 implementation report — 2026-10-20

## Changes and design

Existing Step 1 `Room` remains source compatible. New `SecureRoom`
introduces `WaitingForParticipant → Active → Closed`, including cancellation
or expiry directly from the waiting state. Implemented:
- OS-backed 128-bit room and participant identifiers (`getrandom` 0.3);
- separate, 256-bit, redacted invitation and participant-access secret types;
- SHA-256 room/participant-bound verifiers with constant-time comparison;
- one-time invitation redemption, activation, replay rejection and no third
  participant, independent invite and room deadlines, original deadline
  recording, and credential revocation on closure/expiry;
- deterministic monotonic-time lifecycle tests, room and credential checks;
- safe Rust, bounded per-room memory, no threads or network layer.

Added `src/credentials.rs`, `src/secure_room.rs`,
`tests/participation.rs`, security research, design and E2EE ADR. Updated
`src/lib.rs`, `src/ids.rs`, `Cargo.toml`, `Cargo.lock`,
`README.md`, verification documentation and the GitHub Actions gate.

## Security observations

- Authenticated participant requests are checked before changing room state.
- Strongly random capabilities are not interchangeable with public IDs.
- Secret types redact Debug and use best-effort zeroization of owned bytes.
- Server stores fixed-size one-way verifiers; exact original expiration deadline
  is preserved. An invitation-only attacker can win the first redemption race;
  trusted out-of-band distribution remains essential.
- Future atomicity requires an exclusive room-manager owner. The library's
  ordinary `&mut self` protects individual in-process transitions only.
- No E2EE, message transport, Tor-only enforcement, Android UI, durable
  timestamp restoration, anti-DoS manager, or independent audit yet.

## Research

Research date: 2026-10-20. See
`docs/research/step-2-security-research.md`, `docs/SECURITY_DESIGN.md`,
and `docs/adr/0001-future-e2ee.md`. Relevant sources include
SimpleXMQ's 2026-05-25 protocol revision, current getrandom documentation,
and NIST FIPS 203 and its errata planning note. Source code has not been
copied from those projects.

## Verification

Step 1 baseline GitHub Actions run
https://github.com/fngvarlog/ghostroom/actions/runs/39494030406
passed all six checks with stable rustc 1.99.1.

Step 2 CI run
https://github.com/fngvarlog/ghostroom/actions/runs/39495303162
passed formatting, check, strict Clippy, debug tests, release tests and docs
on commit `31fa9b9c3e7c58207c5d13ad066225de1e09696c`.
It ran 30 original lifecycle tests and 22 new participation tests.
An intermediate run failed on one mistaken test expectation and was fixed.
Later CI also passed with the same 52 tests. These results are observed for
the named commits; the final strict-format and committed lockfile revision
must be verified by its own workflow run before claiming the final tip is green.

No performance benchmark, penetration test, Android-target compilation,
dependency audit, or full threat model was executed.

## Final committed-source verification

Final strict CI run:
https://github.com/fngvarlog/ghostroom/actions/runs/39495597918

At commit `38411b1b776bcfbc05e67bdec3ec50ad407f4b05`, all six
verification commands passed against the committed, formatted source and
resolved `Cargo.lock`. The run uses stable Rust 1.99.1 and reports 30
Step 1 tests plus 22 Step 2 tests passed in both debug and release modes.
This is not an external security audit.

## Remediation audit GR-01 through GR-13 (2026-10-20)

Original audit: `GhostRoom_Repository_Audit_2026-10-20.pdf`.

- GR-01 — Fixed and verified by CI: replace public credential-only
  `authenticate` with capability + active state + trusted-time
  `authorize_active`. Deadline regression tests added.
- GR-02 — Fixed and verified by CI: export ID-only `Room` and `RoomError`
  through `legacy`, no longer as root exports. App-facing code uses
  `SecureRoom`. This is a deliberate API break in an early-stage library.
- GR-03 — Fixed and verified by CI: waiting rooms terminate when the only
  invitation expires; no unjoinable indefinite room without TTL.
- GR-04 — Fixed and verified by CI: invalid or consumed invitation tokens
  return generic `InvalidInvitation` before time and lifecycle checks.
- GR-05 — Fixed and verified by CI: `state_at(now)` observes logical
  expiration; `recorded_state()` is explicitly stale-capable and not auth.
- GR-06 — Deferred: manager-level room/attempt counts and TTL caps require
  a server-owned room registry. Waiting-room lifetime is now finite.
- GR-07 — Fixed and verified by CI: `SecureRoom` and issued credential
  bundles use fully redacted custom Debug implementations.
- GR-08 — Deferred: registry collision handling is impossible without a
  registry; implement bounded retry and deterministic collision tests there.
- GR-09 — Deferred: `&mut self` serializes one owned object, not network
  operations; future manager must serialize per-room lookup/auth/mutation.
- GR-10 — Fixed and verified by CI: bounded three-attempt participant ID
  collision retry. Mocked entropy collision test deferred to manager layer.
- GR-11 — Deferred: deterministic ID constructors remain for Step 1 tests;
  isolated legacy `Room` and security documentation clarify that numeric
  IDs are never bearer credentials.
- GR-12 — Deferred transport-only: domain error values must be mapped to
  generic public response classes without disclosing detailed internals.
- GR-13 — Fixed and verified by CI: tests assert exact fully redacted
  Debug forms rather than checking for a substring.

This is a domain review, not an independent security audit, a production
concurrency test, or certification. New product features are blocked until
the mandatory CI gate is green. See `VERIFICATION.md`.

## Remediation verification evidence and code review

Commit `8abe20f105dd1ba892a0a22f986b82566a3564a2`:
https://github.com/fngvarlog/ghostroom/actions/runs/39505118813

Observed Rust version: `rustc 1.99.1`. All eight required commands
completed successfully: `cargo fmt --check`, `cargo check
--all-targets --all-features`, strict `cargo clippy`, normal debug
and release tests, `cargo test --locked`, `cargo test --locked --release`,
and `cargo doc --no-deps`. Each of the four test runs executed
30 legacy lifecycle plus 30 credential-gated participation tests
without failures. `Cargo.lock` was not changed by the remediation.

Source changes: `src/secure_room.rs` (deadline-aware authorization,
privacy-preserving invitation admission, waiting expiry, bounded ID retry,
redacted diagnostics), `src/lib.rs` and `src/room.rs` (legacy API
isolation), `src/credentials.rs` (avoid unnecessary verifier Clone),
`tests/lifecycle.rs` (legacy import) and `tests/participation.rs`
(eight deterministic regression tests plus updated policy expectations).
Documentation and workflow changes are described above.

Important remaining tasks: a future room registry must atomically own
per-room operations, enforce room count/rate/TTL limits, check IDs
for collisions, normalize network errors, and provide cleanup. External
security review, fuzzing, Android checks, Tor tests, performance measurement,
and dependency advisory scanner execution remain unperformed. These do not
become safe merely because the in-memory domain tests pass.

## Follow-up to repository status audit (20 October 2026)

The 8-page `GhostRoom_Repository_Status_Audit_2026-10-20.pdf` was
checked against the **actual** development branch, not only its stale
observed commit `a451810`. F-01 to F-04 were already fixed by the
preceding remediation commit. F-05 remained confirmed: the trusted
`expire_if_due(now)` mutator was public. Its visibility is now
`pub(crate)`; internal unit tests and a Rustdoc compile-fail test
cover the access boundary. F-07 was also confirmed: an ID equality
assertion compared the same expression against itself. It was removed
and replaced by an independent-ID sampling regression test.

F-06 (global registry ID uniqueness) and F-08 (real concurrent ownership,
quota enforcement and cleanup) are still **deferred** until the room
manager is implemented and tested; do not call them fixed. The manager
contract and mandatory race/collision tests are described in
`docs/ROOM_MANAGER_REQUIREMENTS.md`. No networking features were added.

This follow-up does not mean the 8-page report's former PR status is
current: the original report recorded commit `a451810`, whereas the
current branch has newer remediation commits. Review current GitHub
Actions and `VERIFICATION.md` for exact final-commit results.

## Status-audit follow-up validation

The trusted expiration mutator is now crate-private; integration tests
exercise expiry via the public capability-bound API, while two in-crate
unit tests verify the private maintenance transition and a compile-fail
Rustdoc example verifies external API invisibility. The meaningless
self-equality ID assertion has been replaced with a 16-room non-repetition
sanity check. No global registry uniqueness guarantee is claimed.

Commit `1e2a34bd30d5ab7871b4d524b05a055cf1d13a28` passed the
eight-command GitHub Actions gate:
https://github.com/fngvarlog/ghostroom/actions/runs/39507838360
Tests: two private maintenance, 30 legacy, 31 participation, and one
Rustdoc compile-fail check. This follow-up does not implement the room
manager or a network transport.
