# Step 5 acceptance checklist — do not implement until approved

**Status: NO-GO / BLOCKED — Step 5 production E2EE implementation is not approved (ADR 0002).** Steps 4, 4B and 4C add no E2EE code.

## Decision gate before writing E2EE application code

- [x] **Post-quantum confidentiality against recorded traffic is mandatory**
      for the first public release. Classical-only fallback is prohibited;
      unextended vodozemac/Olm is ineligible.
- [ ] Select a **complete, maintained, independently reviewed cryptographic
      messaging protocol implementation**, not assembled primitives or a custom
      handshake, ratchet or KEM combination. Establish actual supported initial
      PQ key agreement **and ongoing PQ ratchet / forward secrecy** properties.
- [ ] Identify a complete PQXDH+ongoing PQ ratchet profile with **supported,
      release-pinned Rust and Java/Kotlin/Android interfaces where required**;
      demonstrate usable public APIs, exact version/profile selection,
      client-visible verification and reliable anti-downgrade enforcement,
      not merely source-level internal implementation.
- [ ] Resolve third-party support and AGPL-3.0/legal distribution terms
      before using libsignal or SimpleX. Do not change MIT license silently.
- [ ] Show credible PQ *ongoing* forward secrecy and document PQ PCS
      separately from classical ratchet recovery.
- [ ] Adopt an explicit protocol ADR and exact implementation release,
      license obligations, supported API and proposed cipher suite,
      without inventing a ratchet, KEM combination or key schedule.
- [ ] Identify an accountable maintainer, supported version/upgrade policy
      and a documented **vulnerability-response and disclosure plan**;
      verify transitive dependencies, artifact hashes/provenance/signatures
      where available, release integrity, existing audits and advisories.
- [ ] Demonstrate both users' **independently authenticated peer identities**,
      room, onion destination, roles, keys and session/transcript binding,
      including deliberate server key substitution and split-session attacks;
      no unverified-mode fallback or invented key agreement.
- [ ] Verify Android + Rust integration across at least arm64-v8a and
      one additional ABI; choose UniFFI/JNI toolchain and check memory
      ownership, errors, crashes, secret-copying and concurrency.
- [ ] Freeze strict initial mutual QR/SAS authentication UX and a
      fallback-free failure policy, including identity changes.
- [ ] Test replay, duplication, explicit downgrade attempts before **and after
      session acknowledgement**, reconnection, crashes and session-key state
      recovery; set strict bounds on skipped keys, pending sessions,
      payload bytes and replay cache with no old key reuse.
- [ ] Run **official test vectors/samples and reproducible two-client
      cross-platform negative tests** using the approved version and ABI
      artifacts; record actual outcomes and verify post-quantum profile.
- [ ] Define Android Keystore versus strict memory-only storage,
      backups, lock screen, notifications, screenshots and process
      restart behavior before shipping.
- [ ] Obtain **qualified legal review** of GhostRoom's MIT license and the
      selected library's terms, native linking, redistribution obligations,
      attributions and planned Android/server release model.
- [ ] Record **explicit human security and licensing approval** in a new
      evidence-backed amendment changing ADR 0002 to ACCEPTED **before**
      any production encryption integration; all preceding boxes must pass.

## Crypto/session correctness matrix

| Test | Execution tier | Required evidence |
|---|---|---|
| Official known-answer vectors / upstream samples | Rust core | Verify library test vector outputs without fabricated expected values |
| Two new devices negotiate same valid session | Rust + Kotlin/Android | Matching authenticated identities, roles, room and transcript |
| Invalid prekey, tampering, malformed pubkey | Rust | Reject, no state advance or plaintext leakage |
| Attacker substitutes Alice's public key | Rust/Android integration | OOB commitment mismatch and no user plaintext |
| Attacker substitutes Bob's key | Rust/Android integration | Mutual SAS/QR verification fails; no auto-trust |
| Wrong room, onion context, peer role, transcript | Rust | Reject authenticated context mismatch |
| Cross-session reflection, simultaneous handshakes | Rust | No undetected parallel sessions or swapped roles |
| Replay, duplicate, edited ciphertext | Rust | Library + bounded app replay cache reject |
| Out-of-order within documented window | Rust | Correct plaintext once only |
| Out-of-order beyond window | Rust | Safe error, no unbounded key-cache growth |
| Compromise/ratchet-update recovery | Rust | Demonstrate only properties supported by chosen ratchet; check key disposal |
| Old/unknown protocol version | Rust/Kotlin | Fail closed; no downgrade |
| Network partition or delayed handshake | Rust/Android and later E2E | No silent identity/session reset |
| Process death/restart | Android | No reuse of discarded message keys; fail closed or validated state recovery |
| Room deadline, cancel, close versus message processing | Rust + later server integration | Cannot accept after manager closure; future queue atomicity tested only with real queue |
| Key removal and backups | Android | Verify storage scope; discuss physical erasure limitations |
| FFI invalid buffers and cancellation | Rust/Kotlin | No memory unsafety, leaks or sensitive logs |
| Unsupported ABI / provider failure | Android | Explicit error; never disable E2EE |
| Client/interoperability round trip | 2 Android endpoints or independent Rust/Kotlin | Matching wire-format version, sender/recipient state, replay rules |
| Load/performance | Real Android device | Report hardware, Rust/Android versions, memory, CPU and wire byte size |

