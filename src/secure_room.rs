//! Authenticated, synchronous waiting-room and two-person lifecycle.
use crate::credentials::{InvitationSecret, ParticipantAccessCapability, RandomError, Verifier};
use crate::{CloseReason, Closure, ParticipantId, RoomId};
use std::time::Instant;

/// Secure room operation error; never contains credentials or verifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticipationError {
    /// The creator or participant identity and capability did not match.
    Unauthorized,
    /// Wrong, revoked, or consumed invitation capability.
    InvalidInvitation,
    /// The invitation reached its own deadline.
    InvitationExpired,
    /// The operation occurred before room creation.
    BeforeCreation,
    /// The room's expiration deadline is invalid.
    InvalidDeadline,
    /// The room is waiting for the invitee.
    NotActive,
    /// This action is not permitted in the current lifecycle state.
    InvalidState,
    /// The room was closed by a participant.
    AlreadyClosed,
    /// The room has expired.
    Expired,
    /// OS cryptographic randomness was unavailable; no operation was committed.
    RandomFailure,
}
impl std::fmt::Display for ParticipationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ParticipationError {}
impl From<RandomError> for ParticipationError {
    fn from(_: RandomError) -> Self {
        Self::RandomFailure
    }
}

/// Observable room lifecycle; closure is terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticipationState {
    /// Creator has a capability; the invitation has not been redeemed.
    WaitingForParticipant,
    /// Exactly two authorized participants are present.
    Active,
    /// Terminal state; original closure timestamp/reason are preserved.
    Closed(Closure),
}

/// Creator credentials and single-use invitation to be delivered separately.
pub struct CreatedRoomCredentials {
    /// Room-local creator identifier, which is not an access credential.
    pub creator_id: ParticipantId,
    /// Creator's bearer credential.
    pub creator_access: ParticipantAccessCapability,
    /// Invitation bearer secret, which should be sent only to the invitee.
    pub invitation: InvitationSecret,
}
impl std::fmt::Debug for CreatedRoomCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CreatedRoomCredentials([REDACTED])")
    }
}

/// Participant details returned exactly once after successfully joining.
pub struct JoinedParticipant {
    /// Newly issued room-local participant ID.
    pub id: ParticipantId,
    /// Newly issued room-scoped bearer credential.
    pub access: ParticipantAccessCapability,
}
impl std::fmt::Debug for JoinedParticipant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JoinedParticipant([REDACTED])")
    }
}

/// Bounded, authenticated participation and terminal-state engine.
///
/// This is a single-process domain object. The future server must serialize
/// redemption and closure per room; this type does not provide locks or a
/// transport authenticator. All times are trusted, process-local `Instant` values.
///
/// External consumers cannot create/mutate this domain type directly.
/// They must use the room manager. The trusted expiration mutator is private:
///
/// ```compile_fail
/// use ghostroom_core::SecureRoom;
/// use std::time::{Duration, Instant};
/// let now = Instant::now();
/// let _ = SecureRoom::expire_if_due;
/// ```
pub struct SecureRoom {
    id: RoomId,
    creator: ParticipantId,
    invitee: Option<ParticipantId>,
    created_at: Instant,
    expires_at: Option<Instant>,
    invitation_expires_at: Instant,
    invitation_verifier: Option<Verifier>,
    creator_verifier: Option<Verifier>,
    invitee_verifier: Option<Verifier>,
    state: ParticipationState,
}

impl std::fmt::Debug for SecureRoom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecureRoom([REDACTED])")
    }
}

