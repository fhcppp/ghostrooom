use std::time::{Duration, Instant};

use ghostroom_core::legacy::{Room, RoomError, RoomState};
use ghostroom_core::{CloseReason, Closure, ParticipantId, RoomId};

const FIRST: ParticipantId = ParticipantId::from_u128(11);
const SECOND: ParticipantId = ParticipantId::from_u128(22);
const OUTSIDER: ParticipantId = ParticipantId::from_u128(33);
const ROOM_ID: RoomId = RoomId::from_u128(0xDEAD_BEEF);

fn at(base: Instant, seconds: u64) -> Instant {
    base + Duration::from_secs(seconds)
}

fn room(base: Instant, expiration: Option<u64>) -> Room {
    Room::new(
        ROOM_ID,
        [FIRST, SECOND],
        base,
        expiration.map(|s| at(base, s)),
    )
    .expect("valid test fixture")
}

#[test]
fn new_room_is_active() {
    let base = Instant::now();
    let r = room(base, None);
    assert_eq!(r.id(), ROOM_ID);
    assert_eq!(r.id().as_u128(), 0xDEAD_BEEF);
    assert_eq!(r.created_at(), base);
    assert_eq!(r.expires_at(), None);
    assert_eq!(r.state(), RoomState::Active);
    assert_eq!(r.closure(), None);
    assert!(r.is_active_at(base));
}

#[test]
fn exactly_two_distinct_participants() {
    let r = room(Instant::now(), None);
    assert_eq!(r.participants(), [FIRST, SECOND]);
    assert_ne!(r.participants()[0], r.participants()[1]);
    assert_eq!(FIRST.as_u128(), 11);
    assert!(r.contains(FIRST));
    assert!(r.contains(SECOND));
    assert!(!r.contains(OUTSIDER));
}

#[test]
fn duplicate_participants_are_rejected() {
    let result = Room::new(ROOM_ID, [FIRST, FIRST], Instant::now(), None);
    assert!(matches!(result, Err(RoomError::DuplicateParticipants)));
}

#[test]
fn room_without_deadline_remains_active_later() {
    let base = Instant::now();
    let mut r = room(base, None);
    assert!(r.is_active_at(at(base, 100_000)));
    assert_eq!(r.require_active(FIRST, at(base, 100_000)), Ok(()));
    assert_eq!(r.state(), RoomState::Active);
    assert!(!r.expire_if_due(at(base, 100_000)));
}

#[test]
fn valid_future_deadline_is_accepted() {
    let base = Instant::now();
    let r = room(base, Some(60));
    assert_eq!(r.expires_at(), Some(at(base, 60)));
    assert!(r.is_active_at(base));
}

#[test]
fn deadline_equal_to_creation_is_rejected() {
    let base = Instant::now();
    let result = Room::new(ROOM_ID, [FIRST, SECOND], base, Some(base));
    assert!(matches!(result, Err(RoomError::InvalidExpirationDeadline)));
}

#[test]
fn deadline_before_creation_is_rejected() {
    let base = Instant::now();
    let result = Room::new(
        ROOM_ID,
        [FIRST, SECOND],
        base,
        Some(base - Duration::from_secs(1)),
    );
    assert!(matches!(result, Err(RoomError::InvalidExpirationDeadline)));
}

#[test]
fn first_participant_can_close() {
    let base = Instant::now();
    let mut r = room(base, None);
    assert_eq!(r.close(FIRST, at(base, 1)), Ok(()));
    assert_eq!(
        r.closure(),
        Some(Closure {
            at: at(base, 1),
            reason: CloseReason::Participant(FIRST),
        })
    );
    assert!(!r.is_active_at(at(base, 1)));
}

#[test]
fn second_participant_can_close() {
    let base = Instant::now();
    let mut r = room(base, None);
    assert_eq!(r.close(SECOND, at(base, 2)), Ok(()));
    assert_eq!(
        r.closure().map(|closure| closure.reason),
        Some(CloseReason::Participant(SECOND))
    );
}

#[test]
fn outsider_cannot_close_room() {
    let base = Instant::now();
    let mut r = room(base, None);
    assert_eq!(
        r.close(OUTSIDER, at(base, 2)),
        Err(RoomError::UnauthorizedParticipant)
    );
    assert_eq!(r.state(), RoomState::Active);
}

