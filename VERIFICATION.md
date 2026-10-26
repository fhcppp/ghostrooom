# Verification on 2026-10-20

The execution environment did not contain `cargo` or `rustc` on `PATH`.
A Debian package installation attempt timed out when updating repositories;
a direct connectivity check for `static.rust-lang.org` failed with DNS
resolution failure. The verification commands were still attempted to collect
actual, non-fabricated results.

## Actual commands attempted and results

```text
$ cargo fmt --check
sh: 1: cargo: not found
EXIT_STATUS=127

$ cargo clippy --all-targets --all-features -- -D warnings
sh: 1: cargo: not found
EXIT_STATUS=127

$ cargo test
sh: 1: cargo: not found
EXIT_STATUS=127

$ cargo test --release
sh: 1: cargo: not found
EXIT_STATUS=127
```

**No build, formatting, lint, test or release-test pass is claimed.** The
source should be checked with the full gate in a stable-Rust Debian
environment before proceeding to Step 2. A copy of the original four-command
output is in `check_output.txt`. The **original** verification helper ran four
checks; its updated version runs all six and exits nonzero on failure.


## Follow-up audit on 2026-10-20 (six-command gate)

The GitHub `main` source at `702ad60423d6f53fad0ad4111149f40ae1902ad3`
was inspected. The prior four-command failure record above is retained as
historical evidence. The newer acceptance gate requires **six** commands.

Environment checks:

```text
$ rustc --version
bash: rustc: command not found
$ cargo --version
bash: cargo: command not found
$ rustup --version
bash: rustup: command not found
$ git --version
git version 2.47.3
$ git ls-remote https://github.com/fngvarlog/ghostroom.git HEAD
fatal: unable to access 'https://github.com/fngvarlog/ghostroom.git/': Could not resolve host: github.com
```

A subsequent `apt-get update` attempt failed to fetch Debian repository
metadata because `deb.debian.org` DNS lookup failed. The package manager
returned process status 0 despite these warnings; package/toolchain
installation did **not** succeed.

The updated verification helper was run locally and exits nonzero when any
individual check fails. All six checks remain **unexecuted**, including
`cargo fmt --check`, `cargo check --all-targets --all-features`,
`cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`,
`cargo test --release`, and `cargo doc --no-deps`. They could not run without
Cargo. The written suite now has 30 tests, not an observed 30-test pass.

**Gate result: FAIL (environment blocker). Step 2 was not started.**


## CI verification update — Step 1 and Step 2, 2026-10-20

The earlier failed, unavailable local toolchain attempts above remain
historically accurate. GitHub Actions supplied a working Linux stable Rust
toolchain (rustc 1.99.1) and independently executed the six gates:

- Step 1 commit `a47fe4e98cae13373435ae2f859fb00ca6923948`:
  all six checks passed. Run
  https://github.com/fngvarlog/ghostroom/actions/runs/39494030406
- Step 2 commit `31fa9b9c3e7c58207c5d13ad066225de1e09696c`:
  `cargo fmt --check` (on formatted CI workspace),
  `cargo check --all-targets --all-features`,
  strict `cargo clippy`, `cargo test`, `cargo test --release`,
  and `cargo doc --no-deps` passed. Debug and release suites contain
  30 Step 1 and 22 Step 2 tests. Run
  https://github.com/fngvarlog/ghostroom/actions/runs/39495303162
- CI format output was applied to committed sources afterward; the exact
  final commit and its strict-format verification must be checked separately.

No security audit, secret-lifecycle memory forensics, fuzzing,
Android compatibility test, or benchmark is claimed.

## Final strict-source CI gate

Run https://github.com/fngvarlog/ghostroom/actions/runs/39495597918
on commit `38411b1b776bcfbc05e67bdec3ec50ad407f4b05` completed
successfully: formatting, check, Clippy with `-D warnings`, debug
tests, release tests, and documentation all passed. Tests: 30 original
plus 22 Step 2, all passed in both configurations. This run checks
the actual committed formatting (not a CI-reformatted workspace).

## Repository audit hardening — 2026-10-20