## Test environments

1. Pure Rust library unit and property tests with upstream official
   test vectors; add stateful fuzzing on parsers after implementation.
2. Cross-language Kotlin/JNI or UniFFI compatibility tests with
   real Android ABIs and a deterministic test harness.
3. Two-client integration with malicious/fake relay, including key
   swaps, duplicate delivery, packet reorder and restarts.
4. Production-like Android tests of lock screen, backups, notifications
   and memory lifetime. Run sanitizer/static analysis when supported,
   and commission external review before security claims.

Never paste simulated ciphertext/test-vector bytes into the repository
as if they came from official upstream vectors. Never silently accept
partial verification or use server-issued capabilities to claim peer
crypto identity. The future Step 5 implementation may start only after
the ADR is accepted and these design choices are reviewed.

## Step 4B permitted and forbidden work

**Permitted:** non-shipping, isolated evaluation of official Signal
Android artifacts and source APIs; reproduce upstream public vectors,
quantify Android support and packaging, document a legally reviewed
integration arrangement, and commission cryptographic design review.

**Forbidden until ADR status changes to ACCEPTED with evidence:**
production E2EE code, PQ primitives or new ratchets, adding crypto
dependencies to GhostRoom's workspace, shipping any classical-only
fallback, implementing envelope version 1 as an assumed Olm format,
claiming the server or Android clients provide E2EE, and starting
Step 5 message encryption. A green Rust core CI run is not approval.

## Step 4C evidence-based gate status (2026-10-21)

Detailed report:
[Step 4C Signal feasibility](research/step-4c-signal-feasibility.md).
**Outcome B**: Signal is **not currently an approvable third-party
Android integration**; **ADR 0002 remains NO-GO**. The source-level
Java and native library interfaces are credible evaluation leads but
do not remove the licensing, support, Android and anti-downgrade gates.

| Gate | Observed evidence | Required before acceptance |
|---|---|---|
| Current upstream release and source provenance | Latest GitHub release observed `v0.105.2` on 2026-10-21; source commit `b369755533ef5a3fae9b31b989114f22e074dfab`, unsigned annotated tag | Record signed/verified provenance for **actual official Maven POM, AAR and JAR binaries**, resolved dependency versions and SHA-256 checksums |
| Java-facing protocol APIs | Tagged source exposes `SessionBuilder.process(PreKeyBundle)`, `SessionCipher.encrypt/decrypt`, `SessionRecord.getSessionVersion/hasSenderChain` | Exercise fully supported, version-pinned mobile API; show strict non-classical profile enforcement on two clients |
| Native PQ status check | JNI `hasSenderChain` checks `NotStale | EstablishedWithPqxdh | Spqr`, but upstream warns unacknowledged SPQR state may later downgrade | Deliberately test before/after acknowledgement, attempted downgrade, restart, concurrent initiation and fail-closed behavior |
| Peer authentication | `IdentityKeyStore` is caller-owned and no GhostRoom mutual QR/SAS ceremony exists | Independently reviewed binding of identities, onion service, room, roles and transcript, with two explicit confirmations |
| Ratchet/session persistence | Stores and native Triple Ratchet observed in source only | Android atomic storage/crash-recovery and replay tests without message key reuse |
| Android environment | Upstream source lists minSdk 23, compileSdk 34 and four ABIs; local Java 21 available but no Gradle, SDK/NDK, emulator, adb or downloadable artifacts | Executed `arm64-v8a` + second ABI build and real two-client tests, document exact Gradle/SDK/NDK/device versions |
| Upstream support and licensing | README explicitly says outside-Signal use unsupported; repository AGPL-3.0 | Documented permissible and maintainable integration path plus qualified legal review of app/native AGPL distribution within MIT project |
| Protocol crypto properties | Official PQXDH and Triple Ratchet SPQR specifications inspected; no candidate binary executed | Independently review ongoing PQ FS, PQ PCS assumptions, malicious relay controls, replay and drop-induced PCS delays |
| Runtime performance | **Not measured** | Actual device/time/memory/native size and ciphertext-byte measurements; no simulated numbers |

The Step 4C evaluation **stopped before writing throwaway crypto
code** because an officially supported third-party contract and
licensing decision were not established and the Maven/Android build
environment was unavailable. This is a recorded blocker, **not**
a failed test run or a claim the official Signal product lacks PQ
support. No GhostRoom production dependency or Rust source changed.
The next permitted task is support/licensing clearance and an
isolated, official-API compatibility harness; no Step 5 production
encryption implementation until an evidence-backed **ACCEPTED**
amendment to ADR 0002.