#[test]
fn unauthorized_operation_cannot_trigger_expiration() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert_eq!(
        r.require_active(OUTSIDER, at(base, 11)),
        Err(RoomError::UnauthorizedParticipant)
    );
    assert_eq!(
        r.close(OUTSIDER, at(base, 12)),
        Err(RoomError::UnauthorizedParticipant)
    );
    assert_eq!(r.state(), RoomState::Active);
    assert!(!r.is_active_at(at(base, 11)));
}

#[test]
fn room_is_active_immediately_before_deadline() {
    let base = Instant::now();
    let mut r = room(base, Some(60));
    let almost = at(base, 60) - Duration::from_nanos(1);
    assert!(r.is_active_at(almost));
    assert_eq!(r.require_active(FIRST, almost), Ok(()));
    assert_eq!(r.state(), RoomState::Active);
}

#[test]
fn expiration_at_exact_deadline() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert!(!r.is_active_at(at(base, 10)));
    assert_eq!(
        r.require_active(FIRST, at(base, 10)),
        Err(RoomError::Expired)
    );
    assert_eq!(
        r.closure(),
        Some(Closure {
            at: at(base, 10),
            reason: CloseReason::Expired,
        })
    );
}

#[test]
fn late_expiration_records_configured_deadline_not_discovery() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert!(r.expire_if_due(at(base, 999)));
    assert_eq!(r.closure().map(|c| c.at), Some(at(base, 10)));
}

#[test]
fn expired_room_rejects_active_operations() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert_eq!(
        r.require_active(SECOND, at(base, 11)),
        Err(RoomError::Expired)
    );
    assert_eq!(r.close(FIRST, at(base, 12)), Err(RoomError::Expired));
    assert_eq!(
        r.require_active(FIRST, at(base, 13)),
        Err(RoomError::Expired)
    );
}

#[test]
fn closed_room_never_reopens() {
    let base = Instant::now();
    let mut r = room(base, None);
    r.close(FIRST, at(base, 1)).expect("valid first closure");
    assert_eq!(
        r.require_active(SECOND, at(base, 2)),
        Err(RoomError::AlreadyClosed)
    );
    assert!(!r.is_active_at(at(base, 3)));
    assert!(matches!(r.state(), RoomState::Closed(_)));
}

#[test]
fn repeated_closure_preserves_original_metadata() {
    let base = Instant::now();
    let mut r = room(base, None);
    r.close(FIRST, at(base, 1)).expect("valid first closure");
    let original = r.closure();
    assert_eq!(r.close(SECOND, at(base, 2)), Err(RoomError::AlreadyClosed));
    assert_eq!(r.closure(), original);
}

#[test]
fn expiration_wins_over_close_at_exact_deadline() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert_eq!(r.close(FIRST, at(base, 10)), Err(RoomError::Expired));
    assert_eq!(
        r.closure(),
        Some(Closure {
            at: at(base, 10),
            reason: CloseReason::Expired,
        })
    );
}

#[test]
fn expiration_wins_when_close_arrives_late() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert_eq!(r.close(SECOND, at(base, 500)), Err(RoomError::Expired));
    assert_eq!(r.closure().map(|c| c.at), Some(at(base, 10)));
}

#[test]
fn repeated_expiration_poll_is_idempotent() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert!(!r.expire_if_due(at(base, 9)));
    assert!(r.expire_if_due(at(base, 10)));
    let original = r.state();
    assert!(!r.expire_if_due(at(base, 100)));
    assert!(!r.expire_if_due(at(base, 10)));
    assert_eq!(r.state(), original);
}

#[test]
fn closure_before_deadline_prevents_later_expiration() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    r.close(SECOND, at(base, 9)).expect("valid closure");
    let original = r.closure();
    assert!(!r.expire_if_due(at(base, 99)));
    assert_eq!(
        r.require_active(FIRST, at(base, 99)),
        Err(RoomError::AlreadyClosed)
    );
    assert_eq!(r.closure(), original);
}

#[test]
fn invalid_operations_cannot_change_closed_room() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    r.close(FIRST, at(base, 1)).expect("valid closure");
    let original = r.state();
    for second in [2, 3, 100, 10_000] {
        assert_eq!(
            r.require_active(OUTSIDER, at(base, second)),
            Err(RoomError::UnauthorizedParticipant)
        );
        assert_eq!(
            r.close(SECOND, at(base, second)),
            Err(RoomError::AlreadyClosed)
        );
        assert!(!r.expire_if_due(at(base, second)));
        assert_eq!(r.state(), original);
    }
}

