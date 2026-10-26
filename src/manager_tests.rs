use crate::manager::RoomManager;
use crate::*;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

fn cfg() -> RoomManagerConfig {
    RoomManagerConfig {
        max_rooms: 3,
        max_waiting_lifetime: Duration::from_secs(30),
        max_active_lifetime: Duration::from_secs(20),
        max_invitation_lifetime: Duration::from_secs(10),
        max_cleanup_per_pass: 1,
        max_id_retries: 3,
    }
}
struct Harness {
    secs: Arc<AtomicU64>,
    manager: Arc<RoomManager>,
}
impl Harness {
    fn new(config: RoomManagerConfig) -> Self {
        let secs = Arc::new(AtomicU64::new(0));
        let clock_secs = Arc::clone(&secs);
        let start = Instant::now();
        let manager = RoomManager::with_sources(
            config,
            Arc::new(move || start + Duration::from_secs(clock_secs.load(Ordering::SeqCst))),
            Arc::new(|| RoomId::generate().map_err(|_| ManagerError::RandomFailure)),
        )
        .unwrap();
        Self {
            secs,
            manager: Arc::new(manager),
        }
    }
    fn advance(&self, seconds: u64) {
        self.secs.store(seconds, Ordering::SeqCst);
    }
}
#[test]
fn creation_capacity() {
    let h = Harness::new(cfg());
    assert!(h.manager.is_empty().unwrap());
    for _ in 0..3 {
        h.manager.create().unwrap();
    }
    assert_eq!(h.manager.len(), Ok(3));
    assert_eq!(h.manager.create().err(), Some(ManagerError::AtCapacity));
}
#[test]
fn invalid_configuration() {
    for which in 0..6 {
        let mut c = cfg();
        match which {
            0 => c.max_rooms = 0,
            1 => c.max_cleanup_per_pass = 0,
            2 => c.max_id_retries = 0,
            3 => c.max_waiting_lifetime = Duration::ZERO,
            4 => c.max_invitation_lifetime = Duration::ZERO,
            _ => c.max_active_lifetime = Duration::ZERO,
        };
        assert_eq!(
            RoomManager::new(c).err(),
            Some(ManagerError::InvalidConfiguration)
        );
    }
}
#[test]
fn collision_then_success_preserves_first_room() {
    let n = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&n);
    let m = RoomManager::with_sources(
        cfg(),
        Arc::new(Instant::now),
        Arc::new(move || {
            let n = count.fetch_add(1, Ordering::SeqCst);
            Ok(RoomId::from_u128(if n < 2 { 7 } else { 8 }))
        }),
    )
    .unwrap();
    let first = m.create().unwrap();
    let second = m.create().unwrap();
    assert_eq!(first.id, RoomId::from_u128(7));
    assert_eq!(second.id, RoomId::from_u128(8));
    assert_eq!(
        m.cancel(
            first.id,
            first.credentials.creator_id,
            &first.credentials.creator_access
        ),
        Ok(())
    );
}
#[test]
fn collision_exhaustion_no_partial_entry() {
    let m = RoomManager::with_sources(
        cfg(),
        Arc::new(Instant::now),
        Arc::new(|| Ok(RoomId::from_u128(7))),
    )
    .unwrap();
    let first = m.create().unwrap();
    assert_eq!(m.create().err(), Some(ManagerError::IdCollisionExhausted));
    assert_eq!(m.len(), Ok(1));
    assert_eq!(
        m.cancel(
            first.id,
            first.credentials.creator_id,
            &first.credentials.creator_access
        ),
        Ok(())
    );
}
#[test]
fn id_failure_leaves_registry_empty() {
    let m = RoomManager::with_sources(
        cfg(),
        Arc::new(Instant::now),
        Arc::new(|| Err(ManagerError::RandomFailure)),
    )
    .unwrap();
    assert_eq!(m.create().err(), Some(ManagerError::RandomFailure));
    assert_eq!(m.len(), Ok(0));
}
#[test]
fn single_use_invite_and_no_third_participant() {
    let h = Harness::new(cfg());
    let c = h.manager.create().unwrap();
    let bob = h.manager.join(c.id, &c.credentials.invitation).unwrap();
    assert_eq!(
        h.manager.join(c.id, &c.credentials.invitation).err(),
        Some(ManagerError::InvalidInvitation)
    );
    assert_eq!(
        h.manager.status(c.id, bob.id, &bob.access),
        Ok(RoomLifecycle::Active)
    );
}
#[test]
fn simultaneous_redeem_exactly_one_wins() {
    let h = Harness::new(cfg());
    let c = h.manager.create().unwrap();
    let invitation = Arc::new(c.credentials.invitation);
    let barrier = Arc::new(Barrier::new(12));
    let mut jobs = Vec::new();
    for _ in 0..12 {
        let m = Arc::clone(&h.manager);
        let inv = Arc::clone(&invitation);
        let bar = Arc::clone(&barrier);
        jobs.push(thread::spawn(move || {
            bar.wait();
            m.join(c.id, &inv).is_ok()
        }));
    }
    assert_eq!(
        jobs.into_iter()
            .map(|j| j.join().unwrap())
            .filter(|ok| *ok)
            .count(),
        1
    );
}
#[test]
fn join_cancel_race_is_serialized() {
    let h = Harness::new(cfg());
    let c = h.manager.create().unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let a = Arc::clone(&h.manager);
    let b = Arc::clone(&h.manager);
    let ba = Arc::clone(&barrier);
    let bb = Arc::clone(&barrier);
    let id = c.id;
    let invite = c.credentials.invitation;
    let who = c.credentials.creator_id;
    let cap = c.credentials.creator_access;
    let join = thread::spawn(move || {
        ba.wait();
        a.join(id, &invite).is_ok()
    });
    let cancel = thread::spawn(move || {
        bb.wait();
        b.cancel(id, who, &cap).is_ok()
    });
    barrier.wait();
    assert_ne!(join.join().unwrap(), cancel.join().unwrap());
}
#[test]
fn exact_invitation_deadline_fails_closed() {
    let h = Harness::new(cfg());
    let c = h.manager.create().unwrap();
    h.advance(10);
    assert_eq!(
        h.manager.join(c.id, &c.credentials.invitation).err(),
        Some(ManagerError::Expired)
    );
    assert_eq!(
        h.manager.join(c.id, &c.credentials.invitation).err(),
        Some(ManagerError::InvalidInvitation)
    );
}
#[test]
fn authorization_expires_active_lifetime() {
    let h = Harness::new(cfg());
    let c = h.manager.create().unwrap();
    h.advance(5);
    let bob = h.manager.join(c.id, &c.credentials.invitation).unwrap();
    h.advance(24);
    assert_eq!(
        h.manager.status(c.id, bob.id, &bob.access),
        Ok(RoomLifecycle::Active)
    );
    h.advance(25);
    assert_eq!(
        h.manager.status(c.id, bob.id, &bob.access),
        Err(ManagerError::Expired)
    );
}
#[test]
fn incorrect_capability_cannot_close_or_act() {
    let h = Harness::new(cfg());
    let c = h.manager.create().unwrap();
    let bob = h.manager.join(c.id, &c.credentials.invitation).unwrap();
    assert_eq!(
        h.manager.close(c.id, bob.id, &c.credentials.creator_access),
        Err(ManagerError::Unauthorized)
    );
    assert_eq!(
        h.manager
            .status(c.id, bob.id, &c.credentials.creator_access),
        Err(ManagerError::Unauthorized)
    );
}
#[test]
fn closure_rejects_later_authorized_action() {
    let h = Harness::new(cfg());
    let c = h.manager.create().unwrap();
    let bob = h.manager.join(c.id, &c.credentials.invitation).unwrap();
    h.manager
        .close(
            c.id,
            c.credentials.creator_id,
            &c.credentials.creator_access,
        )
        .unwrap();
    assert_eq!(
        h.manager.status(c.id, bob.id, &bob.access),
        Err(ManagerError::Unauthorized)
    );
}
#[test]
fn both_members_can_close() {
    for creator in [true, false] {
        let h = Harness::new(cfg());
        let c = h.manager.create().unwrap();
        let bob = h.manager.join(c.id, &c.credentials.invitation).unwrap();
        let result = if creator {
            h.manager.close(
                c.id,
                c.credentials.creator_id,
                &c.credentials.creator_access,
            )
        } else {
            h.manager.close(c.id, bob.id, &bob.access)
        };
        assert_eq!(result, Ok(()));
    }
}
#[test]
fn close_waits_for_contended_room_lock() {
    use std::sync::mpsc;
    use std::sync::TryLockError;

    let h = Harness::new(cfg());
    let room = h.manager.create().unwrap();
    let joined = h
        .manager
        .join(room.id, &room.credentials.invitation)
        .unwrap();
    let entry = h.manager.lookup(room.id).unwrap();
    let guard = entry.lock().unwrap();
    let contender_entry = Arc::clone(&entry);
    let (attempted_tx, attempted_rx) = mpsc::channel();
    let manager = Arc::clone(&h.manager);
    let id = room.id;
    let user = room.credentials.creator_id;
    let cap = room.credentials.creator_access;
    let closer = thread::spawn(move || {
        assert!(matches!(
            contender_entry.try_lock(),
            Err(TryLockError::WouldBlock)
        ));
        attempted_tx.send(()).unwrap();
        manager.close(id, user, &cap)
    });
    attempted_rx.recv().unwrap();
    drop(guard);
    assert_eq!(closer.join().unwrap(), Ok(()));
    assert_eq!(
        h.manager.status(room.id, joined.id, &joined.access),
        Err(ManagerError::Unauthorized)
    );
}
#[test]
fn bounded_cleanup_round_robin_and_idempotence() {
    let h = Harness::new(cfg());
    for _ in 0..3 {
        h.manager.create().unwrap();
    }
    h.advance(11);
    for _ in 0..3 {
        let pass = h.manager.cleanup().unwrap();
        assert_eq!(pass.inspected, 1);
        assert_eq!(pass.removed, 1);
    }
    assert!(h.manager.is_empty().unwrap());
    assert_eq!(
        h.manager.cleanup().unwrap(),
        CleanupStats {
            inspected: 0,
            removed: 0
        }
    );
}
#[test]
fn capacity_recovered_after_cancel_and_cleanup() {
    let mut c = cfg();
    c.max_rooms = 1;
    let h = Harness::new(c);
    let room = h.manager.create().unwrap();
    assert_eq!(
        h.manager.cancel(
            room.id,
            room.credentials.creator_id,
            &room.credentials.creator_access
        ),
        Ok(())
    );
    assert_eq!(h.manager.create().err(), Some(ManagerError::AtCapacity));
    assert_eq!(h.manager.cleanup().unwrap().removed, 1);
    assert!(h.manager.create().is_ok());
}
#[test]
fn repeated_create_delete_is_bounded() {
    let mut c = cfg();
    c.max_rooms = 1;
    let h = Harness::new(c);
    for i in 0..24 {
        h.advance(i * 2);
        let room = h.manager.create().unwrap();
        h.manager
            .cancel(
                room.id,
                room.credentials.creator_id,
                &room.credentials.creator_access,
            )
            .unwrap();
        assert_eq!(h.manager.cleanup().unwrap().removed, 1);
        assert_eq!(h.manager.len(), Ok(0));
    }
}
#[test]
fn no_partial_entry_after_unknown_command() {
    let h = Harness::new(cfg());
    let id = RoomId::from_u128(999);
    assert_eq!(
        h.manager
            .join(id, &InvitationSecret::from_bytes([1; 32]))
            .err(),
        Some(ManagerError::NotFound)
    );
    assert_eq!(h.manager.len(), Ok(0));
}
#[test]
fn poisoned_registry_fails_closed() {
    let h = Harness::new(cfg());
    let m = Arc::clone(&h.manager);
    let j = thread::spawn(move || {
        let _guard = m.registry.lock().unwrap();
        panic!("test poison");
    });
    assert!(j.join().is_err());
    assert_eq!(h.manager.create().err(), Some(ManagerError::Poisoned));
}
#[test]
fn poisoned_entry_fails_closed() {
    let h = Harness::new(cfg());
    let c = h.manager.create().unwrap();
    let entry = h.manager.lookup(c.id).unwrap();
    let j = thread::spawn(move || {
        let _guard = entry.lock().unwrap();
        panic!("test poison");
    });
    assert!(j.join().is_err());
    assert_eq!(
        h.manager.close(
            c.id,
            c.credentials.creator_id,
            &c.credentials.creator_access
        ),
        Err(ManagerError::Poisoned)
    );
}
#[test]
fn stale_entry_retired_after_cleanup() {
    let h = Harness::new(cfg());
    let c = h.manager.create().unwrap();
    let stale = h.manager.lookup(c.id).unwrap();
    h.manager
        .cancel(
            c.id,
            c.credentials.creator_id,
            &c.credentials.creator_access,
        )
        .unwrap();
    assert_eq!(h.manager.cleanup().unwrap().removed, 1);
    assert!(stale.lock().unwrap().retired);
    assert_eq!(
        h.manager.cancel(
            c.id,
            c.credentials.creator_id,
            &c.credentials.creator_access
        ),
        Err(ManagerError::NotFound)
    );
}
#[test]
fn same_id_reuse_cannot_be_removed_by_stale_entry() {
    let m = RoomManager::with_sources(
        cfg(),
        Arc::new(Instant::now),
        Arc::new(|| Ok(RoomId::from_u128(7))),
    )
    .unwrap();
    let c = m.create().unwrap();
    let stale = m.lookup(c.id).unwrap();
    m.cancel(
        c.id,
        c.credentials.creator_id,
        &c.credentials.creator_access,
    )
    .unwrap();
    assert_eq!(m.cleanup().unwrap().removed, 1);
    let next = m.create().unwrap();
    assert!(!Arc::ptr_eq(&stale, &m.lookup(next.id).unwrap()));
    assert_eq!(m.len(), Ok(1));
}

