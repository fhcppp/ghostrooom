# GhostRoom Step 4 — E2EE research and architectural decision report

**Date:** 2026-10-21. **Current Step 4B decision: NO-GO / BLOCKED**
under mandatory post-quantum first-release confidentiality. Earlier
Step 4 architecture research is retained as historical background. No E2EE implementation, transport, Kotlin code or new Cargo
dependency was introduced.

## Verified starting state

Source branch: `step3-room-manager-2026-10-20`.
Source/PR #2 head: `83fa0a691bd7ac975fd1d7dfafd866b69e2c2871`.
PR #2: https://github.com/fngvarlog/ghostroom/pull/2
(PR #1 https://github.com/fngvarlog/ghostroom/pull/1 remains draft).
Base Step 3 CI: https://github.com/fngvarlog/ghostroom/actions/runs/39515539280
passed all eight checks on exactly that commit using Rust 1.99.1;
61 library tests (28 manager, 31 participation, 2 maintenance),
30 Step 1 tests, and one Rustdoc compile-fail test passed in four
test configurations. The assistant's execution environment did not
contain Cargo/rustc, so the evidence is CI only.

## Historical Step 4 research and provisional choice (superseded)

See [source-based candidate review](docs/research/step-4-e2ee-candidate-review.md).
Signal libsignal has two-party PQXDH/SPQR designs and Android JNI but
unsupported external use and AGPL-3.0 constraints. SimpleX Chat/SimpleXMQ
has a strongly privacy-oriented model and optional SNTRUP761-based PQDR
but is AGPL-3.0 and requires adapting a distinct application/agent
stack. vodozemac Olm is an Apache-2.0, Rust-native classical
double-ratchet API with SAS support and dated historic audit evidence;
it has **no verified post-quantum KEM**. OpenMLS is MIT-licensed,
standards-based group E2EE with more state than a fixed two-person
room and additional authentication-policy work.

**Historical proposal before the mandatory-PQ decision, now REJECTED:** vodozemac Olm for an initial release
*only if classical-only security is explicitly accepted* and the
exact library, authentication binding, replay limits and Android
integration are validated. If PQ confidentiality is mandatory,
**do not select this profile**. At that time the ADR was unresolved;
subsequent Step 4B/4C evaluation made its current status **NO-GO /
BLOCKED**, not ACCEPTED. Adding arbitrary ML-KEM to Olm is forbidden.

## Core authentication and threat decisions

Server room credentials authorize routing and room operations, not
cryptographic identity. Assume the server can substitute keys,
split/parallel sessions, drop/replay ciphertext and observe metadata.
Require fresh client-held identities for each room and a trusted
out-of-band QR/commitment to Alice's identity. Bob's identity must
also be mutually verified, with transcript-bound SAS/QR confirmation
from both people. Include room ID, onion target, both roles/participants,
identity/prekeys, selected version and session commitment in a canonical
authenticated transcript. No silently trusted server-supplied keys;
skipped/mismatched verification blocks user plaintext.

If an authenticated independent OOB channel exists, server key
substitution is detectable under correct protocol implementation.
With only a server-provided invite and untrusted communications,
identity spoofing is possible. Server may still deny service and
learn timing even with fully verified E2EE. Neither room expiry nor
zeroization erases screenshots or copies already held by recipients.

## Message envelope proposal

[Versioned envelope](docs/E2EE_MESSAGE_ENVELOPE.md) separates
untrusted routing fields (`protocol_version`, `room_id`, routing slot,
session reference, bounded opaque library ciphertext) from strictly
authenticated *inside-encryption* application context (room, identities,
session/transcript, sender role, per-message replay identifier,
application payload). Do **not** create duplicate ratchet nonces,
counters, AEAD tags, MACs or custom cipher metadata; the selected
library controls those. The proposal makes no unsupported claim of
external AEAD associated-data support by Olm.

## Client and session lifecycle

[Detailed model](docs/E2EE_ARCHITECTURE.md) covers onboarding,
two-way peer identity verification, first prekey/normal Olm message,
bounded skipped keys and replay handling, authenticated transcript,
ratchet evolution, closure/expiry, reconnect after state loss, and
best-effort local secret disposal. Proposed shared Rust client core
plus Kotlin binding is not validated or implemented. Android
process-death, Keystore, backup and FFI semantics remain open.
Potential plaintext persistence and screenshot/notification copies
are outside any guaranteed deletion.

## Step 5 entry and test plan

The [Step 5 checklist](docs/E2EE_STEP5_TEST_PLAN.md) includes official
vectors, peer key substitution, wrong-room binding, malformed and
unsupported versions, replay/duplicates/out-of-order, damaged
ciphertext, ratchet recovery, session reset, server MITM/parallel
sessions, cross-platform Android interoperability, key deletion,
memory and wire-overhead measurement, and licensing/advisory review.
No official vectors, benchmarks or Android binaries were run here.

## Files and current limits

Research and decision documentation:
- `docs/research/step-4-e2ee-candidate-review.md`
- `docs/adr/0002-ephemeral-two-party-e2ee.md`
- `docs/adr/0001-future-e2ee.md` (linked predecessor)
- `docs/E2EE_ARCHITECTURE.md`
- `docs/E2EE_MESSAGE_ENVELOPE.md`
- `docs/E2EE_STEP5_TEST_PLAN.md`

Status reconciliation:
- `README.md`, `docs/SECURITY_DESIGN.md`
- `docs/ROOM_MANAGER_REQUIREMENTS.md`, `STEP3_REPORT.md`
- `docs/audits/status_audit_2026-10-20_disposition.md`
- `VERIFICATION.md`, `.github/workflows/rust.yml`
- `STEP4_REPORT.md` (this report)

The existing Step 3 code and Cargo dependencies remain unchanged.
The existing Rust CI gate should be rerun on the Step 4 documentation
commit and its exact head checked before PR review. No security
certification, Android interoperability, end-to-end encryption, server,
queue or Tor transport is claimed.

## Historical Step 4 questions (PQ requirement subsequently decided)

1. **ANSWERED in Step 4B:** post-quantum recorded-traffic confidentiality
   **IS MANDATORY** at first release. Classical-only Olm must not ship.
2. What authentic out-of-band verification experience will both
   users actually have, and is unverified messaging always forbidden?
3. Can vodozemac's released Olm API safely implement the proposed
   transcript/room binding without protocol invention?
4. Is Apache-2.0 licensing acceptable and are all transitive,
   native/Android bindings and notices reviewed?
5. What client state storage, backup prohibition and compromised
   device recovery policy will the product support?
6. Who reviews and approves the final crypto threat model and
   Step 5 interoperability evidence?

Until those decisions are recorded and the ADR is accepted, Step 5
encryption implementation remains blocked.

## Verified research-commit CI

Step 4 documentation and ADR commit
`e5bf37e6e27bbbc56c44b0b2b83d1c5be880637a`
passed all eight existing Rust gates in
[GitHub Actions run 39547508837](https://github.com/fngvarlog/ghostroom/actions/runs/39547508837).
This confirms only that Rust regressions were not introduced by the
documentation changes. The E2EE design itself has **not been
cryptographically implemented, interoperably tested, or independently
reviewed**. The follow-up verification-report commit requires its
own matching successful CI result.

## Step 4B final decision (2026-10-21)

**NO-GO**. Post-quantum confidentiality against recorded traffic is a
mandatory first-release requirement. Classical-only Olm/vodozemac is
rejected for production. Signal PQXDH + Triple Ratchet is a leading
**non-shipping compatibility evaluation** target but is not approved:
third-party use of libsignal is unsupported, the release-specific
Android API and profile downgrade enforcement are not verified, and
AGPL-3.0 distribution and future maintenance implications for this
MIT project remain unresolved. SimpleX has optional PQDR but no verified
standalone supported Rust/Kotlin integration; OpenMLS' enumerated
ciphersuites are classical. No custom ML-KEM assembly is allowed.

[ADR 0002](docs/adr/0002-ephemeral-two-party-e2ee.md)
records the binding NO-GO/blocked decision, exact upstream evidence,
malicious-server authentication obligations, and seven objective exit
criteria. The [Step 5 checklist](docs/E2EE_STEP5_TEST_PLAN.md)
blocks production encryption until an ACCEPTED, evidence-backed
amendment. No crypto library was added or app protocol implemented.

## Step 4C reconciliation — current status (2026-10-21)

**Outcome B — integration not approved; production E2EE remains NO-GO /
BLOCKED.** The Step 4C source/API review of Signal libsignal v0.105.2
established the existence of documented Java/Kotlin session operations
and a native PQXDH/SPQR sender-chain check. It did **not** establish a
supported third-party Android contract, enforceable downgrade rejection
through peer acknowledgement, binary artifact provenance, Android runtime
or official-vector test results, qualified AGPL/MIT distribution review,
or secure GhostRoom peer/session storage. No cryptographic prototype was
built or run; no cryptographic test failed.

[Step 4C evidence](docs/research/step-4c-signal-feasibility.md) and
[ADR 0002](docs/adr/0002-ephemeral-two-party-e2ee.md) supersede earlier
provisional choices and open PQ-product questions in this historical
report. [Step 5 acceptance gates](docs/E2EE_STEP5_TEST_PLAN.md) remain
unmet. No encryption source or runtime dependency was added.
