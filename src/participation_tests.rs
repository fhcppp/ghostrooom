//! Internal Step 2 domain regressions. Only RoomManager is application-facing.
use crate::{
    CloseReason, InvitationSecret, ParticipantAccessCapability, ParticipantId, ParticipationError,
    ParticipationState, RoomId, SecureRoom,
};
use std::time::{Duration, Instant};
fn at(t: Instant, n: u64) -> Instant {
    t + Duration::from_secs(n)
}
fn create(
    t: Instant,
    room: Option<u64>,
    invite: u64,
) -> (SecureRoom, crate::CreatedRoomCredentials) {
    SecureRoom::create(t, room.map(|d| at(t, d)), at(t, invite)).expect("valid room")
}
#[test]
fn initial_room_waits_for_participant() {
    let t = Instant::now();
    let (r, c) = create(t, Some(100), 50);
    assert_eq!(
        r.recorded_state(),
        ParticipationState::WaitingForParticipant
    );
    assert_eq!(r.participants(), (c.creator_id, None));
    assert!(r.closure().is_none());
}
#[test]
fn independent_room_creation_does_not_reuse_room_ids() {
    let start = Instant::now();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..16 {
        let (room, _) = create(start, None, 10);
        assert!(
            seen.insert(room.id()),
            "room identifier unexpectedly repeated"
        );
    }
    // This is a probabilistic RNG sanity check, not proof of global uniqueness.
    // Deterministic collisions must be tested in the future room registry.
}
#[test]
fn generated_identifiers_have_fixed_representations() {
    let _ = RoomId::generate().expect("OS entropy");
    let _ = ParticipantId::generate().expect("OS entropy");
    assert_eq!(std::mem::size_of::<RoomId>(), 16);
    assert_eq!(std::mem::size_of::<ParticipantId>(), 16);
}
#[test]
fn invalid_deadlines_are_rejected() {
    let t = Instant::now();
    for (room, invite) in [(Some(10), 11), (Some(10), 12), (None, 0), (Some(0), 1)] {
        assert!(matches!(
            SecureRoom::create(t, room.map(|n| at(t, n)), at(t, invite)),
            Err(ParticipationError::InvalidDeadline)
        ));
    }
}
#[test]
fn creator_credentials_are_accepted_but_id_alone_is_not() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(100), 50);
    assert_eq!(
        r.authorize_active(c.creator_id, &c.creator_access, at(t, 1)),
        Err(ParticipationError::NotActive)
    );
    let fake = ParticipantAccessCapability::from_bytes([9; 32]);
    assert_eq!(
        r.authorize_active(c.creator_id, &fake, at(t, 1)),
        Err(ParticipationError::Unauthorized)
    );
    assert_eq!(
        r.cancel(c.creator_id, &fake, at(t, 1)),
        Err(ParticipationError::Unauthorized)
    );
    assert_eq!(
        r.recorded_state(),
        ParticipationState::WaitingForParticipant
    );
}
#[test]
fn invitation_redemption_is_single_use_and_activates() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(100), 50);
    let bob = r.join(&c.invitation, at(t, 1)).expect("valid invitation");
    assert_ne!(bob.id, c.creator_id);
    assert_eq!(r.recorded_state(), ParticipationState::Active);
    assert_eq!(r.participants().1, Some(bob.id));
    assert_eq!(r.authorize_active(bob.id, &bob.access, at(t, 2)), Ok(()));
    assert_eq!(
        r.join(&c.invitation, at(t, 2)).err(),
        Some(ParticipationError::InvalidInvitation)
    );
    assert_eq!(r.participants().1, Some(bob.id));
}
#[test]
fn bad_invitation_does_not_mutate() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(100), 50);
    let bad = InvitationSecret::from_bytes([3; 32]);
    assert_eq!(
        r.join(&bad, at(t, 2)).err(),
        Some(ParticipationError::InvalidInvitation)
    );
    assert_eq!(
        r.recorded_state(),
        ParticipationState::WaitingForParticipant
    );
    assert!(r.join(&c.invitation, at(t, 3)).is_ok());
}
#[test]
fn deadline_precedes_authenticated_join() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(10), 10);
    assert_eq!(
        r.join(&c.invitation, at(t, 10)).err(),
        Some(ParticipationError::Expired)
    );
    assert_eq!(r.closure().unwrap().at, at(t, 10));
    assert_eq!(r.closure().unwrap().reason, CloseReason::Expired);
}
#[test]
fn late_join_records_original_expiry_instant() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(10), 9);
    assert_eq!(
        r.join(&c.invitation, at(t, 100)).err(),
        Some(ParticipationError::InvitationExpired)
    );
    assert_eq!(r.closure().unwrap().at, at(t, 9));
}
#[test]
fn invitation_expires_independently_and_cannot_redeem_again() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(100), 10);
    assert_eq!(
        r.join(&c.invitation, at(t, 10)).err(),
        Some(ParticipationError::InvitationExpired)
    );
    assert_eq!(
        r.recorded_state(),
        ParticipationState::Closed(crate::Closure {
            at: at(t, 10),
            reason: CloseReason::Expired
        })
    );
    assert_eq!(
        r.join(&c.invitation, at(t, 11)).err(),
        Some(ParticipationError::InvalidInvitation)
    );
}
#[test]
fn a_valid_creator_can_cancel_waiting_room() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(100), 10);
    assert_eq!(r.cancel(c.creator_id, &c.creator_access, at(t, 2)), Ok(()));
    assert_eq!(
        r.closure().unwrap().reason,
        CloseReason::Participant(c.creator_id)
    );
    assert_eq!(
        r.authorize_active(c.creator_id, &c.creator_access, at(t, 3)),
        Err(ParticipationError::Unauthorized)
    );
    assert_eq!(
        r.join(&c.invitation, at(t, 3)).err(),
        Some(ParticipationError::InvalidInvitation)
    );
}
#[test]
fn cancelling_after_join_is_forbidden() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(100), 10);
    let _ = r.join(&c.invitation, at(t, 1)).unwrap();
    assert_eq!(
        r.cancel(c.creator_id, &c.creator_access, at(t, 2)),
        Err(ParticipationError::InvalidState)
    );
    assert_eq!(r.recorded_state(), ParticipationState::Active);
}
#[test]
fn both_participants_can_close_an_active_room() {
    for creator_closes in [true, false] {
        let t = Instant::now();
        let (mut r, c) = create(t, None, 10);
        let bob = r.join(&c.invitation, at(t, 1)).unwrap();
        let (id, cap) = if creator_closes {
            (c.creator_id, &c.creator_access)
        } else {
            (bob.id, &bob.access)
        };
        assert_eq!(r.close(id, cap, at(t, 2)), Ok(()));
        assert_eq!(r.closure().unwrap().reason, CloseReason::Participant(id));
        assert_eq!(
            r.authorize_active(c.creator_id, &c.creator_access, at(t, 3)),
            Err(ParticipationError::Unauthorized)
        );
        assert_eq!(
            r.authorize_active(bob.id, &bob.access, at(t, 3)),
            Err(ParticipationError::Unauthorized)
        );
    }
}
#[test]
fn cannot_close_waiting_room_as_active() {
    let t = Instant::now();
    let (mut r, c) = create(t, None, 10);
    assert_eq!(
        r.close(c.creator_id, &c.creator_access, at(t, 1)),
        Err(ParticipationError::NotActive)
    );
    assert_eq!(
        r.recorded_state(),
        ParticipationState::WaitingForParticipant
    );
}
#[test]
fn unrelated_participant_cannot_mutate_on_expired_room() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(10), 10);
    let attacker = ParticipantAccessCapability::from_bytes([7; 32]);
    assert_eq!(
        r.close(ParticipantId::from_u128(55), &attacker, at(t, 20)),
        Err(ParticipationError::Unauthorized)
    );
    assert_eq!(
        r.recorded_state(),
        ParticipationState::WaitingForParticipant
    );
    assert_eq!(
        r.cancel(c.creator_id, &c.creator_access, at(t, 20)),
        Err(ParticipationError::Expired)
    );
    assert_eq!(r.closure().unwrap().at, at(t, 10));
}
#[test]
fn valid_credentials_are_bound_to_their_own_room() {
    let t = Instant::now();
    let (mut a, ca) = create(t, None, 10);
    let (mut b, cb) = create(t, None, 10);
    assert_eq!(
        b.cancel(ca.creator_id, &ca.creator_access, at(t, 1)),
        Err(ParticipationError::Unauthorized)
    );
    assert_eq!(
        b.join(&ca.invitation, at(t, 1)).err(),
        Some(ParticipationError::InvalidInvitation)
    );
    assert_eq!(
        b.recorded_state(),
        ParticipationState::WaitingForParticipant
    );
    assert_eq!(
        a.cancel(ca.creator_id, &ca.creator_access, at(t, 1)),
        Ok(())
    );
    assert_eq!(
        b.cancel(cb.creator_id, &cb.creator_access, at(t, 2)),
        Ok(())
    );
}
#[test]
fn wrong_capability_for_correct_id_rejected() {
    let t = Instant::now();
    let (mut r, c) = create(t, None, 10);
    let bob = r.join(&c.invitation, at(t, 1)).unwrap();
    assert_eq!(
        r.close(bob.id, &c.creator_access, at(t, 2)),
        Err(ParticipationError::Unauthorized)
    );
    assert_eq!(
        r.authorize_active(c.creator_id, &bob.access, at(t, 2)),
        Err(ParticipationError::Unauthorized)
    );
    assert_eq!(r.recorded_state(), ParticipationState::Active);
}
#[test]
fn waiting_room_never_grants_active_access() {
    let t = Instant::now();
    let (mut r, c) = create(t, None, 10);
    assert_eq!(
        r.authorize_active(c.creator_id, &c.creator_access, at(t, 1)),
        Err(ParticipationError::NotActive)
    );
}
#[test]
fn time_before_creation_fails_without_mutation() {
    let t = Instant::now();
    let (mut r, c) = create(t, None, 10);
    let prior = t - Duration::from_nanos(1);
    assert_eq!(
        r.join(&c.invitation, prior).err(),
        Some(ParticipationError::BeforeCreation)
    );
    assert_eq!(
        r.cancel(c.creator_id, &c.creator_access, prior),
        Err(ParticipationError::BeforeCreation)
    );
    // The maintenance-only expiry mutator is not part of the public API.
    assert_eq!(
        r.recorded_state(),
        ParticipationState::WaitingForParticipant
    );
}
#[test]
fn internal_expiry_is_terminal_and_idempotent() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(10), 10);
    assert_eq!(
        r.authorize_active(c.creator_id, &c.creator_access, at(t, 9)),
        Err(ParticipationError::NotActive)
    );
    assert_eq!(
        r.cancel(c.creator_id, &c.creator_access, at(t, 10)),
        Err(ParticipationError::Expired)
    );
    let closure = r.closure();
    assert_eq!(
        r.cancel(c.creator_id, &c.creator_access, at(t, 20)),
        Err(ParticipationError::Unauthorized)
    );
    assert_eq!(r.closure(), closure);
    assert_eq!(
        r.join(&c.invitation, at(t, 20)).err(),
        Some(ParticipationError::InvalidInvitation)
    );
}
#[test]
fn closure_never_overwrites_original_time() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(10), 9);
    let bob = r.join(&c.invitation, at(t, 1)).unwrap();
    r.close(bob.id, &bob.access, at(t, 2)).unwrap();
    let first = r.closure();
    // No public maintenance entrypoint can override this closure.
    assert_eq!(
        r.close(c.creator_id, &c.creator_access, at(t, 101)),
        Err(ParticipationError::Unauthorized)
    );
    assert_eq!(r.closure(), first);
}
#[test]
fn credentials_have_redacted_debug_format() {
    let t = Instant::now();
    let (mut r, c) = create(t, None, 10);
    let invite = format!("{:?}", c.invitation);
    let cap = format!("{:?}", c.creator_access);
    assert!(invite.contains("REDACTED"));
    assert!(cap.contains("REDACTED"));
    let joined = r.join(&c.invitation, at(t, 1)).unwrap();
    assert!(format!("{joined:?}").contains("REDACTED"));
    assert!(format!("{r:?}").contains("REDACTED"));
}
#[test]
fn malformed_raw_lengths_are_not_accepted_by_typed_secret_api() {
    // from_bytes accepts only [u8; 32], so malformed raw lengths cannot be
    // represented at its callsite. Link/QR encoding is intentionally deferred.
    let valid = InvitationSecret::from_bytes([8; 32]);
    assert_eq!(valid.as_bytes().len(), 32);
}