impl SecureRoom {
    /// Create a waiting room; return newly generated secrets to its creator.
    ///
    /// Both room and invitation deadlines must be later than creation.
    /// The invitation must not outlive the room. There is no globally unique
    /// registry here: the existing RoomManager retries on colliding IDs.
    #[cfg(test)]
    pub(crate) fn create(
        created_at: Instant,
        expires_at: Option<Instant>,
        invitation_expires_at: Instant,
    ) -> Result<(Self, CreatedRoomCredentials), ParticipationError> {
        if expires_at.is_some_and(|t| t <= created_at)
            || invitation_expires_at <= created_at
            || expires_at.is_some_and(|t| invitation_expires_at > t)
        {
            return Err(ParticipationError::InvalidDeadline);
        }
        let id = RoomId::generate()?;
        Self::create_with_id(id, created_at, expires_at, invitation_expires_at)
    }

    /// Manager-only creation: the registry provides an ID to insert atomically.
    /// No caller may treat a generated ID as globally unique until insertion.
    pub(crate) fn create_with_id(
        id: RoomId,
        created_at: Instant,
        expires_at: Option<Instant>,
        invitation_expires_at: Instant,
    ) -> Result<(Self, CreatedRoomCredentials), ParticipationError> {
        if expires_at.is_some_and(|t| t <= created_at)
            || invitation_expires_at <= created_at
            || expires_at.is_some_and(|t| invitation_expires_at > t)
        {
            return Err(ParticipationError::InvalidDeadline);
        }
        let creator = ParticipantId::generate()?;
        let access = ParticipantAccessCapability::generate()?;
        let invitation = InvitationSecret::generate()?;
        let room = Self {
            id,
            creator,
            invitee: None,
            created_at,
            expires_at,
            invitation_expires_at,
            invitation_verifier: Some(Verifier::invitation(id, &invitation)),
            creator_verifier: Some(Verifier::participant(id, creator, &access)),
            invitee_verifier: None,
            state: ParticipationState::WaitingForParticipant,
        };
        Ok((
            room,
            CreatedRoomCredentials {
                creator_id: creator,
                creator_access: access,
                invitation,
            },
        ))
    }

    /// Return the public room identifier; it never authorizes any operation.
    #[must_use]
    pub const fn id(&self) -> RoomId {
        self.id
    }
    /// Return the room's creation instant.
    #[must_use]
    pub const fn created_at(&self) -> Instant {
        self.created_at
    }
    /// Return the optional room deadline.
    #[must_use]
    pub const fn expires_at(&self) -> Option<Instant> {
        self.expires_at
    }

    /// Only the trusted manager may shorten the active-phase deadline.
    /// It must call this with a deadline strictly after an authorized join.
    pub(crate) fn limit_active_until(&mut self, deadline: Instant) {
        self.expires_at = Some(match self.expires_at {
            Some(existing) => existing.min(deadline),
            None => deadline,
        });
    }
    /// Return the single-use invitation's deadline.
    #[must_use]
    pub const fn invitation_expires_at(&self) -> Instant {
        self.invitation_expires_at
    }
    /// Recorded state; this may lag logical expiration. Not authorization.
    #[must_use]
    pub const fn recorded_state(&self) -> ParticipationState {
        self.state
    }

    /// Logical state at a trusted time, without mutating stored state.
    /// This is an observation, not an authorization result.
    #[must_use]
    pub fn state_at(&self, now: Instant) -> ParticipationState {
        if let Some((at, _)) = self.due_expiration(now) {
            ParticipationState::Closed(Closure {
                at,
                reason: CloseReason::Expired,
            })
        } else {
            self.state
        }
    }

    /// Return the two participant IDs, if an invitee has joined.
    #[must_use]
    pub const fn participants(&self) -> (ParticipantId, Option<ParticipantId>) {
        (self.creator, self.invitee)
    }
    /// Return the original terminal metadata, if any.
    #[must_use]
    pub const fn closure(&self) -> Option<Closure> {
        match self.state {
            ParticipationState::Closed(closure) => Some(closure),
            _ => None,
        }
    }

