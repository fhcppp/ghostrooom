# Step 2 security research — 2026-10-20

This is a design review, not a cryptographic audit. GhostRoom's Step 2 code
does not implement message encryption, a Tor client, Android clients, or a
network protocol. Review upstream security advisories again before shipping.

## Existing messaging designs

- [SimpleX Chat](https://github.com/simplex-chat/simplex-chat) avoids permanent
  global user IDs and separates messaging-client responsibilities from relays.
  Its security policy describes independent reviews; neither those reviews
  nor its threat model validate this GhostRoom implementation.
- [SimpleXMQ](https://github.com/simplex-chat/simplexmq) distinguishes sender
  and recipient authority, documents lifecycle controls and queue authentication,
  and includes a [2026-05-25 protocol revision](https://github.com/simplex-chat/simplexmq/blob/stable/protocol/simplex-messaging.md).
  Queue deletion and transport crypto are deferred here.
- [chattor](https://github.com/jmsingleton/chattor) and
  [Ricochet Refresh](https://github.com/blueprint-freespeech/ricochet-refresh)
  demonstrate peer onion communication. GhostRoom instead plans self-hosted
  relay instances, reducing peer-onion uptime requirements while putting
  metadata and queue retention responsibilities on relay operators.
- [Orbot](https://github.com/guardianproject/orbot-android) documents Android
  Tor service behavior. [Arti](https://gitlab.torproject.org/tpo/core/arti)
  offers Rust Tor functionality; Android compatibility and deployment maturity
  require separate verification.
- [Tokio](https://github.com/tokio-rs/tokio) will be useful when a future server
  must serialize room state transitions, control backpressure, and cancel tasks.

## Credential primitives selected

- [getrandom](https://docs.rs/getrandom/latest/getrandom/) version 0.3, using
  OS-backed cryptographic randomness on Linux/Android. A failure aborts
  creation or redemption without changing room state. Room/participant IDs
  use 128 random bits; credentials use 256 random bits.
- [sha2](https://docs.rs/sha2/0.10.9/sha2/) version 0.10: SHA-256 digest
  verifiers, with distinct domain separators and the room ID (and for participant
  access, participant ID) included. A digest is suitable for 256-bit uniformly
  random bearer tokens, but NOT suitable as a password KDF.
- [subtle](https://docs.rs/subtle/) version 2 compares fixed-size digests in
  constant time. Authentication branches over public participant IDs.
- [zeroize](https://docs.rs/zeroize/) version 1 erases owned byte arrays on
  drop; it cannot guarantee the removal of copies in transport or VM memory.

No secret-bearing type implements `Clone`, `Copy`, or raw `Debug`.
The OS RNG contract matters more than statistical uniqueness unit tests.
Credential digests are in-memory only; whether they need keyed protection at
rest depends on a future persistence and compromise model. SHA-256 verifiers
cannot be reversed efficiently given randomly chosen 256-bit capabilities,
but an attacker controlling server state can alter access decisions.

## E2EE designs evaluated for future steps

- [libsignal](https://github.com/signalapp/libsignal): maintained Double
  Ratchet-related implementations; its language bindings, licensing, protocol
  compatibility, and Android integration need careful verification.
- [vodozemac](https://github.com/matrix-org/vodozemac): Rust Olm/Megolm
  ecosystem, with protocol-specific trust and cross-platform constraints.
- [OpenMLS](https://github.com/openmls/openmls): MLS implementation; the
  group-oriented model may be excessive for a two-person ephemeral room.
- [NIST FIPS 203](https://csrc.nist.gov/pubs/fips/203/final) standardizes
  ML-KEM, not a complete secure messaging protocol. The NIST page notes
  a 2025 planning note for forthcoming corrections; review errata before use.
- Hybrid post-quantum handshakes may improve resistance to recorded traffic
  being decrypted later, but do not automatically provide forward secrecy,
  post-compromise recovery, authentication, or safe ratchet-state storage.

Do not build a custom E2EE protocol in GhostRoom. Prefer a mature reviewed
implementation and an explicit protocol/version negotiation policy. A future
implementation needs authenticated identity binding, replay windows, message
ordering, ephemeral key erasure, forward secrecy, and compromise recovery.
Invitation possession alone is not E2EE identity verification.

## Deferred analysis

Before production: perform crate and transitive dependency version/advisory
review, Android target CI, licensing review (including any copyleft components),
app-specific Tor fail-closed tests, authenticated channel bootstrap, user key
verification UX, persistent secret and lock-screen storage review, replay/
rate-limit enforcement, memory limits, queue retention, metadata leakage
analysis, threat modeling, and independent security assessment.
