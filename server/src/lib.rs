//! Loopback-only GhostRoom control-plane API (no messaging or E2EE).
#![forbid(unsafe_code)]

use axum::{
    body::{Body, Bytes},
    extract::{
        rejection::{BytesRejection, JsonRejection},
        DefaultBodyLimit, Path, State,
    },
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use ghostroom_core::{
    InvitationSecret, ManagerError, ParticipantAccessCapability, ParticipantId, RoomId,
    RoomLifecycle, RoomManager,
};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{sync::Semaphore, time::timeout};
use zeroize::Zeroize;

#[derive(Clone)]
struct AppState {
    manager: Arc<RoomManager>,
    permits: Arc<Semaphore>,
    limits: ServerLimits,
    create_rate: Arc<Mutex<TokenBucket>>,
    write_rate: Arc<Mutex<TokenBucket>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ServerLimits {
    pub max_body_bytes: usize,
    pub max_in_flight_requests: usize,
    pub request_timeout: Duration,
    pub creates_per_minute: u32,
    pub mutations_per_minute: u32,
}

impl ServerLimits {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.max_body_bytes == 0 || self.max_body_bytes > 8192 {
            return Err("request body limit must be in 1..=8192");
        }
        if self.max_in_flight_requests == 0 || self.max_in_flight_requests > 1024 {
            return Err("concurrency limit must be in 1..=1024");
        }
        if self.request_timeout < Duration::from_millis(50)
            || self.request_timeout > Duration::from_secs(30)
        {
            return Err("request timeout must be in 50ms..=30s");
        }
        if !(1..=600).contains(&self.creates_per_minute)
            || !(1..=600).contains(&self.mutations_per_minute)
        {
            return Err("rate limits must be in 1..=600 per minute");
        }
        Ok(self)
    }
}

pub fn validate_listener(addr: SocketAddr) -> Result<SocketAddr, &'static str> {
    if !addr.ip().is_loopback() {
        return Err("only loopback listeners are supported; Tor forwarding is not implemented");
    }
    Ok(addr)
}

/// Fixed-size instance-wide token bucket: no per-IP or client-provided keys.
/// Monotonic time is used only to replenish capacity; all room deadlines stay
/// owned by RoomManager.
struct TokenBucket {
    remaining: u32,
    last: Instant,
    limit: u32,
}
impl TokenBucket {
    fn new(limit: u32) -> Self {
        Self {
            remaining: limit,
            last: Instant::now(),
            limit,
        }
    }
    fn take(&mut self, now: Instant) -> bool {
        let spacing = Duration::from_secs(60) / self.limit;
        let earned = now.saturating_duration_since(self.last).as_nanos() / spacing.as_nanos();
        if earned > 0 {
            self.remaining =
                (u128::from(self.remaining) + earned).min(u128::from(self.limit)) as u32;
            self.last = now;
        }
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }
}

