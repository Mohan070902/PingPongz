#![allow(non_snake_case, dead_code)]

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::info;

use PingPongzzz::app::PingPongApp;
use PingPongzzz::config::app_config::AppConfig;
use PingPongzzz::database::database::Database;
use PingPongzzz::discovery::{MdnsDiscoveryService, PeerTracker, UdpDiscoveryService};
use PingPongzzz::identity::IdentityManager;
use PingPongzzz::network::ConnectionManager;
use PingPongzzz::notifications::Notifier;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(target_os = "linux")]
    {
        // Automatically normalize WSLg display environment for seamless desktop GUI rendering
        if std::env::var("WSL_DISTRO_NAME").is_ok() || std::env::var("WSL_INTEROP").is_ok() {
            std::env::remove_var("WAYLAND_DISPLAY");
            std::env::set_var("WINIT_UNIX_BACKEND", "x11");
        }
        if std::env::var("LIBGL_ALWAYS_SOFTWARE").is_err() {
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
        }
    }

    // 1. Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("=================================");
    info!("   Rubix – PingPongzzz v1.0.0");
    info!("=================================");

    // 2. Initialize Tokio async runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let _guard = rt.enter();

    // 3. Load App Configuration
    let config = AppConfig::new();

    // 4. Initialize Database (FR-DB-01, FR-DB-06, FR-DB-07)
    let connection = Database::initialize()?;
    let db = Arc::new(Mutex::new(connection));

    // 5. Initialize Identity (FR-ID-01 to FR-ID-06)
    let identity = {
        let conn = db.lock().unwrap();
        IdentityManager::load_or_create(&conn)?
    };

    // 6. Initialize Peer Tracker & load persisted peers (FR-PC-02, FR-PD-07)
    let peer_tracker = PeerTracker::new();
    {
        let conn = db.lock().unwrap();
        peer_tracker.load_from_db(&conn)?;
    }

    // 7. Channels for background network events and peer discovery
    let (event_tx, event_rx) = mpsc::channel(64);
    let (peer_tx, peer_rx) = mpsc::channel(64);

    // 8. Initialize Connection Manager & start TCP listener / heartbeats (FR-PC-01 to FR-PC-08)
    let conn_mgr = ConnectionManager::new(identity.clone(), peer_tracker.clone(), event_tx);
    conn_mgr.start(config.tcp_port);

    // 9. Start Discovery Services (mDNS + UDP broadcast fallback) (FR-PD-01 to FR-PD-05)
    MdnsDiscoveryService::start(
        &config.mdns_service_type,
        config.tcp_port,
        identity.fingerprint(),
        identity.hostname(),
        peer_tx.clone(),
    );

    UdpDiscoveryService::start(
        config.udp_port,
        config.tcp_port,
        identity.fingerprint(),
        identity.hostname(),
        peer_tx,
    );

    // 10. Notifier for desktop alerts (FR-NOT-01 to FR-NOT-03)
    let notifier = Notifier::new();

    // 11. Launch eframe GUI window (FR-UI-01 to FR-UI-05)
    let app = PingPongApp::new(
        config,
        identity,
        db,
        peer_tracker,
        conn_mgr,
        notifier,
        event_rx,
        peer_rx,
    );

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rubix – PingPongzzz")
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Rubix – PingPongzzz",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
    .map_err(|e| format!("GUI launch failed: {}", e).into())
}