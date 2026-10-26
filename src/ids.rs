//! Compact, room-local identifiers; neither type is an access credential.
use crate::credentials::RandomError;

/// Public 128-bit room ID, not a bearer authorization credential.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct RoomId(u128);
impl RoomId {
    /// Create an unpredictable room identifier with OS-backed entropy.
    pub fn generate() -> Result<Self, RandomError> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes)?;
        Ok(Self(u128::from_be_bytes(bytes)))
    }
    /// Deterministic adapter constructor (legacy Step 1 compatibility).
    ///
    /// Do not use for production room creation; use `generate` and detect
    /// collisions in the actual RoomManager registry.
    #[must_use]
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }
    /// Return raw identifier bytes as a compact integer.
    #[must_use]
    pub const fn as_u128(self) -> u128 {
        self.0
    }
}

/// Room-scoped participant label, not a proof of authentication.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParticipantId(u128);
impl ParticipantId {
    /// Generate an unpredictable participant ID from the host OS.
    pub fn generate() -> Result<Self, RandomError> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes)?;
        Ok(Self(u128::from_be_bytes(bytes)))
    }
    /// Deterministic adapter constructor (legacy Step 1 compatibility).
    ///
    /// A label never authenticates its bearer. Use separate capabilities.
    #[must_use]
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }
    /// Return the raw compact identifier.
    #[must_use]
    pub const fn as_u128(self) -> u128 {
        self.0
    }
}
