//! GhostRoom HTTP control plane: loopback-only, no message transfer.
use ghostroom_core::{CleanupStats, ManagerError, RoomManager, RoomManagerConfig};
use ghostroom_server::{
    router,
    transport::{self, TransportLimits},
    validate_listener, ServerLimits,
};
use std::{env, error::Error, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    sync::{oneshot, watch},
    time,
};

/// A failed cleanup pass is a terminal service condition: notify the
/// supervisor exactly once and stop maintenance instead of hiding the error.
/// Never include room IDs, capabilities, or manager details in diagnostics.
fn maintenance_continue(
    result: Result<CleanupStats, ManagerError>,
    fatal: &mut Option<oneshot::Sender<()>>,
) -> bool {
    if result.is_ok() {
        true
    } else {
        eprintln!("GhostRoom fatal: maintenance failed; shutting down");
        if let Some(tx) = fatal.take() {
            let _ = tx.send(());
        }
        false
    }
}

fn config_number(key: &str, default: usize, max: usize) -> Result<usize, Box<dyn Error>> {
    let value = env::var(key)
        .ok()
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(default);
    if value == 0 || value > max {
        return Err(format!("{key}: must be in 1..={max}").into());
    }
    Ok(value)
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let bind: SocketAddr = env::var("GHOSTROOM_BIND")
        .unwrap_or_else(|_| "127.0.0.1:38421".to_owned())
        .parse()?;
    validate_listener(bind)?;
    let max_rooms = config_number("GHOSTROOM_MAX_ROOMS", 128, 4096)?;
    let body = config_number("GHOSTROOM_MAX_BODY_BYTES", 1024, 8192)?;
    let inflight = config_number("GHOSTROOM_MAX_IN_FLIGHT", 32, 1024)?;
    let deadline_ms = config_number("GHOSTROOM_REQUEST_TIMEOUT_MS", 5000, 30000)?;
    if deadline_ms < 50 {
        return Err("GHOSTROOM_REQUEST_TIMEOUT_MS: minimum 50".into());
    }
    let creates = config_number("GHOSTROOM_CREATES_PER_MINUTE", 30, 600)?;
    let mutations = config_number("GHOSTROOM_MUTATIONS_PER_MINUTE", 120, 600)?;
    let cleanup_per_pass = config_number("GHOSTROOM_CLEANUP_BUDGET", 16, 4096)?;
    let transport = TransportLimits {
        max_connections: config_number("GHOSTROOM_MAX_CONNECTIONS", 64, 4096)?,
        header_timeout: Duration::from_millis(
            config_number("GHOSTROOM_HEADER_TIMEOUT_MS", 5000, 30000)? as u64,
        ),
        max_connection_age: Duration::from_secs(
            config_number("GHOSTROOM_MAX_CONNECTION_AGE_S", 60, 3600)? as u64,
        ),
        max_header_bytes: config_number("GHOSTROOM_MAX_HEADER_BYTES", 8192, 32768)?,
        max_headers: config_number("GHOSTROOM_MAX_HEADERS", 64, 100)?,
    }
    .validate()?;
    let manager = Arc::new(RoomManager::new(RoomManagerConfig {
        max_rooms,
        max_waiting_lifetime: Duration::from_secs(600),
        max_active_lifetime: Duration::from_secs(3600),
        max_invitation_lifetime: Duration::from_secs(300),
        max_cleanup_per_pass: cleanup_per_pass,
        max_id_retries: 4,
    })?);
    let app = router(
        Arc::clone(&manager),
        ServerLimits {
            max_body_bytes: body,
            max_in_flight_requests: inflight,
            request_timeout: Duration::from_millis(deadline_ms as u64),
            creates_per_minute: creates as u32,
            mutations_per_minute: mutations as u32,
        },
    )?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    eprintln!(
        "GhostRoom control API listening on {}",
        listener.local_addr()?
    );
    // Shutdown is driven by either the service manager or unrecoverable
    // cleanup failure; a poisoned registry must never silently disable GC.
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let (fatal_tx, mut fatal_rx) = oneshot::channel::<()>();
    let maint = tokio::spawn(async move {
        let mut fatal_tx = Some(fatal_tx);
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                biased;
                changed = shutdown_rx.changed() => {
                    if changed.is_err() || *shutdown_rx.borrow() { break; }
                }
                _ = interval.tick() => {
                    if !maintenance_continue(manager.cleanup(), &mut fatal_tx) { break; }
                }
            }
        }
    });
    #[cfg(unix)]
    let signal = async {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = term.recv() => {},
        }
    };
    #[cfg(not(unix))]
    let signal = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let fatal = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let fatal_in_signal = std::sync::Arc::clone(&fatal);
    let served = transport::serve(listener, app, transport, async move {
        tokio::select! {
            _ = signal => {},
            _ = &mut fatal_rx => {
                fatal_in_signal.store(true, std::sync::atomic::Ordering::SeqCst);
            },
        }
    })
    .await;
    let _ = shutdown_tx.send(true);
    let maintenance = maint.await;
    if fatal.load(std::sync::atomic::Ordering::SeqCst) || maintenance.is_err() {
        return Err("unrecoverable room maintenance failure".into());
    }
    served?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_maintenance_does_not_signal_failure() {
        let (tx, rx) = oneshot::channel();
        let mut tx = Some(tx);
        assert!(maintenance_continue(
            Ok(CleanupStats {
                inspected: 1,
                removed: 1
            }),
            &mut tx
        ));
        assert!(tx.is_some());
        drop(tx);
        assert!(rx.blocking_recv().is_err());
    }

    #[test]
    fn unrecoverable_maintenance_failure_signals_once() {
        let (tx, rx) = oneshot::channel();
        let mut tx = Some(tx);
        assert!(!maintenance_continue(Err(ManagerError::Poisoned), &mut tx));
        assert!(tx.is_none());
        assert!(rx.blocking_recv().is_ok());
        assert!(!maintenance_continue(Err(ManagerError::Poisoned), &mut tx));
    }
}
