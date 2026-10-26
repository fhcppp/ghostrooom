//! In-memory lifecycle state machine for one two-person room.

use std::time::Instant;

use crate::error::RoomError;
use crate::{ParticipantId, RoomId};

/// Why a room made its one-way transition from active to closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseReason {
    /// A validated room participant requested closure.
    Participant(ParticipantId),
    /// The configured deadline passed or was reached.
    Expired,
}

/// Immutable metadata for a completed terminal transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Closure {
    /// Logical timestamp of the first valid terminal transition.
    pub at: Instant,
    /// Whether a participant closed the room or the deadline expired.
    pub reason: CloseReason,
}

/// A room is either active or permanently closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoomState {
    /// No terminal transition has yet been recorded.
    Active,
    /// Terminal state with the original closure details.
    Closed(Closure),
}

/// One bounded, two-participant room with no message storage.
///
/// `Room` is single-owner mutable domain state. It contains no synchronization;
/// a future room manager must serialize transitions when several clients race.
#[derive(Debug)]
pub struct Room {
    id: RoomId,
    participants: [ParticipantId; 2],
    created_at: Instant,
    expires_at: Option<Instant>,
    state: RoomState,
}

impl Room {
    /// Creates an active room with two distinct participants.
    ///
    /// If `expires_at` is present, it must be strictly later than `created_at`.
    /// IDs and instants are supplied by the caller; no random numbers or clocks
    /// are acquired internally.
    pub fn new(
        id: RoomId,
        participants: [ParticipantId; 2],
        created_at: Instant,
        expires_at: Option<Instant>,
    ) -> Result<Self, RoomError> {
        if participants[0] == participants[1] {
            return Err(RoomError::DuplicateParticipants);
        }
        if expires_at.is_some_and(|deadline| deadline <= created_at) {
            return Err(RoomError::InvalidExpirationDeadline);
        }
        Ok(Self {
            id,
            participants,
            created_at,
            expires_at,
            state: RoomState::Active,
        })
    }

    /// Returns this room's ID (not an authentication token).
    #[must_use]
    pub const fn id(&self) -> RoomId {
        self.id
    }

    /// Returns a copy of the two fixed participant IDs.
    #[must_use]
    pub const fn participants(&self) -> [ParticipantId; 2] {
        self.participants
    }

    /// Returns the room's monotonic creation instant.
    #[must_use]
    pub const fn created_at(&self) -> Instant {
        self.created_at
    }

    /// Returns the optional monotonic expiration instant.
    #[must_use]
    pub const fn expires_at(&self) -> Option<Instant> {
        self.expires_at
    }

    /// Returns the stored state without allowing mutation of its fields.
    ///
    /// Unlike `is_active_at`, this reflects only *recorded* transitions: a
    /// deadline can pass before any trusted operation discovers it.
    #[must_use]
    pub const fn state(&self) -> RoomState {
        self.state
    }

    /// Returns the immutable closure details, if a transition was recorded.
    #[must_use]
    pub const fn closure(&self) -> Option<Closure> {
        match self.state {
            RoomState::Active => None,
            RoomState::Closed(details) => Some(details),
        }
    }

    /// Returns whether `participant` belongs to this room.
    ///
    /// Membership is not proof of remote-client authorization.
    #[must_use]
    pub fn contains(&self, participant: ParticipantId) -> bool {
        self.participants[0] == participant || self.participants[1] == participant
    }

    /// Inspects logical activity at `now` without mutating the stored state.
    ///
    /// The room is active only from creation up to (but not including) its
    /// expiration deadline, and only if it has not already closed.
    #[must_use]
    pub fn is_active_at(&self, now: Instant) -> bool {
        self.state == RoomState::Active
            && now >= self.created_at
            && self.expires_at.is_none_or(|deadline| now < deadline)
    }

    /// Validates membership and continued activity, recording expiry if due.
    ///
    /// **Only call after authenticating a room-scoped capability** in the
    /// transport. `now` must be captured by trusted server code, never supplied
    /// by a remote client. An unrelated participant never mutates state,
    /// even at expiry.
    /// At the deadline the room transitions to `Closed` and returns `Expired`.
    pub fn require_active(
        &mut self,
        participant: ParticipantId,
        now: Instant,
    ) -> Result<(), RoomError> {
        self.authorize(participant)?;
        self.require_active_at(now)
    }

    /// Closes an active room at the supplied instant on behalf of a participant.
    ///
    /// `now` must come from a trusted monotonic server clock, not from an
    /// untrusted client. Authorization occurs before expiration detection. At
    /// or after the deadline expiration wins, is recorded at the deadline, and yields
    /// `RoomError::Expired`; otherwise the participant's closure wins.
    /// Subsequent calls cannot overwrite the original terminal metadata.
    pub fn close(&mut self, participant: ParticipantId, now: Instant) -> Result<(), RoomError> {
        self.authorize(participant)?;
        self.require_active_at(now)?;
        self.state = RoomState::Closed(Closure {
            at: now,
            reason: CloseReason::Participant(participant),
        });
        Ok(())
    }

    /// Records expiration if this active room's deadline has been reached.
    ///
    /// Returns `true` only if it records a new transition. Subsequent polls,
    /// and polls of a participant-closed room, return `false` without mutation.
    /// The closure time is the configured deadline, not the detection instant.
    ///
    /// This is an **internal trusted-server operation**, not an authorized
    /// client command. Never expose it directly to unauthenticated requests.
    pub fn expire_if_due(&mut self, now: Instant) -> bool {
        if self.state != RoomState::Active {
            return false;
        }
        let Some(deadline) = self.expires_at else {
            return false;
        };
        if now < deadline {
            return false;
        }
        self.state = RoomState::Closed(Closure {
            at: deadline,
            reason: CloseReason::Expired,
        });
        true
    }

    fn authorize(&self, participant: ParticipantId) -> Result<(), RoomError> {
        if self.contains(participant) {
            Ok(())
        } else {
            Err(RoomError::UnauthorizedParticipant)
        }
    }

    fn require_active_at(&mut self, now: Instant) -> Result<(), RoomError> {
        if let RoomState::Closed(details) = self.state {
            return Err(match details.reason {
                CloseReason::Participant(_) => RoomError::AlreadyClosed,
                CloseReason::Expired => RoomError::Expired,
            });
        }
        if now < self.created_at {
            return Err(RoomError::OperationBeforeCreation);
        }
        if self.expire_if_due(now) {
            return Err(RoomError::Expired);
        }
        Ok(())
    }
}