The prior successful runs above apply to prior commit hashes and must not
be represented as validation of the remediation code. The remediation gate
adds `cargo test --locked` and `cargo test --locked --release`, besides the
six previously executed checks. The exact results on the remediation commit
are recorded in GitHub Actions (link added after it runs).

Local execution environment lacks Cargo and direct DNS resolution for
GitHub. The GitHub-hosted runner provides the compiler and test environment;
record results from its actual run, not inferred results. No independent
security, Android, concurrency, or dependency-vulnerability audit is claimed.

## 2026-10-20 audit remediation — verified build gate

Commit: `8abe20f105dd1ba892a0a22f986b82566a3564a2`.

Run: https://github.com/fngvarlog/ghostroom/actions/runs/39505118813

Rust: `rustc 1.99.1`. Observed status: **PASS** for all eight:
`cargo fmt --check`, `cargo check --all-targets --all-features`,
`cargo clippy --all-targets --all-features -- -D warnings`,
`cargo test`, `cargo test --release`, `cargo test --locked`,
`cargo test --locked --release`, `cargo doc --no-deps`.

Each of four test configurations passed 30 lifecycle tests plus
30 participation tests, no failures. This is the **committed source**
verification, not merely a workspace reformatted before the checks.
Dependency advisory scanner and independent security testing were not
run. This later successful run supersedes the pending-verification
statements in earlier sections without altering their historical results.

## Follow-up status audit checks (20 October 2026)

The eight-command verification gate is unchanged. Additional source
and test checks cover trusted-polling API visibility (one Rustdoc
compile-fail guardrail), internal maintenance semantics (two unit tests)
and sampled room-ID uniqueness (one integration test). Some earlier
integration tests were rewritten to use the public credential-bound
methods instead of the now-private maintenance function. Original test
history is retained above. **CI results for these changes are added
only after the final committed revision has actually passed.**

## 2026-10-20 status-audit follow-up verification

Source commit `1e2a34bd30d5ab7871b4d524b05a055cf1d13a28`:
https://github.com/fngvarlog/ghostroom/actions/runs/39507838360

GitHub Actions on stable Rust completed successfully for all eight
mandatory commands: formatting, check, strict Clippy, regular debug/release
tests, locked debug/release tests, and Rust documentation. Each test run
reported two internal maintenance tests, 30 legacy lifecycle tests, 31
participation integration tests, and one compile-fail Rustdoc test.
The substantive regression checks passed in all four test configurations.
This is CI evidence for the committed source, not an independent penetration
test or concurrent room-manager test. Advisory scanning and Android target
testing remain unperformed.

## Step 3 baseline and pending validation — 20 October 2026

Verified starting branch `step1-audit-fixes-2026-10-20` at
`09c09289d609c42a70f8ed36175fdbe86b75dfc3`, still draft PR #1;
`main` remains `702ad60423d6f53fad0ad4111149f40ae1902ad3`.
GitHub Actions run https://github.com/fngvarlog/ghostroom/actions/runs/39507970139
completed all eight original checks successfully on that exact commit.

This local container had no rustc, cargo or rustup. GitHub Actions serves
as the checked compiler/CI environment, not as evidence of local compilation.
Step 3 introduces internal deterministic manager tests and moves the Step 2
participation tests to in-crate test scope after mutating domain APIs became
crate-private. Final HEAD tests must be checked independently; no Step 3 pass
is claimed here before observing the new CI run.

## Step 3 final code verification — 2026-10-20

Branch `step3-room-manager-2026-10-20`, committed source:
`8499dbb53a962e66c979ea726fc48a98462975df`.
Workflow: https://github.com/fngvarlog/ghostroom/actions/runs/39514957147

Observed stable Rust version: rustc 1.99.1. **All eight gates passed:**

- `cargo fmt --check` — PASS
- `cargo check --all-targets --all-features` — PASS
- `cargo clippy --all-targets --all-features -- -D warnings` — PASS
- `cargo test` — PASS
- `cargo test --release` — PASS
- `cargo test --locked` — PASS
- `cargo test --locked --release` — PASS
- `cargo doc --no-deps` — PASS