#[test]
fn operations_before_creation_are_rejected_without_mutation() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    let earlier = base - Duration::from_nanos(1);
    assert!(!r.is_active_at(earlier));
    assert_eq!(
        r.require_active(FIRST, earlier),
        Err(RoomError::OperationBeforeCreation)
    );
    assert_eq!(
        r.close(FIRST, earlier),
        Err(RoomError::OperationBeforeCreation)
    );
    assert!(!r.expire_if_due(earlier));
    assert_eq!(r.state(), RoomState::Active);
}

#[test]
fn unauthorized_attempt_on_expired_room_still_cannot_mutate() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert_eq!(
        r.close(OUTSIDER, at(base, 20)),
        Err(RoomError::UnauthorizedParticipant)
    );
    assert_eq!(r.state(), RoomState::Active);
    assert_eq!(r.close(SECOND, at(base, 20)), Err(RoomError::Expired));
    let original = r.closure();
    assert_eq!(
        r.close(OUTSIDER, at(base, 30)),
        Err(RoomError::UnauthorizedParticipant)
    );
    assert_eq!(r.closure(), original);
}

#[test]
fn deterministic_sequence_preserves_terminal_invariants() {
    let base = Instant::now();
    let mut cases = [room(base, Some(4)), room(base, Some(9)), room(base, None)];

    assert_eq!(cases[0].require_active(FIRST, at(base, 1)), Ok(()));
    assert_eq!(cases[0].close(SECOND, at(base, 4)), Err(RoomError::Expired));
    assert_eq!(cases[1].close(FIRST, at(base, 7)), Ok(()));
    assert_eq!(cases[2].require_active(SECOND, at(base, 8)), Ok(()));
    assert_eq!(cases[2].close(SECOND, at(base, 9)), Ok(()));

    let original: [_; 3] = cases.each_ref().map(Room::closure);
    for r in &mut cases {
        assert!(!r.expire_if_due(at(base, 100)));
        assert!(r.require_active(OUTSIDER, at(base, 100)).is_err());
        assert!(r.close(FIRST, at(base, 101)).is_err());
        assert!(matches!(r.state(), RoomState::Closed(_)));
    }
    assert_eq!(cases.each_ref().map(Room::closure), original);
    assert_eq!(original[0].map(|c| c.reason), Some(CloseReason::Expired));
    assert_eq!(
        original[1].map(|c| c.reason),
        Some(CloseReason::Participant(FIRST))
    );
    assert_eq!(
        original[2].map(|c| c.reason),
        Some(CloseReason::Participant(SECOND))
    );
}

#[test]
fn domain_errors_have_human_readable_messages() {
    for err in [
        RoomError::DuplicateParticipants,
        RoomError::InvalidExpirationDeadline,
        RoomError::UnauthorizedParticipant,
        RoomError::AlreadyClosed,
        RoomError::Expired,
        RoomError::OperationBeforeCreation,
    ] {
        assert!(!err.to_string().is_empty());
    }
}

#[test]
fn authorized_operation_at_creation_instant_is_valid() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert_eq!(r.require_active(FIRST, base), Ok(()));
    assert_eq!(r.close(SECOND, base), Ok(()));
    assert_eq!(
        r.closure(),
        Some(Closure {
            at: base,
            reason: CloseReason::Participant(SECOND),
        })
    );
}

#[test]
fn logical_expiration_query_does_not_modify_recorded_state() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert!(!r.is_active_at(at(base, 11)));
    assert_eq!(r.state(), RoomState::Active);
    assert_eq!(r.closure(), None);
    assert_eq!(
        r.require_active(FIRST, at(base, 11)),
        Err(RoomError::Expired)
    );
    assert_eq!(r.closure().map(|closure| closure.at), Some(at(base, 10)));
}

#[test]
fn participant_closed_room_never_becomes_active_for_older_times() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    assert_eq!(r.close(FIRST, at(base, 4)), Ok(()));
    assert!(!r.is_active_at(at(base, 3)));
    assert!(!r.is_active_at(base));
    assert_eq!(
        r.require_active(SECOND, base),
        Err(RoomError::AlreadyClosed)
    );
    assert_eq!(
        r.closure().map(|closure| closure.reason),
        Some(CloseReason::Participant(FIRST))
    );
}

#[test]
fn expiry_checks_before_creation_never_change_state() {
    let base = Instant::now();
    let mut r = room(base, Some(10));
    let earlier = base - Duration::from_nanos(1);
    assert!(!r.expire_if_due(earlier));
    assert!(!r.is_active_at(earlier));
    assert_eq!(r.state(), RoomState::Active);
    assert_eq!(r.closure(), None);
}
