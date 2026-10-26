//! Room-scoped secrets and cryptographic verifiers.
use crate::{ParticipantId, RoomId};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

/// Error obtaining cryptographically strong bytes from the host OS.
#[derive(Debug)]
pub struct RandomError;

impl std::fmt::Display for RandomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("operating-system random generation failed")
    }
}
impl std::error::Error for RandomError {}
impl From<getrandom::Error> for RandomError {
    fn from(_: getrandom::Error) -> Self {
        Self
    }
}

/// Secret used exactly once to join a waiting room.
///
/// A secret is never included in Debug output and its backing bytes are
/// zeroized on drop (which cannot erase arbitrary external copies).
pub struct InvitationSecret([u8; 32]);

impl InvitationSecret {
    /// Generate 256 random bits from an OS-backed cryptographic source.
    pub fn generate() -> Result<Self, RandomError> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes)?;
        Ok(Self(bytes))
    }

    /// Strictly decode a 32-byte raw secret supplied by a trusted transport.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the raw bytes for secure out-of-band delivery to the invitee.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl std::fmt::Debug for InvitationSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InvitationSecret([REDACTED])")
    }
}
impl Drop for InvitationSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Room-scoped participant bearer credential (not a participant identifier).
pub struct ParticipantAccessCapability([u8; 32]);

impl ParticipantAccessCapability {
    /// Generate 256 random bits from an OS-backed cryptographic source.
    pub fn generate() -> Result<Self, RandomError> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes)?;
        Ok(Self(bytes))
    }

    /// Strictly decode 32 capability bytes provided by an authenticated channel.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow capability bytes for delivery to the owning client.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl std::fmt::Debug for ParticipantAccessCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ParticipantAccessCapability([REDACTED])")
    }
}
impl Drop for ParticipantAccessCapability {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// A constant-time compared, domain-separated one-way verifier.
///
/// This is SHA-256 of context, room ID, and 256-bit random capability.
/// It is not a password KDF: it relies on uniformly random credentials.
/// A database/host compromise still allows an attacker to replace verifiers;
/// authenticated state and transport security are separate future concerns.
pub(crate) struct Verifier([u8; 32]);

impl Verifier {
    pub(crate) fn invitation(room: RoomId, secret: &InvitationSecret) -> Self {
        Self::digest(b"ghostroom.invitation.v1", room, secret.as_bytes())
    }
    pub(crate) fn participant(
        room: RoomId,
        id: ParticipantId,
        secret: &ParticipantAccessCapability,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"ghostroom.participant.v1");
        hasher.update(room.as_u128().to_be_bytes());
        hasher.update(id.as_u128().to_be_bytes());
        hasher.update(secret.as_bytes());
        Self(hasher.finalize().into())
    }
    fn digest(domain: &[u8], room: RoomId, secret: &[u8; 32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(room.as_u128().to_be_bytes());
        hasher.update(secret);
        Self(hasher.finalize().into())
    }
    pub(crate) fn matches(&self, other: &Self) -> bool {
        bool::from(self.0.ct_eq(&other.0))
    }
}
impl Drop for Verifier {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
impl std::fmt::Debug for Verifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Verifier([REDACTED])")
    }
}
