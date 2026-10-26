# Step 4C — Signal Android PQ E2EE feasibility evaluation

**Research executed:** 2026-10-21 (UTC).
**GhostRoom branch:** `step4-e2ee-architecture-2026-10-21`.
**Reviewed starting commit:** `cebbe92e1cb0987046166c337dc3d6a0e17e4737`.
**GitHub PR ancestry verified:** this Step 4C report and its checklist were
committed as `a4e6187ec642e88f40267fe21101cefa580f67fd`, a direct child
of `cebbe92`; both commits were already in draft PR #3 before the final
reconciliation. No duplicate cherry-pick, rebase or history rewrite was needed.
**PR:** [draft #3](https://github.com/fngvarlog/ghostroom/pull/3), based on Step 3.
**Decision:** **Outcome B — NOT A CURRENTLY ACCEPTABLE GHOSTROOM INTEGRATION.**
This rejects *approval of third-party deployment*, **not the security of
Signal's own messaging product or its research protocol**. No cryptographic
prototype was built or executed, so **no cryptographic test failed**.
API presence was verified from tagged source; Maven binaries, profile
negotiation behavior, Android device/ABI operation and distribution
permission were not verified.
**ADR 0002 remains NO-GO / BLOCKED.** Step 5 production encryption prohibited.

## 1. Exact upstream version, release and provenance

Source: official [signalapp/libsignal GitHub repository](https://github.com/signalapp/libsignal)
and its [README](https://github.com/signalapp/libsignal/blob/v0.105.2/README.md).
The latest published GitHub release returned on the research date was
[`v0.105.2`](https://github.com/signalapp/libsignal/releases/tag/v0.105.2),
published **2026-10-20 17:31:27 UTC**. The version was queried, **not
assumed from ADR 0002**.

- Annotated Git tag object: `e9c46b21214dad2d0990682b0e929c397dc5c621`.
- Peeled upstream source commit: `b369755533ef5a3fae9b31b989114f22e074dfab`.
- GitHub tag verification metadata reported `verified=false`,
  `reason=unsigned`; **no signed provenance assertion** is made.
- The relevant official Maven coordinates documented upstream are
  `org.signal:libsignal-client` and `org.signal:libsignal-android`
  at the exact **candidate** version `0.105.2`, published through
  [Signal's Maven endpoint](https://build-artifacts.signal.org/libraries/maven/).
  **Neither actual Maven POM nor AAR/JAR was fetched**, so its real
  availability at those coordinates, resolved transitive dependency
  versions, binary sizes, file checksums, signer identities, and package
  integrity were **NOT VERIFIED**.
- GitHub release assets enumerated iOS checksum and platform debug symbol
  files; **no official Android AAR checksum asset appeared in the
  enumerated release assets**. The presence of a GitHub release tag
  is not a hash of the Maven Android artifact.
- Upstream release notes/artifact publication must be rechecked before
  any later evaluation if a newer version exists.

Official upstream source is pinned throughout the following references
to commit [`b3697555`](https://github.com/signalapp/libsignal/commit/b369755533ef5a3fae9b31b989114f22e074dfab).
This establishes **source inspection only**, not that the Maven binaries
were built from that exact commit.

## 2. Java/Kotlin APIs examined in the pinned source

| Java-visible surface | Verified source evidence | What is, and is not, demonstrated |
|---|---|---|
| `SessionBuilder(...).process(PreKeyBundle)` | [SessionBuilder.java](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/java/shared/java/org/signal/libsignal/protocol/SessionBuilder.java#L83-L129) | Session initiation API accepts a bundle. No caller-provided mandatory-SPQR policy parameter was found in the inspected constructor or `process` overloads. |
| `PreKeyBundle` with `kyberPreKeyId/Public/Signature` | [PreKeyBundle.kt](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/java/shared/java/org/signal/libsignal/protocol/state/PreKeyBundle.kt#L26-L37) | PQ-related prekey input exists. A KEM input alone does not prove ongoing PQ ratcheting or authenticated peer context. |
| `SessionCipher.encrypt(...)` and `decrypt(...)` | [SessionCipher.java](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/java/shared/java/org/signal/libsignal/protocol/SessionCipher.java#L146-L165) | Public session encryption/decryption functions exist. They do not expose a documented explicit PQXDH+SPQR profile-selecting parameter. |
| `SessionRecord.getSessionVersion()` | [SessionRecord.java](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/java/shared/java/org/signal/libsignal/protocol/state/SessionRecord.java#L52-L58) | Returns an integer **session wire version**, not proof of enabled SPQR or anti-downgrade policy. Official Java tests use version 4 for PQXDH bundle cases; this alone is insufficient. |
| `SessionRecord.hasSenderChain()` | [SessionRecord.java](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/java/shared/java/org/signal/libsignal/protocol/state/SessionRecord.java#L108-L125) | Provides a useful yes/no check of sender-chain usability. Tagged [Rust JNI bridge](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/rust/bridge/shared/src/protocol.rs#L977-L983) requires **NotStale + EstablishedWithPqxdh + Spqr**. This is stronger evidence than version alone, **not** a public configurable profile requirement. |
| `SignalProtocolStore` and subordinate stores | [SignalProtocolStore.java](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/java/shared/java/org/signal/libsignal/protocol/state/SignalProtocolStore.java), [SessionCipher.java](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/java/shared/java/org/signal/libsignal/protocol/SessionCipher.java#L50-L79) | Caller supplies durable session, identity, classical prekey, signed prekey, and Kyber prekey stores and addresses. GhostRoom would have to implement trusted, crash-safe storage and verification. |
| Triple Ratchet native integration | [triple_ratchet.rs](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/rust/protocol/src/triple_ratchet.rs), [session_management.rs](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/rust/protocol/src/session_management.rs) | Actual native code combines Double Ratchet + SPQR and delegates session management; the presence of native implementation alone does **not** establish a third-party *supported* Android profile contract. |

**Critical warning from upstream at this exact tag:**
[`SessionUsabilityRequirements::Spqr`](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/rust/protocol/src/state/session.rs#L100-L132)
allows an **unacknowledged** local session that advertises SPQR to be
considered usable; the code documentation warns the peer may discard
SPQR information and the local client may allow a **later downgrade**.
It cautions explicitly not to use this criterion for usability unless
future downgrades will also be rejected. A consumer cannot conclude
that checking `hasSenderChain()` once before the first send is a
complete downgrade-protection policy.

The observed [Rust session management](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/rust/protocol/src/session_management.rs)
invokes `OutgoingTripleRatchet` and `TripleRatchet`. This is evidence
of implementation rather than a proven end-to-end GhostRoom profile,
client behavior or peer identity ceremony.

## 3. Decisive questions — status by requirement

| # | Requirement | Step 4C finding |
|---|---|---|
| 1 | Documented **supported third-party Android API** | **BLOCKED.** Java API exists, but [official README](https://github.com/signalapp/libsignal/blob/v0.105.2/README.md#L19-L25) explicitly states use **outside Signal is unsupported**, and APIs/JNI bridges may change without notice. |
| 2 | Select and enforce **PQXDH + actual Triple Ratchet/SPQR** | **BLOCKED.** Native PQXDH+SPQR code and Java sessions found, but no release-supported Java profile selection/requirement argument demonstrated in inspected `SessionBuilder`/`SessionCipher` API. No runtime negotiated-profile test. |
| 3 | Detect and reject **classical fallback/downgrade** | **BLOCKED.** JNI's `hasSenderChain` check requires PQXDH and SPQR state; however, upstream explicitly warns of **unacknowledged** session downgrade. No end-to-end *fail closed before/after peer acknowledgement* test was executed. |
| 4 | Reliably determine actual negotiated security profile | **PARTIAL / BLOCKED.** `hasSenderChain()` is a meaningful binary usability check as implemented at this revision; `getSessionVersion()` is not proof of SPQR. No supported public negotiated profile descriptor or downgrade policy contract established. |
| 5 | Authenticated peer identity, room, roles and transcript | **BLOCKED.** Identity key trust is delegated to caller-provided stores and peer-verification UX; no API establishes GhostRoom room/onion/role/transcript authentication automatically. No QR/SAS or malicious-relay integration tested. |
| 6 | Persistent session, prekey and identity storage | **CALLER RESPONSIBILITY (unimplemented).** The Java session/identity, one-time, signed and Kyber prekey store interfaces exist. GhostRoom has no Android state vault, trust database, key rotation/consumption scheme or backup/crash policy. |
| 7 | Safe crash/reconnect and ratchet state transitions | **BLOCKED.** Upstream ratchet stores state, but no GhostRoom atomic send-before/after-persistence and restart test was run; recovering lost state must not cause key reuse or unverified reset. |
| 8 | Android/ABI integration | **DOCUMENTED, NOT EXECUTED.** Official source lists Android `armeabi-v7a`, `arm64-v8a`, `x86`, `x86_64` artifacts. Tagged [Android Gradle](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/java/android/build.gradle) uses `minSdkVersion 23` and `compileSdk 34`; README calls for JDK 21 and Android SDK/NDK. No binary or device/emulator was available. |
| 9 | Maintainable supported API | **BLOCKED.** Official upstream explicitly disclaims third-party support and reserves API/bridge changes. No GhostRoom update contract or compatible maintenance plan exists. |
| 10 | MIT project + dependency and distribution licensing | **BLOCKED pending qualified review.** libsignal repository is **AGPL-3.0** (including Rust source with SPDX AGPL-3.0-only). Shipping it with GhostRoom needs a detailed analysis of combinations, linked native libraries, app distribution, corresponding source, notices, possible network-service obligations, dependencies and any alternative authorized licensing. No review or distribution approval obtained. |

This is **not** a categorical claim that the Java API *cannot* perform
PQ sessions: the inspectable session path and native flags demonstrate
a plausible implementation surface. It is a finding that **GhostRoom's
mandatory supported + enforceable + measured third-party integration
requirements were not demonstrated**.

## 4. Formal protocol properties versus verified app behavior

- [PQXDH specification revision 3](https://signal.org/docs/specifications/pqxdh/):
  intended PQ forward secrecy for initial key establishment, but its
  specified mutual identity authentication remains based on classical
  discrete-log assumptions. The server is not an identity verifier.
- [Double Ratchet revision 4](https://signal.org/docs/specifications/doubleratchet/):
  SPQR is an ongoing PQ ratchet; Triple Ratchet combines classical and PQ
  keys for hybrid security. With an appropriate PQ SCKA and correct
  implementation it is designed for PQ confidentiality of ongoing traffic.
  PQ PCS depends on honest exchanged messages and can be **delayed by
  dropped traffic**. A malicious relay can induce such delays.
- Protocol specifications and native sources do not establish that
  GhostRoom selected an actual deployed profile, or has PQ PCS and
  authentication under the relevant attacker model.
- Replay/out-of-order handling exists in protocol designs; app-level
  duplicate and cross-room protections remain unverified.
- Deniability and post-quantum authentication require separate analysis;
  **post-quantum confidentiality must never be called post-quantum
  authentication**.

## 5. Artifact provenance, environment and actual commands

No production dependency changed and **no code prototype was written**.
The user's experiment condition allows stopping before code if
upstream API/support or licensing restrictions make a test inappropriate.
Both apply here: outside-Signal use is **officially unsupported**;
AGPL-3.0 redistribution/combined-work obligations are unresolved.
A speculative local sample would not be an approved integration,
and claiming it proved security would be misleading.

Actual local environment inspection (2026-10-21 UTC):

```text
java -version     OpenJDK 21.0.12.1 (2026-08-18)
javac -version    javac 21.0.12.1
cargo             unavailable
rustc             unavailable
gradle            unavailable
adb               unavailable
sdkmanager        unavailable
emulator          unavailable
```

Actual fetch attempt (headers only, no artifact bytes):

```bash
curl -IL --max-time 10 \
  'https://build-artifacts.signal.org/libraries/maven/org/signal/libsignal-android/0.105.2/libsignal-android-0.105.2.pom'
```

Observed result: `curl: (6) Could not resolve host:
build-artifacts.signal.org` (exit code 6).
The endpoint path is a conventional artifact path based on the
documented Maven coordinate; its existence was **not** verified.

Not executed: Gradle build/test, Maven dependency resolution, AAR
checksum verification, Java two-user handshake, official crypto vectors,
Android tests, per-ABI binaries, identity authentication, anti-downgrade
tests, memory/latency/CPU measures. **There are no measurements** of
native binary size, device RAM, protocol overhead, or performance.
GitHub's SHA/digest metadata for non-Android release assets does not
verify Android AARs.

## 6. Reproducible future experiment — NOT EXECUTED

Proceed **only after** support/AGPL licensing review makes a
non-shipping evaluation permissible and a usable Android environment
and artifact source are available.

1. Resolve **exact** `0.105.2` Signal Maven POM/AAR/JAR artifacts from
   the official repository (or a newer release, recorded and reviewed
   afresh). Record each SHA-256 checksum, Maven POM, native ABI
   packaging, transitive license manifest and verified source release
   provenance; pin resolution and prohibit artifact drift.
2. Use the upstream **Java public** `SessionBuilder`,
   `PreKeyBundle`, `SessionCipher`, `SessionRecord.hasSenderChain`
   and proper `SignalProtocolStore` implementations, not internal
   `Native`, `triple_ratchet.rs`, undocumented bridge hooks, or
   any hand-coded ratchet/KEM. Start with the official
   [Java PQXDH session test](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/java/client/src/test/java/org/signal/libsignal/protocol/SessionBuilderTest.java)
   and [SessionCipher test](https://github.com/signalapp/libsignal/blob/b369755533ef5a3fae9b31b989114f22e074dfab/java/client/src/test/java/org/signal/libsignal/protocol/SessionCipherTest.java),
   not invented test vectors.
3. On two actual endpoints using room-scoped fresh keys, prove
   mutually authenticated identity and room/role/transcript commitments
   with a reviewed independent OOB ceremony. Prove session profile
   before send, after first acknowledgement, after resume, after
   parallel initiation, and after receiving intentionally downgraded
   prekey responses. Abort on all classical fallbacks.
4. Exercise tampering, duplicate/out-of-order delivery, crash/power
   interruption, prekey exhaustion, replay and recovered store state.
   Demonstrate no accidental old message-key reuse.
5. Build and run on `arm64-v8a` and at least one other documented ABI,
   with SDK/NDK and Gradle/JDK versions recorded. Record artifact sizes,
   cold startup, setup latency, memory and per-message wire overhead
   **only from observed measurements**.
6. Have independent security engineering and qualified licensing review
   the exact client integration before reconsidering ADR 0002.

Suggested reproduction command categories (not executed here):
`./gradlew :client:test` for upstream Java tests and
`./gradlew :android:assembleDebug` for tagged upstream Android
artifact source, after confirming actual Gradle task names in that
checkout. No claim these build successfully in this environment.

## 7. Decision and next action

**Outcome B — candidate rejected as a *currently approvable GhostRoom
third-party Android integration*.** The API is technically
promising but external support, complete enforceable downgrade policy,
peer verification, crash-safe session state, exact binary provenance,
runtime behavior, and AGPL distribution path remain **BLOCKED**.
No production crypto dependency, Android source, or GhostRoom protocol
implementation should be added. ADR 0002 remains **NO-GO**.

**Recommended next step:** first resolve the third-party-use/support
arrangement and obtain qualified AGPL distribution guidance. Then make
a disposable, independently isolated official-API compatibility harness
on a real Android toolchain with pinned official artifacts; verify
two-client acknowledged SPQR state and *deliberate downgrade attempts*
before requesting an ADR acceptance review.

This report is a reproducible **source/API and feasibility-gate
evaluation**, not a successful cryptographic prototype or an audit.