#[test]
fn waiting_limit_does_not_shorten_active_room_lifetime() {
    let mut c = cfg();
    c.max_waiting_lifetime = Duration::from_secs(6);
    c.max_invitation_lifetime = Duration::from_secs(5);
    c.max_active_lifetime = Duration::from_secs(20);
    let h = Harness::new(c);
    let created = h.manager.create().unwrap();
    h.advance(4);
    let joined = h
        .manager
        .join(created.id, &created.credentials.invitation)
        .unwrap();
    // Waiting deadline was t=5. Active TTL starts at successful join t=4.
    h.advance(6);
    assert_eq!(
        h.manager.status(created.id, joined.id, &joined.access),
        Ok(RoomLifecycle::Active)
    );
    h.advance(23);
    assert_eq!(
        h.manager.status(created.id, joined.id, &joined.access),
        Ok(RoomLifecycle::Active)
    );
    h.advance(24);
    assert_eq!(
        h.manager.status(created.id, joined.id, &joined.access),
        Err(ManagerError::Expired)
    );
}

#[test]
fn shorter_waiting_limit_wins_before_join() {
    let mut c = cfg();
    c.max_waiting_lifetime = Duration::from_secs(4);
    c.max_invitation_lifetime = Duration::from_secs(15);
    let h = Harness::new(c);
    let created = h.manager.create().unwrap();
    h.advance(4);
    assert_eq!(
        h.manager
            .join(created.id, &created.credentials.invitation)
            .err(),
        Some(ManagerError::Expired)
    );
    assert_eq!(h.manager.cleanup().unwrap().removed, 1);
}

