//! Bounded synchronous room ownership and atomic lifecycle operations.
use crate::{
    CreatedRoomCredentials, InvitationSecret, JoinedParticipant, ParticipantAccessCapability,
    ParticipantId, ParticipationError, ParticipationState, RoomId, SecureRoom,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Limits are strict; zero limits and overflowing deadlines are rejected.
#[derive(Clone, Copy, Debug)]
pub struct RoomManagerConfig {
    pub max_rooms: usize,
    pub max_waiting_lifetime: Duration,
    pub max_active_lifetime: Duration,
    pub max_invitation_lifetime: Duration,
    pub max_cleanup_per_pass: usize,
    pub max_id_retries: usize,
}
impl RoomManagerConfig {
    fn validate(self) -> Result<Self, ManagerError> {
        if self.max_rooms == 0
            || self.max_cleanup_per_pass == 0
            || self.max_id_retries == 0
            || self.max_waiting_lifetime.is_zero()
            || self.max_active_lifetime.is_zero()
            || self.max_invitation_lifetime.is_zero()
        {
            return Err(ManagerError::InvalidConfiguration);
        }
        Ok(self)
    }
}

/// Internal failures are never serialized directly as network responses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagerError {
    InvalidConfiguration,
    AtCapacity,
    IdCollisionExhausted,
    NotFound,
    InvalidInvitation,
    Unauthorized,
    NotActive,
    Closed,
    Expired,
    InvalidState,
    RandomFailure,
    DeadlineOverflow,
    Poisoned,
}
impl std::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ManagerError {}
impl From<ParticipationError> for ManagerError {
    fn from(e: ParticipationError) -> Self {
        match e {
            ParticipationError::Unauthorized => Self::Unauthorized,
            ParticipationError::InvalidInvitation => Self::InvalidInvitation,
            ParticipationError::NotActive => Self::NotActive,
            ParticipationError::AlreadyClosed => Self::Closed,
            ParticipationError::Expired | ParticipationError::InvitationExpired => Self::Expired,
            ParticipationError::RandomFailure => Self::RandomFailure,
            ParticipationError::BeforeCreation
            | ParticipationError::InvalidDeadline
            | ParticipationError::InvalidState => Self::InvalidState,
        }
    }
}
/// Credentials returned once from successful room creation.
pub struct CreatedManagedRoom {
    pub id: RoomId,
    pub credentials: CreatedRoomCredentials,
}
impl std::fmt::Debug for CreatedManagedRoom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CreatedManagedRoom([REDACTED])")
    }
}
/// Minimal, capability-gated, non-authoritative lifecycle observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoomLifecycle {
    WaitingForParticipant,
    Active,
}

/// Number inspected and removed in one bounded maintenance pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupStats {
    pub inspected: usize,
    pub removed: usize,
}

struct Entry {
    room: SecureRoom,
    retired: bool,
}
struct Registry {
    rooms: BTreeMap<RoomId, Arc<Mutex<Entry>>>,
    cursor: Option<RoomId>,
}

