//! Domain errors that do not depend on transport or storage.

use std::fmt;

/// Failure to create or operate on a room.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoomError {
    /// Both participant slots were assigned the same ID.
    DuplicateParticipants,
    /// Expiration must be strictly after room creation.
    InvalidExpirationDeadline,
    /// Caller is not either of the room's two participants.
    UnauthorizedParticipant,
    /// A participant already closed the room; metadata is unchanged.
    AlreadyClosed,
    /// The room expired; metadata records its configured deadline.
    Expired,
    /// The supplied operation time predates room creation.
    OperationBeforeCreation,
}

impl fmt::Display for RoomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateParticipants => f.write_str("a room requires two distinct participants"),
            Self::InvalidExpirationDeadline => {
                f.write_str("expiration must be strictly later than creation")
            }
            Self::UnauthorizedParticipant => {
                f.write_str("participant does not belong to this room")
            }
            Self::AlreadyClosed => f.write_str("room was already closed by a participant"),
            Self::Expired => f.write_str("room has expired"),
            Self::OperationBeforeCreation => f.write_str("operation time is before room creation"),
        }
    }
}

impl std::error::Error for RoomError {}
