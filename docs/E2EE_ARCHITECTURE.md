# GhostRoom Step 4 — E2EE authentication and lifecycle contract

**Design only. Status: NO-GO / BLOCKED for production E2EE; no encryption code exists.** See
[ADR 0002](adr/0002-ephemeral-two-party-e2ee.md) and
[candidate review](research/step-4-e2ee-candidate-review.md).

## Trust and identities

GhostRoom's self-hosted server may be actively malicious. It controls
advertised room public keys, traffic ordering and timing, may replay,
replace or withhold public keys and ciphertext, and may observe metadata.
The server's `RoomId`, `ParticipantId`, invitation and bearer access
capabilities do **not** prove end-to-end peer identity. Tor service
authentication is not substitute for peer authentication.

Alice and Bob each create fresh client-owned **room-scoped** signing/
identity and Olm prekeys using the selected library. Account identity
keys are not uploaded as plaintext private keys and never reused across
rooms. There is no global account/identity directory. A room
identity is expected to be verified per room.

## Historical Olm-specific example — superseded, DO NOT IMPLEMENT

The onboarding steps in this section were written before mandatory
post-quantum confidentiality was decided. Olm-specific accounts,
prekeys, SAS methods and first-message references are historical
research examples, **not current GhostRoom implementation instructions**.
Use only a future, fully reviewed and explicitly ACCEPTED post-quantum
protocol profile. ADR 0002 remains **NO-GO / BLOCKED**.

## Proposed verified onboarding (historical candidate only)

1. Alice requests a waiting room from the manager. On her own device,
   she creates a fresh candidate `olm::Account` and a session-specific
   identity-key commitment. The server sees only appropriate public
   key material, not private keys.
2. A strict, versioned invitation carries the onion-service destination,
   room ID, one-time join capability, expiration/policy indicator and
   Alice's cryptographic identity-key commitment. For **authenticated**
   onboarding, the entire invitation or at least the peer-key commitment
   must reach Bob over a preauthenticated OOB channel (in-person QR
   preferred). Merely sending a URL over an untrusted channel is not
   authentication.
3. Bob validates the invitation's encoding, onion destination and
   commitment; creates his own fresh account, signing and one-time
   prekeys, and redeems the invitation. The server can relay the
   handshake but cannot be trusted to assert Bob's public key.
4. Alice verifies the server-relayed Alice/Bob handshake keys against
   her own recorded invitation commitment and Bob's authenticated
   OOB commitment or mutually confirmed verification ceremony.
   Bob verifies the received Alice keys match his trusted invitation.
5. Their first authenticated exchange must bind, unambiguously and in
   canonical order: protocol profile/version, Tor onion destination,
   room ID, Alice/Bob room roles and participant IDs, identity signing
   keys, Olm Curve25519 prekeys, ephemeral handshake keys, session
   identifier, and a fresh room context. The transcript hash must be
   fixed only after all fields are committed; the actual authenticated
   ratchet must protect the application context and verification
   messages. Do **not** compose a new cryptographic handshake.
6. A QR/fingerprint or SAS verification ceremony covers **both parties'
   key identities and the full transcript** with separate confirmation
   from Alice and Bob. Use vodozemac's SAS/MAC facility only after
   verifying its exact protocol flow and key-to-transcript binding.
   A matching short string alone is not proof if the attacker can
   control both channels/strings without an independent human check.
   Compare in person or over an *already authenticated* separate
   channel. Bind each side's explicit confirmation to the intended
   session; do not accept reflected/swap-role confirmations.
7. Only after both sides explicitly confirm matching verification do
   clients enable sending/displaying user message plaintext. Reject
   mismatched identities, altered room context, absent confirmations,
   unsupported versions or unexpected concurrent session changes.
   The strict initial UX has no unverified-mode fallback.

These are **protocol integration requirements**, not a validated
cryptographic construction. A future implementation, once explicitly approved, must use the
selected library's documented handshake and authenticate binding data
inside the library-protected content; integration must undergo an
independent cryptographic review before deployment.

## Threat outcomes

