use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use ghostroom_core::{RoomManager, RoomManagerConfig};
use ghostroom_server::{router, validate_listener, ServerLimits};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tower::ServiceExt;

fn app(max_rooms: usize) -> axum::Router {
    let mgr = RoomManager::new(RoomManagerConfig {
        max_rooms,
        max_waiting_lifetime: Duration::from_secs(60),
        max_active_lifetime: Duration::from_secs(60),
        max_invitation_lifetime: Duration::from_secs(60),
        max_cleanup_per_pass: 2,
        max_id_retries: 4,
    })
    .unwrap();
    router(
        Arc::new(mgr),
        ServerLimits {
            max_body_bytes: 512,
            max_in_flight_requests: 16,
            request_timeout: Duration::from_secs(2),
            creates_per_minute: 600,
            mutations_per_minute: 600,
        },
    )
    .unwrap()
}
async fn send(router: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, json)
}
fn req(method: &str, path: &str, body: String) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}
fn auth(mut request: Request<Body>, id: &str, secret: &str) -> Request<Body> {
    request
        .headers_mut()
        .insert("x-ghostroom-participant-id", id.parse().unwrap());
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {secret}").parse().unwrap());
    request
}
async fn create(r: &axum::Router) -> Value {
    let (status, body) = send(r, req("POST", "/v1/rooms", String::new())).await;
    assert_eq!(status, StatusCode::CREATED);
    body
}
async fn join(r: &axum::Router, room: &Value) -> (StatusCode, Value) {
    send(
        r,
        req(
            "POST",
            &format!("/v1/rooms/{}/join", room["room_id"].as_str().unwrap()),
            json!({"invitation":room["invitation"]}).to_string(),
        ),
    )
    .await
}
#[tokio::test]
async fn health_contains_no_room_data() {
    let router = app(2);
    let (s, body) = send(&router, req("GET", "/health/live", String::new())).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, json!({"status":"ok"}));
}
#[tokio::test]
async fn creates_joins_and_redeems_invitation_once() {
    let router = app(2);
    let room = create(&router).await;
    for key in ["room_id", "creator_id", "creator_access", "invitation"] {
        assert!(room[key].as_str().unwrap().len() >= 32);
    }
    let (s, bob) = join(&router, &room).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(bob["access"].as_str().unwrap().len(), 64);
    let (s, _) = join(&router, &room).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn status_requires_capability_and_close_revokes() {
    let router = app(2);
    let room = create(&router).await;
    let path = format!("/v1/rooms/{}/status", room["room_id"].as_str().unwrap());
    let (s, _) = send(&router, req("GET", &path, String::new())).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let creator = auth(
        req("GET", &path, String::new()),
        room["creator_id"].as_str().unwrap(),
        room["creator_access"].as_str().unwrap(),
    );
    let (s, state) = send(&router, creator).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(state, json!({"state":"waiting"}));
    let (_, joined) = join(&router, &room).await;
    let actor = auth(
        req("GET", &path, String::new()),
        joined["participant_id"].as_str().unwrap(),
        joined["access"].as_str().unwrap(),
    );
    assert_eq!(send(&router, actor).await.1, json!({"state":"active"}));
    let close = format!("/v1/rooms/{}/close", room["room_id"].as_str().unwrap());
    let outsider = auth(
        req("POST", &close, String::new()),
        joined["participant_id"].as_str().unwrap(),
        room["creator_access"].as_str().unwrap(),
    );
    assert_eq!(send(&router, outsider).await.0, StatusCode::NOT_FOUND);
    let user = auth(
        req("POST", &close, String::new()),
        joined["participant_id"].as_str().unwrap(),
        joined["access"].as_str().unwrap(),
    );
    assert_eq!(send(&router, user).await.0, StatusCode::NO_CONTENT);
    let stale = auth(
        req("GET", &path, String::new()),
        joined["participant_id"].as_str().unwrap(),
        joined["access"].as_str().unwrap(),
    );
    assert_eq!(send(&router, stale).await.0, StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn creator_can_cancel_only_waiting_room() {
    let router = app(2);
    let room = create(&router).await;
    let cancel = format!("/v1/rooms/{}/cancel", room["room_id"].as_str().unwrap());
    let request = auth(
        req("POST", &cancel, String::new()),
        room["creator_id"].as_str().unwrap(),
        room["creator_access"].as_str().unwrap(),
    );
    assert_eq!(send(&router, request).await.0, StatusCode::NO_CONTENT);
    assert_eq!(join(&router, &room).await.0, StatusCode::NOT_FOUND);
    let request = auth(
        req("POST", &cancel, String::new()),
        room["creator_id"].as_str().unwrap(),
        room["creator_access"].as_str().unwrap(),
    );
    assert_eq!(send(&router, request).await.0, StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn rejects_bad_payloads_and_missing_capabilities() {
    let router = app(2);
    let room = create(&router).await;
    let path = format!("/v1/rooms/{}/join", room["room_id"].as_str().unwrap());
    assert_eq!(
        send(&router, req("POST", &path, "{".into())).await.0,
        StatusCode::BAD_REQUEST
    );
    let mut plain = req("POST", &path, "{}".into());
    plain
        .headers_mut()
        .insert("content-type", "text/plain".parse().unwrap());
    assert_eq!(
        send(&router, plain).await.0,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    assert_eq!(
        send(&router, req("POST", &path, "x".repeat(1200))).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        send(
            &router,
            req("POST", &path, json!({"invitation":"abcd"}).to_string())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(&router, req("POST", "/v1/rooms", "garbage".into()))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}
#[tokio::test]
async fn capacity_and_credential_errors_are_concealed() {
    let router = app(1);
    let room = create(&router).await;
    assert_eq!(
        send(&router, req("POST", "/v1/rooms", String::new()))
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let id = room["room_id"].as_str().unwrap();
    let fake = format!("/v1/rooms/{:032x}/status", 1234u128);
    let real = format!("/v1/rooms/{id}/status");
    let (unknown, body_unknown) = send(&router, req("GET", &fake, String::new())).await;
    let (denied, body_denied) = send(&router, req("GET", &real, String::new())).await;
    assert_eq!(unknown, denied);
    assert_eq!(body_unknown, body_denied);
    assert!(!body_denied
        .to_string()
        .contains(room["creator_access"].as_str().unwrap()));
}
#[tokio::test]
async fn rejects_remote_listeners_and_has_no_message_routes() {
    for input in ["0.0.0.0:38421", "192.168.1.1:1234", "[::]:38421"] {
        assert!(validate_listener(input.parse::<SocketAddr>().unwrap()).is_err());
    }
    assert!(validate_listener("127.0.0.1:38421".parse().unwrap()).is_ok());
    let router = app(1);
    for path in [
        "/v1/messages",
        "/v1/rooms/123/send",
        "/v1/rooms/123/receive",
    ] {
        assert_eq!(
            send(&router, req("POST", path, "".into())).await.0,
            StatusCode::NOT_FOUND
        );
    }
}
#[tokio::test]
async fn concurrent_join_exactly_one_winner() {
    let router = app(2);
    let room = create(&router).await;
    let mut jobs = Vec::new();
    for _ in 0..10 {
        let r = router.clone();
        let room = room.clone();
        jobs.push(tokio::spawn(async move { join(&r, &room).await.0 }));
    }
    let mut success = 0;
    for job in jobs {
        if job.await.unwrap() == StatusCode::CREATED {
            success += 1;
        }
    }
    assert_eq!(success, 1);
}

#[tokio::test]
async fn wrong_room_capabilities_and_invitations_do_not_cross_authorize() {
    let r = app(3);
    let room_a = create(&r).await;
    let room_b = create(&r).await;
    let join_b = format!("/v1/rooms/{}/join", room_b["room_id"].as_str().unwrap());
    let attempt = req(
        "POST",
        &join_b,
        json!({"invitation": room_a["invitation"]}).to_string(),
    );
    assert_eq!(send(&r, attempt).await.0, StatusCode::NOT_FOUND);
    let status_b = format!("/v1/rooms/{}/status", room_b["room_id"].as_str().unwrap());
    let wrong = auth(
        req("GET", &status_b, String::new()),
        room_a["creator_id"].as_str().unwrap(),
        room_a["creator_access"].as_str().unwrap(),
    );
    assert_eq!(send(&r, wrong).await.0, StatusCode::NOT_FOUND);
    assert_eq!(join(&r, &room_a).await.0, StatusCode::CREATED);
    assert_eq!(join(&r, &room_b).await.0, StatusCode::CREATED);
}

#[tokio::test]
async fn either_participant_can_close_their_own_room() {
    let r = app(3);
    for by_creator in [true, false] {
        let room = create(&r).await;
        let (_, joined) = join(&r, &room).await;
        let path = format!("/v1/rooms/{}/close", room["room_id"].as_str().unwrap());
        let (id, cap) = if by_creator {
            (
                room["creator_id"].as_str().unwrap(),
                room["creator_access"].as_str().unwrap(),
            )
        } else {
            (
                joined["participant_id"].as_str().unwrap(),
                joined["access"].as_str().unwrap(),
            )
        };
        assert_eq!(
            send(&r, auth(req("POST", &path, String::new()), id, cap))
                .await
                .0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            send(&r, auth(req("POST", &path, String::new()), id, cap))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
}

#[tokio::test]
async fn explicit_limits_reject_bad_configuration() {
    for value in [0, 8193] {
        assert!(ServerLimits {
            max_body_bytes: value,
            max_in_flight_requests: 4,
            request_timeout: Duration::from_secs(2),
            creates_per_minute: 600,
            mutations_per_minute: 600
        }
        .validate()
        .is_err());
    }
    for value in [0, 1025] {
        assert!(ServerLimits {
            max_body_bytes: 128,
            max_in_flight_requests: value,
            request_timeout: Duration::from_secs(2),
            creates_per_minute: 600,
            mutations_per_minute: 600
        }
        .validate()
        .is_err());
    }
    assert!(validate_listener("[::1]:38421".parse().unwrap()).is_ok());
    let r = app(1);
    assert_eq!(
        send(&r, req("GET", "/health/live", "x".repeat(2000)))
            .await
            .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let room = create(&r).await;
    let close = format!("/v1/rooms/{}/close", room["room_id"].as_str().unwrap());
    assert_eq!(
        send(&r, req("POST", &close, "x".repeat(2000))).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn credentials_are_marked_uncacheable() {
    let router = app(1);
    let response = router
        .clone()
        .oneshot(req("POST", "/v1/rooms", String::new()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "no-store"
    );
}

#[tokio::test]
async fn concurrent_close_and_status_have_serialized_outcomes() {
    let router = app(1);
    let room = create(&router).await;
    let (_, joined) = join(&router, &room).await;
    let status_path = format!("/v1/rooms/{}/status", room["room_id"].as_str().unwrap());
    let close_path = format!("/v1/rooms/{}/close", room["room_id"].as_str().unwrap());
    let barrier = Arc::new(tokio::sync::Barrier::new(3));
    let a = router.clone();
    let b = router.clone();
    let ba = Arc::clone(&barrier);
    let bb = Arc::clone(&barrier);
    let creator_id = room["creator_id"].as_str().unwrap().to_owned();
    let creator_cap = room["creator_access"].as_str().unwrap().to_owned();
    let invitee_id = joined["participant_id"].as_str().unwrap().to_owned();
    let invitee_cap = joined["access"].as_str().unwrap().to_owned();
    let observe = tokio::spawn(async move {
        ba.wait().await;
        send(
            &a,
            auth(
                req("GET", &status_path, String::new()),
                &invitee_id,
                &invitee_cap,
            ),
        )
        .await
        .0
    });
    let close = tokio::spawn(async move {
        bb.wait().await;
        send(
            &b,
            auth(
                req("POST", &close_path, String::new()),
                &creator_id,
                &creator_cap,
            ),
        )
        .await
        .0
    });
    barrier.wait().await;
    let observed = observe.await.unwrap();
    assert!(observed == StatusCode::OK || observed == StatusCode::NOT_FOUND);
    assert_eq!(close.await.unwrap(), StatusCode::NO_CONTENT);
    let after = auth(
        req(
            "GET",
            &format!("/v1/rooms/{}/status", room["room_id"].as_str().unwrap()),
            String::new(),
        ),
        joined["participant_id"].as_str().unwrap(),
        joined["access"].as_str().unwrap(),
    );
    assert_eq!(send(&router, after).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tcp_listener_accepts_health_and_shuts_down() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let handler = tokio::spawn(async move {
        axum::serve(listener, app(1))
            .with_graceful_shutdown(async move {
                let _ = stop_rx.await;
            })
            .await
            .unwrap();
    });
    let mut tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    tcp.write_all(b"GET /health/live HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = Vec::new();
    tcp.read_to_end(&mut response).await.unwrap();
    assert!(String::from_utf8_lossy(&response).contains("200 OK"));
    stop_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), handler)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn duplicate_auth_headers_and_wrong_scheme_are_rejected() {
    let r = app(2);
    let room = create(&r).await;
    let url = format!("/v1/rooms/{}/status", room["room_id"].as_str().unwrap());
    for header in ["authorization", "x-ghostroom-participant-id"] {
        let mut request = auth(
            req("GET", &url, String::new()),
            room["creator_id"].as_str().unwrap(),
            room["creator_access"].as_str().unwrap(),
        );
        request
            .headers_mut()
            .append(header, "duplicated".parse().unwrap());
        assert_eq!(send(&r, request).await.0, StatusCode::NOT_FOUND);
    }
    let mut lower = auth(
        req("GET", &url, String::new()),
        room["creator_id"].as_str().unwrap(),
        room["creator_access"].as_str().unwrap(),
    );
    lower.headers_mut().insert(
        "authorization",
        format!("bEaReR {}", room["creator_access"].as_str().unwrap())
            .parse()
            .unwrap(),
    );
    assert_eq!(send(&r, lower).await.0, StatusCode::OK);
    for authorization in ["Token deadbeef", "Bearer 0", "Bearer  ", "Bearer 00 11"] {
        let mut request = auth(
            req("GET", &url, String::new()),
            room["creator_id"].as_str().unwrap(),
            room["creator_access"].as_str().unwrap(),
        );
        request
            .headers_mut()
            .insert("authorization", authorization.parse().unwrap());
        assert_eq!(send(&r, request).await.0, StatusCode::NOT_FOUND);
    }
}
#[tokio::test]
async fn browser_origin_is_restricted_but_native_clients_work() {
    let r = app(4);
    let mut hostile = req("POST", "/v1/rooms", String::new());
    hostile
        .headers_mut()
        .insert("origin", "https://evil.example".parse().unwrap());
    hostile
        .headers_mut()
        .insert("host", "127.0.0.1:38421".parse().unwrap());
    assert_eq!(send(&r, hostile).await.0, StatusCode::FORBIDDEN);
    let mut hostile = req("POST", "/v1/rooms", String::new());
    hostile
        .headers_mut()
        .insert("sec-fetch-site", "cross-site".parse().unwrap());
    assert_eq!(send(&r, hostile).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        send(&r, req("POST", "/v1/rooms", String::new())).await.0,
        StatusCode::CREATED
    );
}
#[tokio::test]
async fn rate_bucket_is_bounded_and_reports_retry_after() {
    let manager = Arc::new(
        RoomManager::new(RoomManagerConfig {
            max_rooms: 8,
            max_waiting_lifetime: Duration::from_secs(60),
            max_active_lifetime: Duration::from_secs(60),
            max_invitation_lifetime: Duration::from_secs(60),
            max_cleanup_per_pass: 2,
            max_id_retries: 4,
        })
        .unwrap(),
    );
    let r = router(
        manager,
        ServerLimits {
            max_body_bytes: 512,
            max_in_flight_requests: 2,
            request_timeout: Duration::from_millis(100),
            creates_per_minute: 1,
            mutations_per_minute: 1,
        },
    )
    .unwrap();
    assert_eq!(
        send(&r, req("POST", "/v1/rooms", String::new())).await.0,
        StatusCode::CREATED
    );
    let response = r
        .oneshot(req("POST", "/v1/rooms", String::new()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response.headers().contains_key("retry-after"));
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
}
#[tokio::test]
async fn actual_tcp_slow_body_times_out_then_capacity_recovers_and_health_works() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let manager = Arc::new(
        RoomManager::new(RoomManagerConfig {
            max_rooms: 4,
            max_waiting_lifetime: Duration::from_secs(60),
            max_active_lifetime: Duration::from_secs(60),
            max_invitation_lifetime: Duration::from_secs(60),
            max_cleanup_per_pass: 2,
            max_id_retries: 4,
        })
        .unwrap(),
    );
    let r = router(
        manager,
        ServerLimits {
            max_body_bytes: 512,
            max_in_flight_requests: 1,
            request_timeout: Duration::from_millis(150),
            creates_per_minute: 10,
            mutations_per_minute: 10,
        },
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(listener, r)
            .with_graceful_shutdown(async move {
                let _ = stop_rx.await;
            })
            .await
            .unwrap();
    });
    let mut slow = tokio::net::TcpStream::connect(addr).await.unwrap();
    slow.write_all(b"POST /v1/rooms HTTP/1.1\r\nHost: localhost\r\nContent-Length: 80\r\nConnection: close\r\n\r\n{").await.unwrap();
    let mut timeout_response = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(3),
        slow.read_to_end(&mut timeout_response),
    )
    .await
    .unwrap()
    .unwrap();
    let text = String::from_utf8_lossy(&timeout_response);
    assert!(
        text.contains("408") || text.contains("request_timeout"),
        "{text}"
    );
    let mut active = tokio::net::TcpStream::connect(addr).await.unwrap();
    active.write_all(b"POST /v1/rooms HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), active.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&response).contains("201 Created"));
    let mut health = tokio::net::TcpStream::connect(addr).await.unwrap();
    health
        .write_all(b"GET /health/live HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), health.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&response).contains("200 OK"));
    stop_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
}

#[cfg(unix)]
#[test]
fn actual_process_exits_cleanly_on_sigterm() {
    use std::io::{BufRead, Read, Write};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    let bin = env!("CARGO_BIN_EXE_ghostroom-server");
    let mut process = Command::new(bin)
        .env("GHOSTROOM_BIND", "127.0.0.1:0")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut log = std::io::BufReader::new(process.stderr.take().unwrap());
    let mut first = String::new();
    log.read_line(&mut first).unwrap();
    assert!(first.contains("listening on "), "{first}");
    let addr = first.split_whitespace().last().unwrap();
    let mut conn = std::net::TcpStream::connect(addr).unwrap();
    conn.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    conn.write_all(b"GET /health/live HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut bytes = Vec::new();
    conn.read_to_end(&mut bytes).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("200 OK"));
    let signalled = Command::new("kill")
        .arg("-TERM")
        .arg(process.id().to_string())
        .status()
        .unwrap();
    assert!(signalled.success());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(process.wait());
    });
    let exited = rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    assert!(
        exited.success(),
        "SIGTERM should trigger clean exit: {exited}"
    );
}