    /// Redeem one valid, outstanding invitation. Invalid and consumed tokens
    /// receive the same error in every lifecycle state. Valid tokens are
    /// checked against the earliest waiting-room deadline before activation.
    pub(crate) fn join(
        &mut self,
        invitation: &InvitationSecret,
        now: Instant,
    ) -> Result<JoinedParticipant, ParticipationError> {
        // Check the invitation before revealing time or stored room state.
        let valid = self
            .invitation_verifier
            .as_ref()
            .is_some_and(|v| v.matches(&Verifier::invitation(self.id, invitation)));
        if !valid {
            return Err(ParticipationError::InvalidInvitation);
        }
        self.ensure_before_deadline(now)?;
        // All random generation precedes any mutation of the room.
        let mut generated = None;
        for _ in 0..3 {
            let candidate = ParticipantId::generate()?;
            if candidate != self.creator {
                generated = Some(candidate);
                break;
            }
        }
        let id = generated.ok_or(ParticipationError::RandomFailure)?;
        let access = ParticipantAccessCapability::generate()?;
        let verifier = Verifier::participant(self.id, id, &access);
        self.invitee = Some(id);
        self.invitee_verifier = Some(verifier);
        self.invitation_verifier = None;
        self.state = ParticipationState::Active;
        Ok(JoinedParticipant { id, access })
    }

    /// Authorize a short-lived, room-local lifecycle status observation.
    /// The returned state is data, never a reusable authorization token.
    pub(crate) fn authorized_state(
        &mut self,
        id: ParticipantId,
        capability: &ParticipantAccessCapability,
        now: Instant,
    ) -> Result<ParticipationState, ParticipationError> {
        self.require_authenticated(id, capability)?;
        self.ensure_before_deadline(now)?;
        Ok(self.state)
    }

    /// Authorize an active-room operation while enforcing trusted time.
    ///
    /// A future server must hold an exclusive per-room owner through the
    /// following protected operation; check-then-unlock-then-act is unsafe.
    pub(crate) fn authorize_active(
        &mut self,
        id: ParticipantId,
        capability: &ParticipantAccessCapability,
        now: Instant,
    ) -> Result<(), ParticipationError> {
        self.require_authenticated(id, capability)?;
        self.ensure_before_deadline(now)?;
        if self.state == ParticipationState::Active {
            Ok(())
        } else {
            Err(ParticipationError::NotActive)
        }
    }

    /// Cancel a waiting room using the creator's bearer credential.
    pub(crate) fn cancel(
        &mut self,
        id: ParticipantId,
        capability: &ParticipantAccessCapability,
        now: Instant,
    ) -> Result<(), ParticipationError> {
        self.require_authenticated(id, capability)?;
        if id != self.creator {
            return Err(ParticipationError::Unauthorized);
        }
        self.ensure_before_deadline(now)?;
        if self.state != ParticipationState::WaitingForParticipant {
            return Err(ParticipationError::InvalidState);
        }
        self.terminate(now, CloseReason::Participant(id));
        Ok(())
    }

    /// Close an active room using either participant's bearer credential.
    pub(crate) fn close(
        &mut self,
        id: ParticipantId,
        capability: &ParticipantAccessCapability,
        now: Instant,
    ) -> Result<(), ParticipationError> {
        self.authorize_active(id, capability, now)?;
        self.terminate(now, CloseReason::Participant(id));
        Ok(())
    }

    /// Crate-internal maintenance operation. Waiting rooms expire at their
    /// invitation deadline, even without a room deadline.
    ///
    /// Not callable by downstream crates or future remote request handlers.
    /// An in-crate room manager must supply trusted time and serialize room
    /// ownership before invoking maintenance.
    pub(crate) fn expire_if_due(&mut self, now: Instant) -> bool {
        if let Some((at, _)) = self.due_expiration(now) {
            self.terminate(at, CloseReason::Expired);
            true
        } else {
            false
        }
    }

    fn credentials_match(
        &self,
        id: ParticipantId,
        capability: &ParticipantAccessCapability,
    ) -> bool {
        let verifier = if id == self.creator {
            self.creator_verifier.as_ref()
        } else if Some(id) == self.invitee {
            self.invitee_verifier.as_ref()
        } else {
            None
        };
        verifier
            .is_some_and(|stored| stored.matches(&Verifier::participant(self.id, id, capability)))
    }