#[test]
fn closed_room_revokes_creator_and_invitee_status() {
    let h = Harness::new(cfg());
    let room = h.manager.create().unwrap();
    assert_eq!(
        h.manager.status(
            room.id,
            room.credentials.creator_id,
            &room.credentials.creator_access
        ),
        Ok(RoomLifecycle::WaitingForParticipant)
    );
    let joined = h
        .manager
        .join(room.id, &room.credentials.invitation)
        .unwrap();
    assert_eq!(
        h.manager.status(room.id, joined.id, &joined.access),
        Ok(RoomLifecycle::Active)
    );
    h.manager.close(room.id, joined.id, &joined.access).unwrap();
    assert_eq!(
        h.manager.status(room.id, joined.id, &joined.access),
        Err(ManagerError::Unauthorized)
    );
    assert_eq!(
        h.manager.status(
            room.id,
            room.credentials.creator_id,
            &room.credentials.creator_access
        ),
        Err(ManagerError::Unauthorized)
    );
}
#[test]
fn concurrent_room_creation_never_exceeds_capacity() {
    let mut c = cfg();
    c.max_rooms = 2;
    let h = Harness::new(c);
    let barrier = Arc::new(Barrier::new(12));
    let mut jobs = Vec::new();
    for _ in 0..12 {
        let manager = Arc::clone(&h.manager);
        let ready = Arc::clone(&barrier);
        jobs.push(thread::spawn(move || {
            ready.wait();
            manager.create().is_ok()
        }));
    }
    let successes = jobs
        .into_iter()
        .map(|j| j.join().unwrap())
        .filter(|ok| *ok)
        .count();
    assert_eq!(successes, 2);
    assert_eq!(h.manager.len(), Ok(2));
}