/// Router serving only room-control operations. The liveness route is
/// deliberately independent of business admission permits and rate buckets.
pub fn router(manager: Arc<RoomManager>, limits: ServerLimits) -> Result<Router, &'static str> {
    let limits = limits.validate()?;
    let state = AppState {
        manager,
        permits: Arc::new(Semaphore::new(limits.max_in_flight_requests)),
        limits,
        create_rate: Arc::new(Mutex::new(TokenBucket::new(limits.creates_per_minute))),
        write_rate: Arc::new(Mutex::new(TokenBucket::new(limits.mutations_per_minute))),
    };
    Ok(build_router(state))
}
fn build_router(state: AppState) -> Router {
    let ops = Router::new()
        .route("/v1/rooms", post(create))
        .route("/v1/rooms/{room_id}/join", post(join))
        .route("/v1/rooms/{room_id}/cancel", post(cancel))
        .route("/v1/rooms/{room_id}/close", post(close))
        .route("/v1/rooms/{room_id}/status", get(status))
        .layer(DefaultBodyLimit::max(state.limits.max_body_bytes))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            bound_in_flight,
        ));
    let health = Router::new()
        .route("/health/live", get(health))
        .layer(DefaultBodyLimit::max(state.limits.max_body_bytes));
    Router::new()
        .merge(ops)
        .merge(health)
        .layer(middleware::from_fn(add_no_store))
        .with_state(state)
}
async fn add_no_store(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
fn permitted_origin(headers: &HeaderMap) -> bool {
    // Native clients need not supply Origin or Sec-Fetch-Site.
    if let Some(site) = headers.get("sec-fetch-site") {
        if site
            .to_str()
            .map(|x| x.eq_ignore_ascii_case("cross-site"))
            .unwrap_or(true)
        {
            return false;
        }
    }
    let mut origins = headers.get_all(header::ORIGIN).iter();
    let Some(origin) = origins.next() else {
        return true;
    };
    if origins.next().is_some() {
        return false;
    }
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let Ok(host) = headers
        .get(header::HOST)
        .ok_or(())
        .and_then(|h| h.to_str().map_err(|_| ()))
    else {
        return false;
    };
    let loopback_host = host == "localhost"
        || host.starts_with("localhost:")
        || host == "127.0.0.1"
        || host.starts_with("127.0.0.1:")
        || host == "[::1]"
        || host.starts_with("[::1]:");
    loopback_host && origin == format!("http://{host}")
}
async fn bound_in_flight(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.method() != Method::GET && !permitted_origin(request.headers()) {
        return ApiError(StatusCode::FORBIDDEN, "origin_forbidden").into_response();
    }
    let create_request = request.method() == Method::POST && request.uri().path() == "/v1/rooms";
    // Status reads are capability-gated, but do not charge the mutation bucket.
    let mutation = request.method() == Method::POST && !create_request;
    if create_request || mutation {
        let bucket = if create_request {
            &state.create_rate
        } else {
            &state.write_rate
        };
        let accepted = match bucket.lock() {
            Ok(mut lock) => lock.take(Instant::now()),
            Err(_) => {
                return ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
                    .into_response()
            }
        };
        if !accepted {
            let rate = if create_request {
                state.limits.creates_per_minute
            } else {
                state.limits.mutations_per_minute
            };
            let seconds = (60 / rate).max(1);
            let mut response =
                ApiError(StatusCode::TOO_MANY_REQUESTS, "rate_limited").into_response();
            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            return response;
        }
    }
    let Ok(_permit) = state.permits.clone().try_acquire_owned() else {
        return ApiError(StatusCode::SERVICE_UNAVAILABLE, "busy").into_response();
    };
    match timeout(state.limits.request_timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => {
            let mut response =
                ApiError(StatusCode::REQUEST_TIMEOUT, "request_timeout").into_response();
            response
                .headers_mut()
                .insert(header::CONNECTION, HeaderValue::from_static("close"));
            response
        }
    }
}
#[derive(Serialize)]
struct Health {
    status: &'static str,
}
async fn health(body: Result<Bytes, BytesRejection>) -> Result<Json<Health>, ApiError> {
    empty_body(body)?;
    Ok(Json(Health { status: "ok" }))
}

#[derive(Serialize)]
struct ErrorResponse {
    code: &'static str,
}
struct ApiError(StatusCode, &'static str);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(ErrorResponse { code: self.1 })).into_response()
    }
}
fn empty_body(body: Result<Bytes, BytesRejection>) -> Result<(), ApiError> {
    let bytes = body.map_err(|e| {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            ApiError(StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large")
        } else {
            bad()
        }
    })?;
    if bytes.is_empty() {
        Ok(())
    } else {
        Err(bad())
    }
}
fn bad() -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, "invalid_request")
}
fn concealed() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "room_unavailable")
}
fn map_manager(e: ManagerError) -> ApiError {
    match e {
        ManagerError::AtCapacity => ApiError(StatusCode::SERVICE_UNAVAILABLE, "at_capacity"),
        ManagerError::IdCollisionExhausted
        | ManagerError::RandomFailure
        | ManagerError::DeadlineOverflow
        | ManagerError::Poisoned
        | ManagerError::InvalidConfiguration => {
            ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
        }
        _ => concealed(),
    }
}
fn hex(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        result.push(char::from(ALPHABET[usize::from(b >> 4)]));
        result.push(char::from(ALPHABET[usize::from(b & 0x0f)]));
    }
    result
}
fn decode32(src: &str) -> Result<[u8; 32], ApiError> {
    if src.len() != 64 || !src.is_ascii() {
        return Err(bad());
    }
    let mut out = [0u8; 32];
    let raw = src.as_bytes();
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = (raw[2 * i] as char).to_digit(16).ok_or_else(bad)?;
        let lo = (raw[2 * i + 1] as char).to_digit(16).ok_or_else(bad)?;
        *slot = ((hi << 4) | lo) as u8;
    }
    Ok(out)
}
fn decode_id(src: &str) -> Result<u128, ApiError> {
    if src.len() != 32 || !src.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(concealed());
    }
    u128::from_str_radix(src, 16).map_err(|_| concealed())
}
fn authorization(
    headers: &HeaderMap,
) -> Result<(ParticipantId, ParticipantAccessCapability), ApiError> {
    let mut ids = headers.get_all("x-ghostroom-participant-id").iter();
    let id = ids
        .next()
        .ok_or_else(concealed)?
        .to_str()
        .map_err(|_| concealed())?;
    if ids.next().is_some() {
        return Err(concealed());
    }
    let id = decode_id(id)?;
    let mut auths = headers.get_all(header::AUTHORIZATION).iter();
    let value = auths
        .next()
        .ok_or_else(concealed)?
        .to_str()
        .map_err(|_| concealed())?;
    if auths.next().is_some() {
        return Err(concealed());
    }
    let (scheme, token) = value.split_once(' ').ok_or_else(concealed)?;
    if !scheme.eq_ignore_ascii_case("bearer") || token.contains(char::is_whitespace) {
        return Err(concealed());
    }
    let bytes = decode32(token).map_err(|_| concealed())?;
    Ok((
        ParticipantId::from_u128(id),
        ParticipantAccessCapability::from_bytes(bytes),
    ))
}

