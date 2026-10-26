//! Pure synchronous GhostRoom lifecycle, invitation, and capability rules.
//!
//! Existing `Room` is the verified Step 1 two-participant engine. `SecureRoom`
//! is the Step 2 waiting-room and authorization engine. It does not provide
//! networking, Tor, encrypted messaging, server synchronization, or persistence.
//! All `Instant` values must originate from trusted server code.
//! Concurrent lifecycle transitions are serialized by [RoomManager].
//! Public participant and room IDs never authenticate; bearer capabilities do.
//! No HTTP listener, Tor routing, ciphertext queue, E2EE, or durable storage
//! is provided by this core crate.
#![forbid(unsafe_code)]
mod credentials;
mod error;
mod ids;
mod manager;
mod room;
mod secure_room;

pub use credentials::{InvitationSecret, ParticipantAccessCapability, RandomError};
pub use ids::{ParticipantId, RoomId};
pub use manager::{
    CleanupStats, CreatedManagedRoom, ManagerError, RoomLifecycle, RoomManager, RoomManagerConfig,
};
pub use room::{CloseReason, Closure};

/// Legacy ID-only state machine for tests and migration only.
///
/// **Never use this module to authorize a remote client.** The supported
/// application-facing lifecycle API is [RoomManager], which checks
/// bearer capabilities and uses trusted manager-owned time.
pub mod legacy {
    pub use crate::error::RoomError;
    pub use crate::room::{Room, RoomState};
}
pub use secure_room::{
    CreatedRoomCredentials, JoinedParticipant, ParticipationError, ParticipationState, SecureRoom,
};

#[cfg(test)]
mod participation_tests;