/// Synchronous manager: registry lock is short-held; each room has one lock.
///
/// Lock ordering: never take registry lock while holding entry lock. Lookups
/// clone a private Arc under registry lock, then lock the entry. Cleanup
/// retires an entry under its entry lock, releases it, then verifies pointer
/// identity under registry lock before removal. Stale cloned Arcs fail closed.
///
/// Manager operations obtain trusted time internally, and no external
/// code executes while a room lock is held. There is no message queue,
/// network I/O, or reusable authorization token in this API.
pub struct RoomManager {
    cfg: RoomManagerConfig,
    registry: Mutex<Registry>,
    clock: Arc<dyn Fn() -> Instant + Send + Sync>,
    ids: Arc<dyn Fn() -> Result<RoomId, ManagerError> + Send + Sync>,
}
impl std::fmt::Debug for RoomManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RoomManager([REDACTED])")
    }
}
impl RoomManager {
    pub fn new(cfg: RoomManagerConfig) -> Result<Self, ManagerError> {
        Self::with_sources(
            cfg,
            Arc::new(Instant::now),
            Arc::new(|| RoomId::generate().map_err(|_| ManagerError::RandomFailure)),
        )
    }
    // Internal test seam. Neither clock nor ID source can be supplied publicly.
    fn with_sources(
        cfg: RoomManagerConfig,
        clock: Arc<dyn Fn() -> Instant + Send + Sync>,
        ids: Arc<dyn Fn() -> Result<RoomId, ManagerError> + Send + Sync>,
    ) -> Result<Self, ManagerError> {
        Ok(Self {
            cfg: cfg.validate()?,
            registry: Mutex::new(Registry {
                rooms: BTreeMap::new(),
                cursor: None,
            }),
            clock,
            ids,
        })
    }
    fn now(&self) -> Instant {
        (self.clock)()
    }
    fn lock_registry(&self) -> Result<std::sync::MutexGuard<'_, Registry>, ManagerError> {
        self.registry.lock().map_err(|_| ManagerError::Poisoned)
    }
    fn lookup(&self, id: RoomId) -> Result<Arc<Mutex<Entry>>, ManagerError> {
        self.lock_registry()?
            .rooms
            .get(&id)
            .cloned()
            .ok_or(ManagerError::NotFound)
    }
    /// Insert exactly one room. Collisions never overwrite existing rooms.
    /// Secure randomness, deadline calculation and verifier construction
    /// occur OUTSIDE the registry lock; only capacity and uniqueness checks,
    /// plus the atomic map insert, execute inside it.
    pub fn create(&self) -> Result<CreatedManagedRoom, ManagerError> {
        // Fast rejection: the second check under insertion lock remains the
        // authoritative one if concurrent creators are racing.
        if self.len()? >= self.cfg.max_rooms {
            return Err(ManagerError::AtCapacity);
        }
        for _ in 0..self.cfg.max_id_retries {
            let id = (self.ids)()?;
            let now = self.now();
            let wait = now
                .checked_add(self.cfg.max_waiting_lifetime)
                .ok_or(ManagerError::DeadlineOverflow)?;
            let invitation = now
                .checked_add(self.cfg.max_invitation_lifetime)
                .ok_or(ManagerError::DeadlineOverflow)?;
            let invitation = invitation.min(wait);
            // Construct credentials outside the lock. A collision discards
            // this candidate and its secret-bearing values without exposing
            // the credentials to a caller or overwriting existing state.
            let (room, credentials) = SecureRoom::create_with_id(id, now, None, invitation)
                .map_err(ManagerError::from)?;
            let mut reg = self.lock_registry()?;
            if reg.rooms.len() >= self.cfg.max_rooms {
                return Err(ManagerError::AtCapacity);
            }
            if reg.rooms.contains_key(&id) {
                continue;
            }
            reg.rooms.insert(
                id,
                Arc::new(Mutex::new(Entry {
                    room,
                    retired: false,
                })),
            );
            return Ok(CreatedManagedRoom { id, credentials });
        }
        Err(ManagerError::IdCollisionExhausted)
    }
    fn operate<T>(
        &self,
        id: RoomId,
        f: impl FnOnce(&mut SecureRoom, Instant) -> Result<T, ManagerError>,
    ) -> Result<T, ManagerError> {
        let entry = self.lookup(id)?;
        let mut guard = entry.lock().map_err(|_| ManagerError::Poisoned)?;
        if guard.retired {
            return Err(ManagerError::NotFound);
        }
        f(&mut guard.room, self.now())
    }
    /// Successful redemption activates a room, bounded to an active TTL.
    pub fn join(
        &self,
        id: RoomId,
        invitation: &InvitationSecret,
    ) -> Result<JoinedParticipant, ManagerError> {
        self.operate(id, |room, now| {
            let active_end = now
                .checked_add(self.cfg.max_active_lifetime)
                .ok_or(ManagerError::DeadlineOverflow)?;
            let joined = room.join(invitation, now).map_err(ManagerError::from)?;
            room.limit_active_until(active_end);
            Ok(joined)
        })
    }
    pub fn cancel(
        &self,
        id: RoomId,
        user: ParticipantId,
        cap: &ParticipantAccessCapability,
    ) -> Result<(), ManagerError> {
        self.operate(id, |room, now| {
            room.cancel(user, cap, now).map_err(ManagerError::from)
        })
    }
    pub fn close(
        &self,
        id: RoomId,
        user: ParticipantId,
        cap: &ParticipantAccessCapability,
    ) -> Result<(), ManagerError> {
        self.operate(id, |room, now| {
            room.close(user, cap, now).map_err(ManagerError::from)
        })
    }
    /// Return a minimal lifecycle value after checking this room's bearer
    /// capability and its trusted deadline, under the same room mutex.
    /// This snapshot is never authority for any later mutation.
    pub fn status(
        &self,
        id: RoomId,
        user: ParticipantId,
        cap: &ParticipantAccessCapability,
    ) -> Result<RoomLifecycle, ManagerError> {
        self.operate(id, |room, now| {
            match room
                .authorized_state(user, cap, now)
                .map_err(ManagerError::from)?
            {
                ParticipationState::WaitingForParticipant => {
                    Ok(RoomLifecycle::WaitingForParticipant)
                }
                ParticipationState::Active => Ok(RoomLifecycle::Active),
                ParticipationState::Closed(_) => Err(ManagerError::Closed),
            }
        })
    }
    /// Return room count. Lock poisoning fails closed.
    pub fn len(&self) -> Result<usize, ManagerError> {
        Ok(self.lock_registry()?.rooms.len())
    }
    pub fn is_empty(&self) -> Result<bool, ManagerError> {
        Ok(self.len()? == 0)
    }
    /// Inspect at most the configured number of entries, rotating the cursor.
    /// Each terminal entry is retired before removal; stale Arcs cannot act.
    pub fn cleanup(&self) -> Result<CleanupStats, ManagerError> {
        use std::ops::Bound::{Excluded, Unbounded};
        let candidates = {
            let mut reg = self.lock_registry()?;
            let n = self.cfg.max_cleanup_per_pass.min(reg.rooms.len());
            let mut ids = Vec::with_capacity(n);
            if let Some(cursor) = reg.cursor {
                ids.extend(
                    reg.rooms
                        .range((Excluded(cursor), Unbounded))
                        .take(n)
                        .map(|(&id, _)| id),
                );
            }
            if ids.len() < n {
                ids.extend(reg.rooms.iter().take(n - ids.len()).map(|(&id, _)| id));
            }
            reg.cursor = ids.last().copied();
            ids.into_iter()
                .filter_map(|id| reg.rooms.get(&id).map(|e| (id, Arc::clone(e))))
                .collect::<Vec<_>>()
        };
        let mut removed = 0;
        for (id, entry) in &candidates {
            let should_remove = {
                let mut guard = entry.lock().map_err(|_| ManagerError::Poisoned)?;
                if !guard.retired {
                    guard.room.expire_if_due(self.now());
                }
                let terminal = matches!(guard.room.recorded_state(), ParticipationState::Closed(_));
                if terminal {
                    guard.retired = true;
                }
                terminal
            };
            if should_remove {
                let mut reg = self.lock_registry()?;
                if reg
                    .rooms
                    .get(id)
                    .is_some_and(|existing| Arc::ptr_eq(existing, entry))
                {
                    reg.rooms.remove(id);
                    removed += 1;
                }
            }
        }
        Ok(CleanupStats {
            inspected: candidates.len(),
            removed,
        })
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
