# GhostRoom future E2EE message envelope — proposal v1

**Not implemented. Step 4B decision: NO-GO / BLOCKED.** This is a versioned design for a future opaque
relay. It is not a new cipher or ratchet wire format. The final schema
depends on the approved E2EE library's actual authenticated-data APIs.

## Historical classical Olm envelope — superseded, DO NOT IMPLEMENT

The field table below is **historical research** for a rejected
classical-only profile. In particular, `opaque_olm_message` and the
number `1` **must not become production wire fields or version
assignments**. A future approved post-quantum implementation must
define its own reviewed, protocol-native opaque message envelope.
ADR 0002 remains **NO-GO / BLOCKED**.

## Outer relay envelope (historical, untrusted, bounded)

| Field | Proposed type | Role |
|---|---|---|
| `protocol_version` | strict u16 allowlist; initial candidate `1` | Fail closed for unknown/fallback versions |
| `room_id` | existing 128-bit RoomId | Routes opaque bytes; **not authentication** |
| `sender_slot` | enum creator/invitee | Untrusted routing hint; never sender authenticity |
| `session_ref` | fixed-sized opaque identifier | Associates pending handshakes; must be checked against authenticated client context |
| `opaque_olm_message` | bounded bytes | Exactly the upstream library's PreKey/Normal serialized Olm message, including its internal ratchet metadata and integrity tag |

The future transport is not permitted to parse user plaintext or
private keys. A strict length limit and canonical encoding must be
selected in Step 5/6; field repetition, unknown critical versions,
trailing bytes or unsupported message types must reject without
partial decryption or downgrade. The relay's arrival timestamp,
delivery sequence, source address and queue ID are **untrusted
metadata**, not authenticated participant identity.

No separate outer nonce, ratchet counter, AEAD tag, DH pubkey or MAC
is introduced. These belong to the **selected library's own message
format**. Olm may not accept arbitrary external AEAD associated data;
do not assert it does. If an authenticated external-data interface is
unavailable, bind room and session context **inside the Olm-encrypted
application plaintext**, verify it after library decryption and
before using/disclosing the body, and obtain a focused review that
the initial handshake cannot be redirected cross-room. This is not
substitute for the peer-verification requirement.

## Inner application content (authenticated by selected ratchet)

The following data must be inside the library-authenticated message,
with fixed versioned encoding, canonical ordering and length limits:

- `app_payload_version`, explicit intended protocol profile;
- `room_id`, authenticated onion-service context or digest of its
  canonical form, and `session_ref`/authenticated session-binding
  commitment;
- `sender_role`, `recipient_role`, client-verifiable room-scoped
  identity key fingerprints and a complete mutually verified
  handshake-transcript digest;
- `message_id`: uniformly unique identifier or strictly validated
  sender-local sequence, distinct within one session;
- `kind`: verified handshake/confirmation, human-readable message,
  or close notification (future explicit versioned allowlist);
- `body`: application data only after successful mutual peer
  verification; never raw private session keys.

These are **application context checks**, not a new cryptographic
construction. Duplicate `message_id`/already-used library message
keys are rejected; out-of-order messages are processed only within
a bounded ratchet skipped-key window. Retain only a bounded digest/ID
replay cache if needed; establish a finite cleanup and restart policy.
The same authenticated room + transcript check applies to every
message; comparison occurs before displaying user plaintext.

## Receiver validation order

1. Validate size, envelope version, canonical encoding, destination
   context, expected sender slot and active manager room state.
2. Select only the locally pinned and verified session; never create a
   session solely because the server supplied a `session_ref`.
3. Decode the library's upstream wire message; have the library
   authenticate/decrypt it with its documented ratchet API.
4. Parse the authenticated inner version/context and check exact
   room, roles, identities, session and transcript.
5. Apply duplicate/replay checks and bounded message-ordering policy.
6. Only then expose the message body to user-visible UI.
7. Reject unknown versions, replayed/invalid sessions, invalid keys,
   authenticated-context mismatches or AEAD failure; do not fall back
   to older ciphers, alternate rooms or unverified identities.

An upstream ratchet's authentication tag validates the ciphertext,
not the server's claimed `sender_slot`. Any content observed before
completion of both-user authentication remains pending, not trusted.

The server stores, dispatches and eventually deletes **opaque bytes**.
It must not possess the ratchet private keys, decode inner application
content, or claim an envelope field as verified end-to-end identity.

## Step 4B profile gate and wire-format revision requirement

This proposal was drafted for classical Olm. It is **not** approved for
production use. In particular, `opaque_olm_message` is an historical
candidate label, **not** a selected field or a claim of post-quantum
protection. Do not implement or assign a production `protocol_version=1`
from this draft. Before encryption implementation, replace candidate-
specific terms using a concrete **supported complete PQ protocol**
profile and its native opaque ciphertext/handshake types; remove any
fields duplicating ratchet internals. Bind selected profile/version,
peer identity, room and roles in authenticated session context, and
reject any classical fallback or server-initiated downgrade. Do not
invent a new nonce, tag, KEM wire format, or key schedule.