#[test]
fn active_authorization_checks_deadlines_without_prior_poll() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(20), 10);
    let bob = r.join(&c.invitation, at(t, 1)).unwrap();
    assert_eq!(r.authorize_active(bob.id, &bob.access, at(t, 19)), Ok(()));
    assert_eq!(r.recorded_state(), ParticipationState::Active);
    assert_eq!(
        r.authorize_active(bob.id, &bob.access, at(t, 20)),
        Err(ParticipationError::Expired)
    );
    assert_eq!(r.closure().unwrap().at, at(t, 20));
    assert_eq!(
        r.authorize_active(bob.id, &bob.access, at(t, 21)),
        Err(ParticipationError::Unauthorized)
    );
}

#[test]
fn stale_recorded_state_does_not_imply_live_room() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(20), 10);
    let _ = r.join(&c.invitation, at(t, 1)).unwrap();
    assert_eq!(r.recorded_state(), ParticipationState::Active);
    assert_eq!(
        r.state_at(at(t, 20)),
        ParticipationState::Closed(crate::Closure {
            at: at(t, 20),
            reason: CloseReason::Expired
        })
    );
    assert_eq!(r.recorded_state(), ParticipationState::Active);
}

#[test]
fn invitation_expiry_closes_waiting_room_without_room_deadline() {
    let t = Instant::now();
    let (mut r, c) = create(t, None, 10);
    assert_eq!(
        r.state_at(at(t, 10)),
        ParticipationState::Closed(crate::Closure {
            at: at(t, 10),
            reason: CloseReason::Expired
        })
    );
    assert_eq!(
        r.join(&c.invitation, at(t, 10)).err(),
        Some(ParticipationError::InvitationExpired)
    );
    assert_eq!(r.closure().unwrap().at, at(t, 10));
    assert_eq!(
        r.cancel(c.creator_id, &c.creator_access, at(t, 11)),
        Err(ParticipationError::Unauthorized)
    );
    assert_eq!(
        r.join(&c.invitation, at(t, 11)).err(),
        Some(ParticipationError::InvalidInvitation)
    );
}

