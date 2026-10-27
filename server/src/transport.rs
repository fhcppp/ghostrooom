//! HTTP/1-only, bounded TCP transport for the loopback control API.
//!
//! Unlike handler middleware, these limits apply before an HTTP request is
//! fully parsed. Only the accept loop owns connection permits, and it never
//! waits for a permit after accepting a socket.

use axum::{body::Body, http::Request, Router};
use hyper::{body::Incoming, server::conn::http1, service::service_fn};
use hyper_util::rt::{TokioIo, TokioTimer};
use std::{future::Future, io, sync::Arc, time::Duration};
use tokio::{
    net::TcpListener,
    sync::{watch, Semaphore},
    task::JoinSet,
    time,
};
use tower::ServiceExt;

/// All values are bounded at startup; defaults live in the server binary.
#[derive(Clone, Copy, Debug)]
pub struct TransportLimits {
    pub max_connections: usize,
    pub header_timeout: Duration,
    pub max_connection_age: Duration,
    pub max_header_bytes: usize,
    pub max_headers: usize,
}

impl TransportLimits {
    pub fn validate(self) -> Result<Self, &'static str> {
        if !(1..=4096).contains(&self.max_connections) {
            return Err("max connections must be in 1..=4096");
        }
        if self.header_timeout < Duration::from_millis(100)
            || self.header_timeout > Duration::from_secs(30)
        {
            return Err("header timeout must be in 100ms..=30s");
        }
        if self.max_connection_age < Duration::from_secs(2)
            || self.max_connection_age > Duration::from_secs(3600)
            || self.max_connection_age < self.header_timeout
        {
            return Err("connection lifetime must be in 2..=3600s and >= header timeout");
        }
        // Hyper requires at least 8192 for its parser buffer.
        if !(8192..=32768).contains(&self.max_header_bytes) {
            return Err("max header bytes must be in 8192..=32768");
        }
        if !(1..=100).contains(&self.max_headers) {
            return Err("max headers must be in 1..=100");
        }
        Ok(self)
    }
}

async fn connection(
    stream: tokio::net::TcpStream,
    app: Router,
    limits: TransportLimits,
    mut stop: watch::Receiver<bool>,
) {
    let mut http = http1::Builder::new();
    http.keep_alive(true)
        .half_close(false)
        .ignore_invalid_headers(false)
        .max_buf_size(limits.max_header_bytes)
        .max_header_size(limits.max_header_bytes)
        .max_headers(limits.max_headers)
        .timer(TokioTimer::new())
        .header_read_timeout(limits.header_timeout);

    // Router::oneshot handles readiness without sharing a mutable Router
    // between sockets. Hyper owns parsing and request-body framing.
    let service = service_fn(move |request: Request<Incoming>| {
        let app = app.clone();
        async move { app.oneshot(request.map(Body::new)).await }
    });
    let mut conn = Box::pin(http.serve_connection(TokioIo::new(stream), service));
    tokio::select! {
        biased;
        _ = stop.changed() => {
            // Stop admitting new HTTP requests, then drain current work.
            conn.as_mut().graceful_shutdown();
            let _ = time::timeout(Duration::from_secs(10), conn).await;
        }
        _ = time::sleep(limits.max_connection_age) => {
            // Absolute deadline, including periodically trickled bytes.
            // Dropping the connection future closes its socket.
        }
        _ = &mut conn => {
            // Parser, peer, and disconnect failures are connection-local.
            // Never log a request, URL, header, or bearer value.
        }
    }
}

/// Accepts at most `max_connections` sockets concurrently. When saturated,
/// accepting pauses: new TCP handshakes are handled by the OS's finite listen
/// backlog (not an application queue). Every spawned worker owns one permit.
///
/// Shutdown stops accepting immediately; active connections are asked to
/// drain and are force-cancelled after at most ten seconds.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    limits: TransportLimits,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    let limits = limits
        .validate()
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "transport accepts loopback listeners only",
        ));
    }
    let permits = Arc::new(Semaphore::new(limits.max_connections));
    let (stop_tx, stop_rx) = watch::channel(false);
    let mut tasks = JoinSet::new();
    tokio::pin!(shutdown);

    loop {
        // Reap completed tasks as they accumulate. JoinSet is bounded by the
        // number of admitted sockets, not by lifetime connection count.
        while let Some(joined) = tasks.try_join_next() {
            if joined.is_err() {
                eprintln!("GhostRoom transport: connection worker stopped unexpectedly");
            }
        }
        tokio::select! {
            biased;
            _ = &mut shutdown => break,
            joined = tasks.join_next(), if !tasks.is_empty() => {
                if joined.is_some_and(|result| result.is_err()) {
                    eprintln!("GhostRoom transport: connection worker stopped unexpectedly");
                }
            }
            permit = permits.clone().acquire_owned() => {
                let permit = permit.map_err(|_| io::Error::other("connection limiter closed"))?;
                let accepted = tokio::select! {
                    biased;
                    _ = &mut shutdown => break,
                    result = listener.accept() => result,
                };
                match accepted {
                    Ok((stream, _peer)) => {
                        let app = app.clone();
                        let stop = stop_rx.clone();
                        tasks.spawn(async move {
                            let _permit = permit;
                            connection(stream, app, limits, stop).await;
                        });
                    }
                    Err(_) => {
                        // Temporary accept errors must not crash the service
                        // or turn into a CPU-burning retry loop.
                        time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }

    let _ = stop_tx.send(true);
    if time::timeout(Duration::from_secs(10), async {
        while let Some(joined) = tasks.join_next().await {
            if joined.is_err() {
                eprintln!("GhostRoom transport: connection worker stopped unexpectedly");
            }
        }
    })
    .await
    .is_err()
    {
        // Do not wait without a deadline for an uncooperative task. Dropping
        // JoinSet aborts any workers still pending; cancellation releases
        // their permits as soon as the runtime polls them.
        tasks.abort_all();
    }
    Ok(())
}