#[derive(Serialize)]
struct Created {
    room_id: String,
    creator_id: String,
    creator_access: String,
    invitation: String,
}
async fn create(
    State(state): State<AppState>,
    body: Result<Bytes, BytesRejection>,
) -> Result<
    (
        StatusCode,
        [(header::HeaderName, &'static str); 1],
        Json<Created>,
    ),
    ApiError,
> {
    empty_body(body)?;
    let room = state.manager.create().map_err(map_manager)?;
    Ok((
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(Created {
            room_id: format!("{:032x}", room.id.as_u128()),
            creator_id: format!("{:032x}", room.credentials.creator_id.as_u128()),
            creator_access: hex(room.credentials.creator_access.as_bytes()),
            invitation: hex(room.credentials.invitation.as_bytes()),
        }),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JoinRequest {
    invitation: String,
}
impl Drop for JoinRequest {
    fn drop(&mut self) {
        self.invitation.zeroize();
    }
}
#[derive(Serialize)]
struct Joined {
    participant_id: String,
    access: String,
}
async fn join(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    input: Result<Json<JoinRequest>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        [(header::HeaderName, &'static str); 1],
        Json<Joined>,
    ),
    ApiError,
> {
    let room_id = RoomId::from_u128(decode_id(&room_id)?);
    let Json(payload) = input.map_err(|e| {
        if e.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE {
            ApiError(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_media_type")
        } else if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            ApiError(StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large")
        } else {
            bad()
        }
    })?;
    let secret = InvitationSecret::from_bytes(decode32(&payload.invitation)?);
    let joined = state.manager.join(room_id, &secret).map_err(map_manager)?;
    Ok((
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(Joined {
            participant_id: format!("{:032x}", joined.id.as_u128()),
            access: hex(joined.access.as_bytes()),
        }),
    ))
}
async fn cancel(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<StatusCode, ApiError> {
    empty_body(body)?;
    let id = RoomId::from_u128(decode_id(&room_id)?);
    let (who, cap) = authorization(&headers)?;
    state.manager.cancel(id, who, &cap).map_err(map_manager)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn close(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<StatusCode, ApiError> {
    empty_body(body)?;
    let id = RoomId::from_u128(decode_id(&room_id)?);
    let (who, cap) = authorization(&headers)?;
    state.manager.close(id, who, &cap).map_err(map_manager)?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Serialize)]
struct Status {
    state: &'static str,
}
async fn status(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<([(header::HeaderName, &'static str); 1], Json<Status>), ApiError> {
    empty_body(body)?;
    let id = RoomId::from_u128(decode_id(&room_id)?);
    let (who, cap) = authorization(&headers)?;
    let state = state.manager.status(id, who, &cap).map_err(map_manager)?;
    let state = match state {
        RoomLifecycle::WaitingForParticipant => "waiting",
        RoomLifecycle::Active => "active",
    };
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(Status { state }),
    ))
}

#[cfg(test)]
mod hardening_tests {
    use super::*;
    use ghostroom_core::RoomManagerConfig;
    use std::time::Duration;
    use tower::ServiceExt;

    #[tokio::test]
    async fn liveness_isolated_from_exhausted_business_permits() {
        let manager = Arc::new(
            RoomManager::new(RoomManagerConfig {
                max_rooms: 2,
                max_waiting_lifetime: Duration::from_secs(60),
                max_active_lifetime: Duration::from_secs(60),
                max_invitation_lifetime: Duration::from_secs(60),
                max_cleanup_per_pass: 1,
                max_id_retries: 2,
            })
            .unwrap(),
        );
        let limits = ServerLimits {
            max_body_bytes: 256,
            max_in_flight_requests: 1,
            request_timeout: Duration::from_secs(1),
            creates_per_minute: 2,
            mutations_per_minute: 2,
        };
        let state = AppState {
            manager,
            permits: Arc::new(Semaphore::new(1)),
            limits,
            create_rate: Arc::new(Mutex::new(TokenBucket::new(2))),
            write_rate: Arc::new(Mutex::new(TokenBucket::new(2))),
        };
        let router = build_router(state.clone());
        let permit = state.permits.clone().try_acquire_owned().unwrap();
        let health = Request::builder()
            .uri("/health/live")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            router.clone().oneshot(health).await.unwrap().status(),
            StatusCode::OK
        );
        let create = Request::builder()
            .method("POST")
            .uri("/v1/rooms")
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(create).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        drop(permit);
    }
}