#[test]
fn waiting_expiry_poll_is_idempotent_and_revokes_credentials() {
    let t = Instant::now();
    let (mut r, c) = create(t, None, 5);
    assert_eq!(
        r.authorize_active(c.creator_id, &c.creator_access, at(t, 4)),
        Err(ParticipationError::NotActive)
    );
    assert_eq!(
        r.cancel(c.creator_id, &c.creator_access, at(t, 10)),
        Err(ParticipationError::InvitationExpired)
    );
    let first = r.closure();
    assert_eq!(first.unwrap().at, at(t, 5));
    // A repeated authorized request cannot mutate a terminal room.
    assert_eq!(r.closure(), first);
    assert_eq!(
        r.cancel(c.creator_id, &c.creator_access, at(t, 20)),
        Err(ParticipationError::Unauthorized)
    );
}

#[test]
fn invalid_invitation_has_identical_error_in_every_state() {
    let t = Instant::now();
    let bad = InvitationSecret::from_bytes([99; 32]);
    let (mut waiting, _) = create(t, None, 10);
    let (mut active, c) = create(t, None, 10);
    let _ = active.join(&c.invitation, at(t, 1)).unwrap();
    let (mut cancelled, d) = create(t, None, 10);
    cancelled
        .cancel(d.creator_id, &d.creator_access, at(t, 1))
        .unwrap();
    let (mut expired, expired_credentials) = create(t, None, 10);
    assert_eq!(
        expired
            .join(&expired_credentials.invitation, at(t, 10))
            .err(),
        Some(ParticipationError::InvitationExpired)
    );
    for room in [&mut waiting, &mut active, &mut cancelled, &mut expired] {
        let before = room.recorded_state();
        assert_eq!(
            room.join(&bad, at(t, 40)).err(),
            Some(ParticipationError::InvalidInvitation)
        );
        assert_eq!(room.recorded_state(), before);
    }
    assert_eq!(
        active.join(&c.invitation, at(t, 2)).err(),
        Some(ParticipationError::InvalidInvitation)
    );
}