#[test]
fn cleanup_waits_for_contended_room_lock_and_reclaims() {
    use std::sync::mpsc;
    use std::sync::TryLockError;

    let h = Harness::new(cfg());
    let room = h.manager.create().unwrap();
    let _joined = h
        .manager
        .join(room.id, &room.credentials.invitation)
        .unwrap();
    let entry = h.manager.lookup(room.id).unwrap();
    let guard = entry.lock().unwrap();
    let contender_entry = Arc::clone(&entry);
    h.advance(31);
    let (attempted_tx, attempted_rx) = mpsc::channel();
    let manager = Arc::clone(&h.manager);
    let cleaner = thread::spawn(move || {
        assert!(matches!(
            contender_entry.try_lock(),
            Err(TryLockError::WouldBlock)
        ));
        attempted_tx.send(()).unwrap();
        manager.cleanup()
    });
    attempted_rx.recv().unwrap();
    drop(guard);
    assert_eq!(cleaner.join().unwrap().unwrap().removed, 1);
    assert_eq!(h.manager.len(), Ok(0));
    assert_eq!(
        h.manager.cancel(
            room.id,
            room.credentials.creator_id,
            &room.credentials.creator_access
        ),
        Err(ManagerError::NotFound)
    );
}
#[test]
fn simultaneous_forced_collisions_insert_unique_rooms() {
    let counter = Arc::new(AtomicUsize::new(0));
    let next = Arc::clone(&counter);
    let manager = Arc::new(
        RoomManager::with_sources(
            cfg(),
            Arc::new(Instant::now),
            Arc::new(move || {
                let index = next.fetch_add(1, Ordering::SeqCst);
                Ok(RoomId::from_u128(if index <= 1 { 7 } else { 8 }))
            }),
        )
        .unwrap(),
    );
    let start = Arc::new(Barrier::new(3));
    let mut jobs = Vec::new();
    for _ in 0..2 {
        let manager = Arc::clone(&manager);
        let start = Arc::clone(&start);
        jobs.push(thread::spawn(move || {
            start.wait();
            manager.create().unwrap()
        }));
    }
    start.wait();
    let mut rooms: Vec<_> = jobs
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    rooms.sort_by_key(|room| room.id.as_u128());
    assert_eq!(rooms[0].id, RoomId::from_u128(7));
    assert_eq!(rooms[1].id, RoomId::from_u128(8));
    assert_eq!(manager.len(), Ok(2));
    for room in &rooms {
        assert_eq!(
            manager.cancel(
                room.id,
                room.credentials.creator_id,
                &room.credentials.creator_access
            ),
            Ok(())
        );
    }
}
