# ADR 0001: E2EE protocol selection initially deferred

**Status: superseded by [ADR 0002](0002-ephemeral-two-party-e2ee.md)**
(2026-10-21). Original record: proposed 2026-10-20.

GhostRoom must not invent a new ratchet, key exchange or encryption protocol.
The final two-person E2EE mechanism must be selected after comparing mature,
audited library implementations (libsignal, vodozemac, potentially MLS if
groups become relevant) for Rust server boundaries, Android support,
licenses, audit history, forward secrecy, post-compromise recovery,
authenticated onboarding, and hybrid PQ capabilities. ML-KEM standardization
does not turn it into a complete messaging protocol. The server-side room
capabilities implemented in Step 2 only gate **room operations** and never
substitute for client-held E2EE identity authentication. Future work must
define replay detection, ordering, key deletion, recovery and transcript
binding before message transport integration.

ADR 0002 records the October 2026 research comparison, conditional
candidate and unresolved authentication and post-quantum gates.

## Step 4B decision (2026-10-21)

Successor [ADR 0002](0002-ephemeral-two-party-e2ee.md) now records
**NO-GO / BLOCKED** under a mandatory first-release post-quantum
confidentiality requirement. This predecessor remains superseded.