    fn require_authenticated(
        &self,
        id: ParticipantId,
        capability: &ParticipantAccessCapability,
    ) -> Result<(), ParticipationError> {
        if self.credentials_match(id, capability) {
            Ok(())
        } else {
            Err(ParticipationError::Unauthorized)
        }
    }

    // Earliest deadline wins; room deadline wins ties.
    // The invitation TTL only applies before successful redemption.
    fn due_expiration(&self, now: Instant) -> Option<(Instant, ParticipationError)> {
        if matches!(self.state, ParticipationState::Closed(_)) {
            return None;
        }
        let invite = if self.state == ParticipationState::WaitingForParticipant {
            Some(self.invitation_expires_at)
        } else {
            None
        };
        let (at, error) = match (self.expires_at, invite) {
            (Some(room), Some(invitation)) if invitation < room => {
                (invitation, ParticipationError::InvitationExpired)
            }
            (Some(room), _) => (room, ParticipationError::Expired),
            (None, Some(invitation)) => (invitation, ParticipationError::InvitationExpired),
            (None, None) => return None,
        };
        (now >= at).then_some((at, error))
    }

    fn ensure_before_deadline(&mut self, now: Instant) -> Result<(), ParticipationError> {
        if let ParticipationState::Closed(c) = self.state {
            return Err(Self::terminal_error(c));
        }
        if now < self.created_at {
            return Err(ParticipationError::BeforeCreation);
        }
        if let Some((_, error)) = self.due_expiration(now) {
            // Keep the trusted maintenance transition inside this crate.
            // The future room manager must own it exclusively.
            let did_expire = self.expire_if_due(now);
            debug_assert!(did_expire);
            return Err(error);
        }
        Ok(())
    }

    fn terminal_error(closure: Closure) -> ParticipationError {
        match closure.reason {
            CloseReason::Expired => ParticipationError::Expired,
            CloseReason::Participant(_) => ParticipationError::AlreadyClosed,
        }
    }

    fn terminate(&mut self, at: Instant, reason: CloseReason) {
        self.state = ParticipationState::Closed(Closure { at, reason });
        // Dropping verifiers revokes all bearer credentials at this boundary.
        self.invitation_verifier = None;
        self.creator_verifier = None;
        self.invitee_verifier = None;
    }
}

#[cfg(test)]
mod maintenance_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn private_poll_uses_invitation_deadline_without_room_ttl() {
        let start = Instant::now();
        let invite_deadline = start + Duration::from_secs(5);
        let (mut room, issued) = SecureRoom::create(start, None, invite_deadline).unwrap();
        assert!(!room.expire_if_due(start + Duration::from_secs(4)));
        assert_eq!(
            room.recorded_state(),
            ParticipationState::WaitingForParticipant
        );
        assert!(room.expire_if_due(start + Duration::from_secs(9)));
        assert_eq!(
            room.closure(),
            Some(Closure {
                at: invite_deadline,
                reason: CloseReason::Expired,
            })
        );
        assert!(!room.expire_if_due(start + Duration::from_secs(12)));
        assert_eq!(
            room.join(&issued.invitation, start + Duration::from_secs(12))
                .err(),
            Some(ParticipationError::InvalidInvitation)
        );
    }

    #[test]
    fn private_poll_preserves_first_manual_closure() {
        let start = Instant::now();
        let (mut room, issued) = SecureRoom::create(
            start,
            Some(start + Duration::from_secs(30)),
            start + Duration::from_secs(10),
        )
        .unwrap();
        room.cancel(
            issued.creator_id,
            &issued.creator_access,
            start + Duration::from_secs(2),
        )
        .unwrap();
        let first = room.closure();
        assert!(!room.expire_if_due(start + Duration::from_secs(100)));
        assert_eq!(room.closure(), first);
    }
}