#[test]
fn precreation_invalid_invite_is_generic_and_does_not_mutate() {
    let t = Instant::now();
    let (mut r, c) = create(t, None, 10);
    let bad = InvitationSecret::from_bytes([99; 32]);
    let prior = t - Duration::from_secs(1);
    assert_eq!(
        r.join(&bad, prior).err(),
        Some(ParticipationError::InvalidInvitation)
    );
    assert_eq!(
        r.join(&c.invitation, prior).err(),
        Some(ParticipationError::BeforeCreation)
    );
    assert_eq!(
        r.recorded_state(),
        ParticipationState::WaitingForParticipant
    );
}

#[test]
fn joining_before_invite_deadline_keeps_active_room_after_invite_deadline() {
    let t = Instant::now();
    let (mut r, c) = create(t, Some(100), 5);
    let bob = r.join(&c.invitation, at(t, 1)).unwrap();
    assert_eq!(r.authorize_active(bob.id, &bob.access, at(t, 50)), Ok(()));
    assert_eq!(r.state_at(at(t, 50)), ParticipationState::Active);
}

#[test]
fn sensitive_debug_output_omits_room_timestamps_and_ids() {
    let t = Instant::now();
    let (mut r, c) = create(t, None, 10);
    let debug = format!("{r:?}");
    assert_eq!(debug, "SecureRoom([REDACTED])");
    assert_eq!(format!("{c:?}"), "CreatedRoomCredentials([REDACTED])");
    let bob = r.join(&c.invitation, at(t, 1)).unwrap();
    assert_eq!(format!("{bob:?}"), "JoinedParticipant([REDACTED])");
}