Each of the four test configurations reported 59 library tests (31 Step 2
participation regressions, 26 manager tests, and 2 domain maintenance unit
tests), 30 legacy lifecycle integration tests, and 1 compile-fail Rustdoc
test, all passing with zero failures. The report describes outcomes for
the named code commit, not earlier failing intermediate revisions.
All checks were executed by GitHub Actions, **not** by local Cargo in the
assistant container; the local container lacked rustc and Cargo.

This result does not validate concurrent message queue operations,
network-facing normalization, OS-level side channels, performance,
Android compatibility, independent code review, or dependency advisories.
The documentation-only final commit triggers its own CI workflow and
requires an independently observed successful result before merging.

## Additional concurrency-race tests

The follow-up source adds tests for cleanup racing an in-flight authorized
callback and simultaneous room creation with deliberately colliding IDs.
The earlier 59-library-test result remains evidence for its named prior
commit only; this later source must independently pass the full gate.

## Latest expanded Step 3 CI run: 61 + 30 + 1 passing tests

Code commit `707f83db32d4f74e380ec942086aa8f36f0fbd00`
was verified at https://github.com/fngvarlog/ghostroom/actions/runs/39515394180.
All eight commands passed on rustc 1.99.1. Each test configuration
reports 61 library tests (28 manager, 31 retained participation,
2 maintenance), 30 Step 1 lifecycle integration tests, and one
compile-fail Rustdoc test, with zero failures.

The two additional manager race tests check (1) cleanup versus a
callback already inside the room lock, and (2) simultaneous creators
with deterministically injected collisions. These are synchronous
unit/concurrency tests, not network transport or message queue tests.
The previous 59-library-test CI result remains valid for the earlier
commit listed above, not for this expanded test suite.

All test evidence comes from actual GitHub Actions execution in the
hosted runner, not from the assistant's local Rust toolchain.

## Step 4 research baseline reconciliation — 2026-10-21

The actual Step 3 branch HEAD and draft PR #2 head were both
`83fa0a691bd7ac975fd1d7dfafd866b69e2c2871`.
Draft PR #1 remained open at Step 2 head
`09c09289d609c42a70f8ed36175fdbe86b75dfc3`;
`main` remained `702ad60423d6f53fad0ad4111149f40ae1902ad3`.

**Matching successful latest HEAD workflow**:
https://github.com/fngvarlog/ghostroom/actions/runs/39515539280
(alongside matching successful run 39515534285). Eight checks
(fmt, check all targets/features, Clippy warnings denied, debug/release
tests, locked debug/release tests, rustdoc) all **passed** on
Rust 1.99.1. Each test configuration: 61 library tests
(28 manager, 31 participation, 2 maintenance), 30 lifecycle integration
tests, and one compile-fail Rustdoc guardrail.

Historical older failed or pending entries above refer to their own
commits and remain true for those commits; they do **not** describe
the current baseline. Step 4 edits are research/docs-only; no
cryptographic dependency, crypto code, network transport or Android
code is added. A new Step 4 HEAD still needs its own matching CI
workflow to verify that documentation changes did not disturb the
existing Rust gate.

Local toolchain inspection on 2026-10-21 found no `cargo` or `rustc`;
therefore only GitHub Actions can provide executed test evidence here.
No vodozemac or libsignal build/test, Android ABI test, licensing
audit, security assessment, or benchmark was run.

## Step 4 documentation-only commit verification (2026-10-21)

Step 4 source/ADR documentation commit
`e5bf37e6e27bbbc56c44b0b2b83d1c5be880637a`:
https://github.com/fngvarlog/ghostroom/actions/runs/39547508837

The GitHub-hosted stable Rust workflow completed **all eight existing
quality gates successfully** (format, all-target check, strict Clippy,
debug test, release test, locked debug and release tests, and rustdoc).
This verifies that the existing Rust project still passes after the
documentation update. There were no cryptographic implementation tests
because no cryptographic application code was added in Step 4.
A later report-only commit must still obtain its own green workflow
before claiming the latest HEAD is verified.