| Situation | Expected property / limitation |
|---|---|
| Honest clients, malicious server, authenticated OOB and full mutual verification | Server cannot silently replace the verified peer encryption identity or decrypt content under the selected protocol assumptions |
| Malicious server with only bearer/invitation secrets | Can join first or deny service and misroute; cannot impersonate a properly verified identity without fooling OOB verification |
| Invitation sent via unauthenticated channel | MITM/impersonation remains possible; never call this identity-verified |
| Verification skipped or pending | Block message plaintext send and display in strict profile |
| SAS mismatch, changed identity, unexpected session | Stop the handshake, discard pending session, signal explicit failure; do not downgrade or auto-trust new keys |
| Server partitions both peers | Can deny service, cause separate attempted sessions or leak traffic metadata; verified clients must refuse divergent transcript or parallel sessions |
| Device compromise or recipient screenshot | Endpoint plaintext/key disclosure possible; no server-room closure can revoke screenshots or backups |

## Lifecycle and local protection

1. **Create:** client-side OS CSPRNG, fresh room identity and session keys;
   secrets stay on endpoint. Do not persist in server logs.
2. **Initiate:** library-supported pre-key/session initiation using pinned
   signed identity commitment and authenticated room context.
3. **Mutual verify:** explicitly confirm transcript and both identities
   before any user plaintext send or display.
4. **First message:** an Olm pre-key or normal message as required by
   upstream API, containing versioned authenticated application context.
5. **Normal operation:** keep authenticated inbound replay state and
   bounded skipped-message-key limits; use the library's ratchet only.
6. **Duplicates/out-of-order:** ratchet processing must reject already
   consumed keys and repeated authenticated application message IDs.
   Out-of-order delivery only within a tested bounded window; no
   unbounded skipped-key cache.
7. **Update:** rely on library-provided DH ratchet; post-compromise
   recovery requires honest fresh DH ratchet activity, verified peers,
   endpoint remediation and correct old key disposal.
8. **Disconnect:** preserve session ratchet state only within strictly
   protected local lifetime. No transparent resetting of sender counters
   after crash; if state is uncertain/lost, create a new room and
   mutually reverify. No cloud backup in initial strict policy.
9. **Compromise:** alert, abort, and start a new verified session.
   Classical Olm has no substantiated quantum resistance.
10. **Close/expire:** manager atomically stops active operations;
    the client closes the ratchet and disposes of owned session keys,
    pending messages, user-facing plaintext buffer and derived secrets.
11. **Delete:** best-effort zeroization of owned buffers and keys,
    deletion of app-owned state, and disabling backup for ephemeral
    storage. The OS may hold copies in stacks, registers, swap,
    tombstones, screenshots, notifications or inaccessible snapshots;
    zeroization never guarantees forensic erasure.

## Android-specific requirements

The app must handle pause/resume, device lock, screenshot/notification
leakage policies, process death and reconnection without duplicating
ratchet use. Keep secrets out of crash telemetry and logcat. Decide
whether app state is strictly memory-only or backed by Android
Keystore-wrapped state before implementation. Native-to-Kotlin copies,
JNI/UniFFI exception paths and cancellation require target-device
testing. Auto backup and cross-device sync remain disabled until
explicitly reviewed. Any E2EE protocol implementation and in-process
crypto state belong to **client devices**, not the relay.

## Open architecture gates

**The first-release post-quantum confidentiality requirement is decided
and mandatory.** The protocol implementation and supported client APIs,
qualified license review, authenticated transcripts, downgrade/replay
protection, skipped-message-key limits, SAS ceremony, Android secret
storage and peer-verification UX remain **unverified or blocked**.
See ADR 0002 and the Step 4C/Step 5 evidence and acceptance checklist.

## Step 4B non-negotiable PQ policy

The product requires post-quantum confidentiality against recorded
traffic from the first public release. The E2EE protocol decision is
**NO-GO**, not simply awaiting a preference for classical Olm. Neither
PQ initial key exchange alone nor cryptographic algorithm availability
proves a protected ongoing ratchet. A reviewed complete client protocol
must document initial PQ secrecy, continuing PQ forward secrecy,
actual PQ post-compromise recovery (or its absence), classical identity
signatures, replay and Android integration. No fallback to an
unverified or classical-only session is permitted. The nearest
feasibility candidate, **Signal libsignal PQXDH + Triple Ratchet**, is
not approved: official external-use support, API/profile enforcement,
Android behavior and AGPL-3.0 obligations remain unresolved. This
document is a threat-model contract, not an implementation specification.
