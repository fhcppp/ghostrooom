//! Real TCP behavior of the bounded HTTP/1 connection layer.
use ghostroom_core::{RoomManager, RoomManagerConfig};
use ghostroom_server::{
    router,
    transport::{self, TransportLimits},
    ServerLimits,
};
use std::{io, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
    time,
};

fn limits() -> TransportLimits {
    TransportLimits {
        max_connections: 2,
        header_timeout: Duration::from_millis(150),
        max_connection_age: Duration::from_secs(2),
        max_header_bytes: 8192,
        max_headers: 64,
    }
}

async fn start(
    transport_limits: TransportLimits,
) -> (
    SocketAddr,
    oneshot::Sender<()>,
    JoinHandle<io::Result<()>>,
) {
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
    let app = router(
        manager,
        ServerLimits {
            max_body_bytes: 512,
            max_in_flight_requests: 4,
            request_timeout: Duration::from_millis(250),
            creates_per_minute: 600,
            mutations_per_minute: 600,
        },
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let join = tokio::spawn(async move {
        transport::serve(listener, app, transport_limits, async move {
            let _ = rx.await;
        })
        .await
    });
    (address, tx, join)
}

async fn stop(sender: oneshot::Sender<()>, task: JoinHandle<io::Result<()>>) {
    sender.send(()).unwrap();
    time::timeout(Duration::from_secs(3), task)
        .await
        .expect("server shutdown exceeded bound")
        .expect("server task panicked")
        .expect("server returned an I/O error");
}

async fn status_line(stream: &mut TcpStream) -> String {
    let mut data = Vec::new();
    let mut buf = [0u8; 256];
    while !data.windows(2).any(|bytes| bytes == b"\r\n") && data.len() < 2048 {
        let n = time::timeout(Duration::from_secs(2), stream.read(&mut buf))
            .await
            .expect("no HTTP response")
            .unwrap();
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }
    String::from_utf8_lossy(&data).into_owned()
}

async fn request(address: SocketAddr, bytes: &[u8]) -> String {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(bytes).await.unwrap();
    status_line(&mut stream).await
}

const HEALTH: &[u8] = b"GET /health/live HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
const KEEP_ALIVE: &[u8] = b"GET /health/live HTTP/1.1\r\nHost: localhost\r\n\r\n";

#[tokio::test]
async fn normal_http_request() {
    let (addr, tx, task) = start(limits()).await;
    assert!(request(addr, HEALTH).await.contains("200 OK"));
    stop(tx, task).await;
}

#[tokio::test]
async fn silent_client_does_not_hold_connection_forever() {
    let (addr, tx, task) = start(limits()).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut all = Vec::new();
    time::timeout(Duration::from_secs(2), stream.read_to_end(&mut all))
        .await
        .expect("incomplete headers were not bounded")
        .unwrap();
    stop(tx, task).await;
}

#[tokio::test]
async fn trickled_headers_do_not_reset_absolute_header_deadline() {
    let (addr, tx, task) = start(limits()).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(b"GET /health/live HTTP/1.1\r\nHost: ")
        .await
        .unwrap();
    // Deliberately trickle bytes below the timeout cadence. The deadline
    // must remain absolute instead of restarting on each network read.
    for _ in 0..5 {
        time::sleep(Duration::from_millis(35)).await;
        if stream.write_all(b"a").await.is_err() {
            break;
        }
    }
    let mut rest = Vec::new();
    time::timeout(Duration::from_secs(2), stream.read_to_end(&mut rest))
        .await
        .expect("slow header connection remained open")
        .unwrap();
    assert!(!String::from_utf8_lossy(&rest).contains("200 OK"));
    stop(tx, task).await;
}

#[tokio::test]
async fn oversized_and_excessive_headers_are_rejected_by_hyper() {
    let (addr, tx, task) = start(limits()).await;
    let huge = format!(
        "GET /health/live HTTP/1.1\r\nHost: localhost\r\nX-Pad: {}\r\n\r\n",
        "x".repeat(9000)
    );
    let huge_result = request(addr, huge.as_bytes()).await;
    assert!(huge_result.contains("431"), "{huge_result}");

    let mut many = "GET /health/live HTTP/1.1\r\nHost: localhost\r\n".to_owned();
    for _ in 0..65 {
        many.push_str("X-Test: 1\r\n");
    }
    many.push_str("\r\n");
    let many_result = request(addr, many.as_bytes()).await;
    assert!(many_result.contains("431"), "{many_result}");
    stop(tx, task).await;
}

#[tokio::test]
async fn idle_keep_alive_connection_is_closed() {
    let (addr, tx, task) = start(limits()).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(KEEP_ALIVE).await.unwrap();
    assert!(status_line(&mut stream).await.contains("200 OK"));
    let mut trailing = Vec::new();
    time::timeout(Duration::from_secs(2), stream.read_to_end(&mut trailing))
        .await
        .expect("idle connection stayed open")
        .unwrap();
    stop(tx, task).await;
}

#[tokio::test]
async fn saturation_pauses_accept_and_disconnect_releases_capacity() {
    let mut cfg = limits();
    cfg.max_connections = 1;
    cfg.header_timeout = Duration::from_millis(500);
    let (addr, tx, task) = start(cfg).await;
    let mut stalled = TcpStream::connect(addr).await.unwrap();
    stalled
        .write_all(b"GET /health/live HTTP/1.1\r\nHost: local")
        .await
        .unwrap();
    let mut second = TcpStream::connect(addr).await.unwrap();
    second.write_all(HEALTH).await.unwrap();
    let mut buf = [0u8; 1024];
    assert!(time::timeout(Duration::from_millis(80), second.read(&mut buf))
        .await
        .is_err(), "saturated server unexpectedly served a second connection");
    drop(stalled);
    assert!(status_line(&mut second).await.contains("200 OK"));
    stop(tx, task).await;
}

#[tokio::test]
async fn malformed_peer_does_not_stop_other_connections() {
    let (addr, tx, task) = start(limits()).await;
    let malformed = request(addr, b"THIS IS NOT HTTP\r\n\r\n").await;
    assert!(!malformed.contains("200 OK"));
    assert!(request(addr, HEALTH).await.contains("200 OK"));
    stop(tx, task).await;
}

#[tokio::test]
async fn shutdown_drains_or_cancels_active_and_stalled_sockets() {
    let (addr, tx, task) = start(limits()).await;
    let mut stalled = TcpStream::connect(addr).await.unwrap();
    stalled.write_all(b"GET /health/live HTTP/1.1\r\nH").await.unwrap();
    let mut waiting_body = TcpStream::connect(addr).await.unwrap();
    waiting_body
        .write_all(b"POST /v1/rooms HTTP/1.1\r\nHost: localhost\r\nContent-Length: 9\r\n\r\n{")
        .await
        .unwrap();
    stop(tx, task).await;
    for mut stream in [stalled, waiting_body] {
        let mut bytes = Vec::new();
        time::timeout(Duration::from_secs(2), stream.read_to_end(&mut bytes))
            .await
            .expect("socket remained open after shutdown")
            .unwrap();
    }
}

#[tokio::test]
async fn service_recovers_after_header_timeout() {
    let mut cfg = limits();
    cfg.max_connections = 1;
    let (addr, tx, task) = start(cfg).await;
    let mut stalled = TcpStream::connect(addr).await.unwrap();
    stalled.write_all(b"GET /health/live HTTP/1.1\r\n").await.unwrap();
    let mut bytes = Vec::new();
    time::timeout(Duration::from_secs(2), stalled.read_to_end(&mut bytes))
        .await
        .expect("header timeout did not close connection")
        .unwrap();
    assert!(request(addr, HEALTH).await.contains("200 OK"));
    stop(tx, task).await;
}

#[test]
fn invalid_limits_are_rejected() {
    let mut cfg = limits();
    cfg.max_connections = 0;
    assert!(cfg.validate().is_err());
    cfg = limits();
    cfg.max_header_bytes = 8191;
    assert!(cfg.validate().is_err());
    cfg = limits();
    cfg.max_headers = 101;
    assert!(cfg.validate().is_err());
    cfg = limits();
    cfg.max_connection_age = Duration::from_millis(150);
    assert!(cfg.validate().is_err());
    cfg = limits();
    cfg.header_timeout = Duration::from_secs(3);
    assert!(cfg.validate().is_err());
}
