# ADR 0002 — First-release post-quantum E2EE decision

**Status: NO-GO / BLOCKED — production E2EE implementation is not approved.**
**Decision date:** 2026-10-21.
**Previous status:** UNRESOLVED (Step 4).
**Predecessor:** [ADR 0001](0001-future-e2ee.md).
**Research:** [Candidate review](../research/step-4-e2ee-candidate-review.md).
**Design:** [Authentication and lifecycle](../E2EE_ARCHITECTURE.md).
**Envelope:** [Future envelope](../E2EE_MESSAGE_ENVELOPE.md).
**Exit gates:** [Step 5 prerequisites](../E2EE_STEP5_TEST_PLAN.md).
**Step 4C evidence:** [Signal Android feasibility report](../research/step-4c-signal-feasibility.md).

## Decision

GhostRoom **requires post-quantum confidentiality for its first public
release**, including resistance to a passive **record-now/decrypt-later**
adversary. This is a product requirement, not a deferred optional feature.
Initial key establishment **alone** does not establish quantum-resistant
forward secrecy or post-compromise recovery of ongoing message traffic.
The selected implementation must identify its actual PQ ratchet, session
profile and negotiated downgrade protection. Do not describe classical
peer signatures or identity verification as post-quantum authentication.

**No current candidate is approved for GhostRoom production integration
at the time of this review. Step 5 encryption implementation is BLOCKED.**
This is a decision **not to integrate yet**, not another unresolved ADR
and not a claim that the reviewed upstream projects are insecure.

