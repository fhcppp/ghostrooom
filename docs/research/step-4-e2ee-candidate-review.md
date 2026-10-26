# Step 4 — E2EE candidate and dependency research

Research date: **2026-10-21** (UTC). Documentation and official upstream
source were inspected, but no upstream libraries were built, benchmarked,
or independently security-audited for GhostRoom. Repository activity
dates represent GitHub metadata observed on this date, not evidence of
cryptographic correctness or maintenance guarantees.

## Sources inspected

1. [SimpleX Chat source, privacy and licensing](https://github.com/simplex-chat/simplex-chat),
   [security policy](https://github.com/simplex-chat/simplex-chat/blob/stable/docs/SECURITY.md):
   client/relay separation, no global user IDs, AGPL-3.0; the policy
   references Trail of Bits implementation work in 2022 and protocol
   review in 2024. Assessments of SimpleX are **not** assessments of GhostRoom.
2. [SimpleXMQ security](https://github.com/simplex-chat/simplexmq/blob/stable/protocol/security.md),
   [agent protocol](https://github.com/simplex-chat/simplexmq/blob/stable/protocol/agent-protocol.md),
   [PQDR design](https://github.com/simplex-chat/simplexmq/blob/stable/protocol/pqdr.md):
   Curve448/X3DH-like initialization, augmented Double Ratchet, optional
   **SNTRUP761** KEM, AES-GCM, HKDF-SHA512, ratchet negotiation and agent
   versioning. The PQDR doc specifies message overhead in kilobytes for
   KEM material. SNTRUP761 is **not** NIST ML-KEM; its existence doesn't
   establish PQ authentication or end-to-end quantum security in all phases.
   Codebase and protocol are not a plug-and-play Rust client library.
3. [Signal libsignal](https://github.com/signalapp/libsignal) and its
   [README](https://github.com/signalapp/libsignal/blob/main/README.md):
   maintained Rust implementation with Java/Android, Swift and TypeScript
   products; **use outside Signal is explicitly unsupported**, and APIs and
   bridge layers may change. Its code is **AGPL-3.0**. Java/Android JNI
   build instructions list JDK 21, Rust Android targets, NDK/SDK, and native
   toolchain dependencies. Official [PQXDH specification](https://signal.org/docs/specifications/pqxdh/)
   explicitly says quantum-resistant authentication is not provided and
   users must independently authenticate identity keys. Signal's 2025
   [SPQR announcement](https://signal.org/blog/spqr/) describes a
   post-quantum ratchet improving FS/PCS. Whether libsignal's current
   particular public APIs support the desired SPQR profile, with stable
   interop outside Signal, remains to be validated.
4. [vodozemac source and README](https://github.com/matrix-org/vodozemac),
   [Olm module source](https://github.com/matrix-org/vodozemac/blob/main/src/olm/mod.rs),
   [SAS source](https://github.com/matrix-org/vodozemac/blob/main/src/sas.rs),
   [Session documentation](https://docs.rs/vodozemac/latest/vodozemac/olm/struct.Session.html):
   maintained **Apache-2.0** pure-Rust implementation of Olm/3DH +
   two-person Double Ratchet, with `olm::Account`,
   `Account::create_outbound_session`,
   `Account::create_inbound_session`,
   `olm::Session`, `olm::SessionConfig`, `OlmMessage`,
   and `sas::Sas`/MAC mechanisms. README presently shows dependency
   `vodozemac = "0.11.1"` — **research reference, not a dependency lock**.
   README references one Least Authority security review,
   [published May 2022](https://leastauthority.com/blog/audit-of-matrix-vodozemac/).
   That historic scope cannot validate all current code or GhostRoom's
   authentication, mobile ABI, or future implementation. **No PQ KEM
   functionality was verified in the Olm API**.
5. [OpenMLS project](https://github.com/openmls/openmls),
   [readme and supported targets](https://github.com/openmls/openmls/blob/main/README.md),
   [license](https://github.com/openmls/openmls/blob/main/LICENSE):
   **MIT** Rust implementation of
   [MLS RFC 9420](https://www.rfc-editor.org/rfc/rfc9420).
   It implements multi-participant authenticated group key agreement.
   Linux is CI-tested; Android Rust target is built but **not CI-tested**
   according to upstream README. Ciphersuites listed include X25519,
   AES-GCM and Ed25519; no general PQ security is evidenced by this list.
   The standard relies on an external authentication service (or a
   well-defined replacement trust model). A two-person room gains
   complexity from epochs, key packages, membership commits and group
   state without a need for group scalability.
6. [NIST FIPS 203](https://csrc.nist.gov/pubs/fips/203/final):
   final ML-KEM standard, published 2024-08-13. The page displays a
   2025-11-17 planning note describing pending corrections. ML-KEM is
   **a KEM**, not a messaging protocol, ratchet or peer authentication
   mechanism; combining it with Olm would invent protocol design and
   **is not authorized**.
7. [RustCrypto zeroize source](https://github.com/RustCrypto/utils/blob/master/zeroize/src/lib.rs):
   volatile-zeroization efforts do not erase previous stack copies,
   spilled registers, reallocated buffers, logs or external backups.
8. [UniFFI project](https://github.com/mozilla/uniffi-rs) and
   [changelog](https://github.com/mozilla/uniffi-rs/blob/main/CHANGELOG.md):
   supports Rust-to-Kotlin mobile interfaces. A June 2026 v0.31.2
   changelog fixes ARM32/JNA handling; an **experimental** Kotlin JNI
   generator is listed as unreleased. No GhostRoom mobile FFI build
   or performance data was collected.

All links refer to upstream documentation or sources; upstream security
claims must be reviewed within their dated audit and protocol scope.

## Comparison matrix

| Property | libsignal | SimpleX/PQDR | vodozemac Olm | MLS/OpenMLS |
|---|---|---|---|---|
| E2EE confidentiality | Ratcheted two-party messaging | Ratcheted connection messaging | Ratcheted two-party Olm sessions | MLS group key/AEAD |
| Authentication | Identity keys + required external key verification | Trusted OOB invite/link, key exchange; app policies matter | Ed25519 account identity plus **app-enforced** trust/SAS or QR verification | Credential validation requires application identity policy/AS |
| Forward secrecy | Classical Double Ratchet; PQXDH initial stage, SPQR where enabled | DH and optionally KEM-augmented PQDR | Classical 3DH/Double Ratchet | Epoch/message FS with correct MLS operation |
| Post-compromise recovery | DH ratchet; SPQR when actually negotiated and integrated | Classical ratchet; PQ KEM ratchet in supported negotiated modes | Classical double-ratchet recovery **after an honest fresh DH turn** | MLS updates/commits, subject to group scheduling |
| Post-quantum | PQXDH and SPQR designs exist; actual external API/profile must be confirmed | Optional SNTRUP761 PQDR; **not ML-KEM** | **No verified PQ KEM** | No verified PQ ciphersuite in inspected README |
| Replay/out-of-order | Ratchet replay/skipped-key rules; bounds require checks | Agent ratchet replay/skipped-key handling; bounds require checks | Olm message/session state; app requires explicit duplicate/retention policy | Epoch/generation semantics; delivery reordering must be managed |
| Session binding | Application must pin identity/session/room transcript | Connection invite/queue state and agent version | **Must add authenticated room/role/session binding in app's protected contents** | Group ID/context/credential binding; auth service required |
| Local key disposal | Client/app must handle lifetime and backup | Client/app must handle lifetime and backup | Client/app must handle lifetime and backup | Client/app must handle lifetime and backup |
| Android | Java/JNI artifacts supported, JDK/NDK requirements | Existing Android app but reusing stack is nontrivial | Rust Android cross-build/UniFFI **not verified here** | Android target built upstream, not tested upstream |
| Rust API | Large Rust implementation; external use unsupported | Primarily Haskell agent/protocol stack | Concrete `olm::Account`/`olm::Session` API; preliminary candidate | Rust `MlsGroup` architecture and crypto/storage providers |
| License | **AGPL-3.0** | **AGPL-3.0** | **Apache-2.0** | **MIT** |
| Audit evidence | Signal specs/security program; no GhostRoom-specific review | SimpleX 2022/2024 reviews, dated/scope-limited | Least Authority 2022 review, dated/scope-limited | No specific independent audit result confirmed by this search |
| Two-person fit | Excellent ratchet design, licensing/support blocker | Excellent privacy model, full-stack/AGPL blocker | **Strongest simple Rust candidate** if classical security acceptable | More group complexity than initial requirements |

**Qualifications:** FS and PCS require correct initialization, honest
fresh key exchange and key erasure, not just use of a named library.
A strong PQ claim is not justified by using KEM for only one stage.
Out-of-order support is not unlimited: each implementation and app must
set bounded skipped-key/session-memory and replay behavior. An attacker
with an unlocked compromised endpoint can read plaintext regardless of
the ratchet. No comparative benchmarking was performed; PQ headers and
MLS state imply potential overhead, but actual mobile costs are unknown.

## Client placement analysis

**Kotlin with libsignal's Java artifacts.** Good Android-native user
integration and maintained JNI implementation, but external use is
unsupported and AGPL-3.0 plus multi-platform native packaging adds
significant maintenance/redistribution questions. The protocol choice
also has to be pinned to an actual interoperable, supported profile.

**Shared Rust client core + Kotlin bindings.** Strong reuse of one
crypto/session state machine across future clients and Rust-hosted tests.
Best candidate uses vodozemac's public Olm API, with a narrow private
wrapper exposing only create/verify/send/receive/close operations.
UniFFI Kotlin bindings are a candidate; no FFI generator version or
Android ABI is selected until tests validate copies, secret lifetime,
exceptions and platform support. Avoid exposing raw private keys,
`Account`, `Session` or decrypted buffers in logs or long-lived
Kotlin objects. Native library signing/build reproducibility and
Android Keystore policy still require design and validation.

**MLS/OpenMLS.** An extensible group design with credential providers and
group operations. No group requirement exists, so the additional state,
commits and application AS policy make it a less simple initial choice.

## Historical Step 4 decision and outstanding blockers (superseded)

**Historical Step 4 status (superseded): UNRESOLVED**. At that time vodozemac was the **preferred conditional
prototype** for a **classical** two-person session, with shared Rust
client code plus *evaluated* Kotlin bindings. This is not an accepted
production or deployment protocol choice. A requirement for PQ resistance
against recorded messages would disqualify the unextended Olm profile.
Adding ML-KEM or SimpleX's KEM to it would be unsupported custom crypto.

Historical pending action, since resolved: determine whether PQ is mandatory
for initial release (**now mandatory; Olm rejected**); examine current vodozemac API/test vectors and replay
bounds; demonstrate two-client authenticated QR/SAS flow; decide how
room/session role binding is enforced with Olm; validate Android build
and JNI/UniFFI error and secret semantics; review full dependency tree,
licenses, advisories, test vectors and mobile CPU/memory performance.
If the PQ requirement is mandatory, revisit libsignal's supported
implementation path and/or a reviewed hybrid protocol; do not silently
ship classical-only E2EE under a PQ label.

## Limitations of this research

The searches inspected README/spec/source and repository metadata, not
all transitive code, historic vulnerability databases, signed release
artifacts, app package builds or mobile performance. Direct repository
activity on 2026-10-20 was observed for the main upstream projects, but
does not establish security review, compatibility or adequate staffing.
Licensing implications for distributing GhostRoom are unresolved legal
review items, not legal advice.

## Step 4B reassessment — mandatory PQ first-release policy (2026-10-21)

Product direction is now **mandatory post-quantum confidentiality for
the first public release**. This overturns the preceding conditional
classical-only Olm preference; **ADR 0002 is NO-GO / BLOCKED**.

Additional current official-source checks:

- [libsignal Rust Triple Ratchet](https://github.com/signalapp/libsignal/blob/main/rust/protocol/src/triple_ratchet.rs) explicitly describes a combined
  Double Ratchet + SPQR send/receive state, with identity checking,
  pre-key selection and session storage outside that module. The
  [PQXDH source](https://github.com/signalapp/libsignal/blob/main/rust/protocol/src/pqxdh.rs) implements initial key agreement.
  The official [README](https://github.com/signalapp/libsignal/blob/main/README.md)
  states **third-party use is unsupported** and documents Android JNI
  artifacts. Latest observed [v0.105.2 release](https://github.com/signalapp/libsignal/releases/tag/v0.105.2), 2026-10-20.
  The presence of internal code and artifacts does **not** verify a
  supported, stable, application-selectable profile across Android
  versions. The repository is AGPL-3.0.
- [SimpleXMQ security specification](https://github.com/simplex-chat/simplexmq/blob/stable/protocol/security.md)
  documents optional SNTRUP761 running alongside DH ratchet; the
  [PQDR design](https://github.com/simplex-chat/simplexmq/blob/stable/protocol/pqdr.md)
  is not a supported standalone Rust SDK. The complete agent and
  compatibility negotiation require review and AGPL-3.0 obligations.
- [vodozemac Olm](https://github.com/matrix-org/vodozemac/blob/main/src/olm/mod.rs)
  exposes classical 3DH/Double Ratchet. Its Apache-2.0 license and
  SAS facilities do not create post-quantum protection.
- [OpenMLS supported ciphersuites](https://github.com/openmls/openmls/blob/main/README.md)
  currently enumerate classical X25519/P-256 suites and call Android
  targets **built but not tested**; its observed release was
  [openmls-v0.9.0](https://github.com/openmls/openmls/releases/tag/openmls-v0.9.0)
  published 2026-08-25. MLS group semantics are not proof of PQ.
- [NIST FIPS 203](https://csrc.nist.gov/pubs/fips/203/final)
  specifies ML-KEM as a KEM, not a complete messaging protocol.
  [UniFFI Kotlin support](https://github.com/mozilla/uniffi-rs)
  does not establish an E2EE protocol or Android interop correctness.

**Engineering result:** NO-GO, not a replacement research suggestion.
Signal is the nearest **disposable compatibility evaluation target**;
there is no accepted integrated library/version or shipping E2EE
profile. Evaluate initial PQ key establishment, continuing PQ forward
secrecy, PQ PCS separately, and classical signature/peer-authentication
limitations. Licensing must be resolved before integration. No upstream
crypto builds, official test vectors, device tests, transitive advisory
scanner, benchmarks or independent audits were run during Step 4B.

## Step 4C disposition (2026-10-21)

**Current ADR status: NO-GO / BLOCKED.** Mandatory first-release PQ
confidentiality is settled; the classical-only Olm evaluation is not a
production option. [The Step 4C libsignal feasibility review](step-4c-signal-feasibility.md)
found relevant Java APIs and native PQXDH/SPQR checks **in source**, but
no executed Android prototype, Maven artifact hashes, proven post-ack
anti-downgrade behavior or supported and legally reviewed third-party
integration. **Outcome B** rejects approval for GhostRoom, not Signal's
cryptographic protocol. Step 5 remains blocked until ADR 0002 is
explicitly approved with complete evidence.