The strongest concrete feasibility route to investigate is
[Signal libsignal](https://github.com/signalapp/libsignal) using its
**existing PQXDH plus Triple Ratchet/SPQR integration**, with
**Signal's own Java/Android client artifacts**, rather than inventing
a new Rust-to-Kotlin cryptographic wrapper. Its repository's released
version observed on 2026-10-21 was
[`v0.105.2`](https://github.com/signalapp/libsignal/releases/tag/v0.105.2)
(published 2026-10-20). **This is an evaluation target, not an approved
GhostRoom version/profile or a claim that its Android API offers all
required controls.** Its actual exposed session creation, negotiated
ratchet selection, profile enforcement, identity storage and third-party
support must be verified against that release.

## Step 4C final feasibility disposition (2026-10-21)

**Outcome B: Signal libsignal is not approved for GhostRoom production
integration under the evidence available.** This does **not** reject the
Signal protocol itself or claim its cryptographic implementation failed
tests. The [Step 4C report](../research/step-4c-signal-feasibility.md)
establishes the following distinct findings at the official
`v0.105.2` source tag (upstream commit
`b369755533ef5a3fae9b31b989114f22e074dfab`):

- **Verified in source:** Java/Kotlin-facing `SessionBuilder`,
  `SessionCipher`, `SessionRecord` and PQ prekey APIs exist. The JNI
  sender-chain usability check requires non-stale PQXDH-established
  sessions with SPQR state. Session version by itself does not prove
  an actual, accepted SPQR profile.
- **Not demonstrated:** a supported third-party interface to pin and
  enforce PQXDH **and ongoing Triple Ratchet/SPQR**; rejection of a
  downgrade after an unacknowledged SPQR session; a reliable
  client-visible negotiated-profile contract; two-client
  transcript/identity verification; or crash-safe session persistence.
  Upstream source explicitly warns about possible later downgrades
  if a peer discards unacknowledged SPQR information.
- **Artifacts and runtime not verified:** neither official Maven
  AAR/JAR/POM nor transitive dependencies or Android SHA-256 checksums
  were obtained. No prototype, official-vector execution, Android
  ABI/device test, or performance measurement was run; therefore
  **no cryptographic test failed**.
- **Support/licensing blocked:** Signal explicitly says third-party
  use is unsupported; AGPL-3.0 obligations alongside GhostRoom's MIT
  project require qualified licensing/distribution review. A usable
  long-term maintenance contract and vulnerability-response process
  have not been demonstrated.

**Consequence:** a promising research surface is not an accepted
production dependency. Classical-only vodozemac/Olm is **rejected for
GhostRoom's mandatory post-quantum first-release requirements**. Do not
add a production cryptographic dependency or begin Step 5 encryption.
The NO-GO may be reconsidered **only after all exit conditions below**
have independent implementation, API, provenance, testing, licensing
and human security-review evidence and a subsequent explicit
**ACCEPTED** ADR amendment. Neither this ADR nor PR #3 is ACCEPTED.

## Non-negotiable security requirements

1. A complete, maintained two-party protocol, not a composition of
   arbitrary primitive crates. Client encryption keys remain on Android
   devices; the relay sees only opaque ciphertext and authorized routing
   metadata.
2. Post-quantum-resistant *initial* key agreement, with correct
   classical/hybrid binding and strict downgrade rejection.
3. Post-quantum confidentiality/forward secrecy during subsequent
   messages against recorded traffic, supported by the selected
   **ongoing PQ ratchet**. Identify the actual deployed profile, not
   only specification-level support.
4. Determine whether post-quantum **post-compromise recovery** is
   provided by that profile. If unsupported, document the residual
   risk and obtain explicit security approval before accepting any
   profile; do not relabel classical DH recovery as PQ recovery.
5. Verified room-scoped peer identities and mutually authenticated
   transcript, regardless of the server's claims. The protocol's
   authentication and deniability limits (including classical
   identity signatures) must be explicit.
6. Replay/duplicate rejection, bounded out-of-order keys, atomic
   send-state persistence or safe fail-closed restart, and client
   key-erasure policy; unsupported versions/identity changes fail closed.
7. A realistically supported, maintainable Rust/Kotlin/Android
   integration with reviewed licensing and transitive dependencies,
   validated on real target ABIs and devices.

## Upstream candidates: evidence and disposition

| Candidate | Observed upstream evidence | Decision |
|---|---|---|
| **Signal libsignal** | [PQXDH](https://signal.org/docs/specifications/pqxdh/) provides PQ key agreement but still classical public-key identity authentication; [Double Ratchet spec sections 5–7](https://signal.org/docs/specifications/doubleratchet/) describe SPQR/Triple Ratchet. Current [Rust source](https://github.com/signalapp/libsignal/blob/main/rust/protocol/src/triple_ratchet.rs) has a Triple Ratchet integration and [PQXDH source](https://github.com/signalapp/libsignal/blob/main/rust/protocol/src/pqxdh.rs). [README](https://github.com/signalapp/libsignal/blob/main/README.md) documents official Java/Android AARs **but states external use unsupported**, with APIs/bridges changeable without notice. Repo is **AGPL-3.0**. | **Closest feasibility target, NOT ACCEPTED.** Exact public Android APIs/profile, lifecycle, third-party support, compatibility, build and AGPL distribution obligations unresolved. |
| **SimpleX Chat / SimpleXMQ** | [Security spec](https://github.com/simplex-chat/simplexmq/blob/stable/protocol/security.md) describes Curve448 plus optional **SNTRUP761** parallel PQDR; [PQDR spec](https://github.com/simplex-chat/simplexmq/blob/stable/protocol/pqdr.md) includes KEM evolution in the ratchet. [Agent protocol](https://github.com/simplex-chat/simplexmq/blob/stable/protocol/agent-protocol.md) permits turning PQ mode **off** for compatibility; its [source architecture](https://github.com/simplex-chat/simplexmq/blob/stable/contributing/PROJECT.md) includes Haskell agent/queue stack. Both repositories are **AGPL-3.0**. | **NOT ACCEPTED.** Integration is not a supported standalone Rust/Kotlin PQ session API, entire agent/transport semantics and mandatory-PQ negotiation need proof; licensing unresolved. Never copy PQDR into a homemade Rust ratchet. |
| **vodozemac / Olm** | [Readme](https://github.com/matrix-org/vodozemac) documents a maintained Apache-2.0 Rust Olm/Megolm implementation and historical Least Authority audit, with `olm::Account`/`Session`; inspected Olm API provides **classical** key agreement/ratchet, with no verified PQ KEM or PQ ratchet. | **REJECTED for first public release**, regardless of otherwise attractive two-person and Rust API fit. No ML-KEM grafting. |
| **OpenMLS / MLS** | [OpenMLS main README](https://github.com/openmls/openmls/blob/main/README.md) lists only classical X25519/P-256 HPKE ciphersuites and says Android targets are *built but not tested* in CI. [RFC 9420](https://www.rfc-editor.org/rfc/rfc9420) is group key agreement, not evidence of a reviewed PQ profile for this implementation. OpenMLS is **MIT**. | **NOT ACCEPTED.** No verified required PQ cipher suite/end-to-end profile and added group epoch/credential/commit complexity for two fixed peers. |
| **Custom ML-KEM / third-party primitive assembly** | [NIST FIPS 203](https://csrc.nist.gov/pubs/fips/203/final) standardizes a KEM only, with a 2025 errata planning note. | **PROHIBITED** as an invented handshake, hybrid ratchet or unsupported adaptation. |

**Licensing is unresolved.** Signal and SimpleX AGPL-3.0 obligations
may require distribution/source-compliance and a conscious overall
GhostRoom licensing strategy; this MIT project must not quietly
claim that adding AGPL code has no implications. Obtain counsel or
qualified license review; explore any authorized alternative license
or supported integration arrangement without assuming one exists.
Vodozemac/Apache-2.0 and OpenMLS/MIT have simpler stated licenses
but do not satisfy the mandatory PQ profile.

**Audit and performance:** upstream audits and protocol analyses
apply only to their dated scope, not GhostRoom. No Android cross-build,
interop test, binary-size measurement, ratchet memory profile, fuzzing,
full dependency-advisory analysis or independent GhostRoom crypto
audit was performed.

## Precise integration boundary if reconsidered

The nearest feasibility prototype would invoke **official**
`org.signal:libsignal-client` and `org.signal:libsignal-android`
Java/Kotlin-accessible JNI artifacts described in libsignal's README,
with release, Maven provenance and Android ABIs pinned after evaluation.
The application-facing protocol must be an upstream-supported
PQXDH + **actual Triple Ratchet/SPQR** session profile, not a
classical Double Ratchet fallback. The Rust server and
`ghostroom-core::RoomManager` remain a **ciphertext-only relay
and access/lifetime authority**; no server PQXDH key material or
client private keys are stored there. There is **no committed
interop contract or library dependency** yet.

The upstream `rust/protocol/src/triple_ratchet.rs` source explicitly
assigns identity verification, session selection, prekeys and storage
to the outer session-management layer. Its existence **does not**
prove that a documented and stable Android API can select and enforce
the desired PQ profile, or that all ratchet/exception semantics are
suitable for GhostRoom. That proof is a release-specific Step 5
*feasibility/compatibility gate*, not permission to start production
encryption.

A future shared Rust+UniFFI alternative remains an option only if
an independently supported, suitably licensed **complete PQ protocol**
exists. UniFFI's Kotlin binding capability does not turn
vodozemac's classical Olm into a PQ ratchet.

## Authentication profile (design, not implemented)

Both devices generate independent short-lived room-scoped identities
using their eventual **approved upstream protocol library**. Alice
creates an authenticated OOB QR commitment to her identity key and
onion-service/room target; Bob verifies it, then both obtain and
mutually verify Bob's key commitment and the complete session
transcript. That transcript must bind protocol profile and version,
room/onion target, participant roles, sender/receiver identities,
prekeys and session commitment; the library-authenticated channel
protects confirmation messages. Both users explicitly confirm a
comparison through a trusted independent OOB process. **No mutual
verification means no user plaintext send/display** in the strict
initial profile.

An attacker-controlled relay can substitute keys, withhold messages,
replay or partition traffic. Authenticated OOB verification and
correct protocol binding detect mismatching peer keys/split
sessions; an invitation alone does not. If OOB authentication is
compromised, the server can impersonate a peer. With correctly
verified identities and intact endpoints the relay should not be
able to read plaintext, but it can deny service and observe
traffic timing/room membership. Endpoints and recipients may
retain plaintext despite room deletion. Quantum-resistant
**confidentiality** does not imply quantum-resistant **identity
authentication** or deniability.

## Lifecycle and versioning constraints

The future implementation must use the selected upstream handshake,
PQ ongoing ratchet, replay and skipped-key mechanics without
reimplementing them. Authenticate room, roles, onion destination,
both identities and profile in initial session binding and message
context. Strictly reject lower profile, unknown versions, changed
identity, replay and divergent simultaneous sessions. Use
client-owned ratchet state with carefully specified crash/reconnect
behavior; unknown/lost state must require fresh mutually verified
session creation, never unsafe replay of counters or a silent
classical fallback.

Keep the versioned [envelope proposal](../E2EE_MESSAGE_ENVELOPE.md)
**conditional**: it must carry only opaque library ciphertext and
routing metadata, not invented nonces/MACs/KEM fields. Profile/version
downgrade resistance must be authenticated at the protocol layer,
not trusted to a server-supplied flag.

## NO-GO exit conditions: all required before ACCEPTED

1. Product owner explicitly signs off the mandatory PQ properties,
   distinguishing PQ initial agreement, ongoing PQ FS and whether
   PQ PCS is a required and established property.
2. A named maintainer or upstream-supported arrangement identifies
   a **complete, versioned, supported** mobile protocol interface
   and exact deployable PQ profile; show release-specific API evidence
   and client-to-client interoperability, not just internal source.
3. License/legal review resolves use of AGPL code alongside MIT
   components, redistribution terms, attributions and app releases.
   Do **not** change GhostRoom's MIT license automatically.
4. Independent crypto engineering review approves malicious-relay
   authentication, peer-key OOB verification, ratchet/profile binding,
   replay limits, key disposal, crash recovery and downgrade policy.
5. Android arm64 plus a second target ABI build and run the selected
   client profile with official vectors, negative MITM/replay tests,
   and measured mobile CPU, memory and ciphertext overhead.
6. Audited dependency versions, lockfiles, release provenance,
   advisories, upgrade policy and operator maintainer are documented.
7. A new ADR amendment changes status to **ACCEPTED** with evidence,
   specific session API/profile and licensing decision before
   Step 5 production encryption work begins.

## Prohibited until exit

No production crypto dependency in `Cargo.toml`, no E2EE protocol
implementation, no custom KEM/ratchet/handshake, no insecure classical
fallback, no Tor/network messenger changes justified as E2EE, and no
marketing claim that GhostRoom has PQ E2EE. A **disposable, isolated,
non-shipping evaluation** may inspect upstream integration APIs,
licensing and official vectors; it is not a production Step 5 start.

This NO-GO is a concrete engineering outcome, not an indefinite
"conditional preference." Revisit only when cited blockers are
demonstrably resolved.
