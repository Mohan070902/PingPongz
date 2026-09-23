use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::discovery::beacon::DiscoveryBeacon;

pub struct DiscoveredPeerEvent {
    pub fingerprint: String,
    pub hostname: String,
    pub ip: String,
    pub tcp_port: u16,
}

pub struct UdpDiscoveryService;

impl UdpDiscoveryService {
    /// FR-PD-02 & FR-PD-05: Start UDP broadcaster and listener
    pub fn start(
        udp_port: u16,
        tcp_port: u16,
        fingerprint: String,
        hostname: String,
        peer_tx: mpsc::Sender<DiscoveredPeerEvent>,
    ) {
        // Task 1: Receiver loop
        let peer_tx_clone = peer_tx.clone();
        let my_fingerprint = fingerprint.clone();

        tokio::spawn(async move {
            let bind_addr = format!("0.0.0.0:{}", udp_port);
            let socket = match UdpSocket::bind(&bind_addr).await {
                Ok(s) => {
                    info!("UDP discovery listening on {}", bind_addr);
                    s
                }
                Err(e) => {
                    // FR-PD-05: Log warning and continue operating without failing startup
                    warn!(
                        "UDP discovery unavailable: could not bind {}: {}. Continuing via mDNS/manual entry.",
                        bind_addr, e
                    );
                    return;
                }
            };

            let socket = Arc::new(socket);
            let mut buf = vec![0u8; 1024];

            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, src_addr)) => {
                        // FR-PD-04: Validate beacon; ignore malformed or foreign packets
                        match DiscoveryBeacon::from_bytes(&buf[..len]) {
                            Ok(beacon) => {
                                // Ignore self beacon
                                if beacon.fingerprint == my_fingerprint {
                                    continue;
                                }

                                let ip = src_addr.ip().to_string();
                                let event = DiscoveredPeerEvent {
                                    fingerprint: beacon.fingerprint,
                                    hostname: beacon.hostname,
                                    ip,
                                    tcp_port: beacon.tcp_port,
                                };

                                let _ = peer_tx_clone.send(event).await;
                            }
                            Err(e) => {
                                // FR-PD-04: Discard silently or at trace level
                                tracing::trace!("Discarded invalid UDP beacon from {}: {}", src_addr, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("UDP recv error: {}", e);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        });

        // Task 2: Broadcaster loop
        tokio::spawn(async move {
            // Bind ephemeral UDP socket for sending
            let send_socket = match UdpSocket::bind("0.0.0.0:0").await {
                Ok(s) => {
                    let _ = s.set_broadcast(true);
                    s
                }
                Err(e) => {
                    warn!("Failed to create UDP broadcast sender socket: {}", e);
                    return;
                }
            };

            let broadcast_target = format!("255.255.255.255:{}", udp_port);
            let beacon = DiscoveryBeacon::new(tcp_port, fingerprint, hostname);

            let beacon_bytes = match beacon.to_bytes() {
                Ok(b) => b,
                Err(e) => {
                    error!("Failed to serialize UDP beacon: {}", e);
                    return;
                }
            };

            let mut interval = tokio::time::interval(Duration::from_secs(3));
            loop {
                interval.tick().await;
                if let Err(e) = send_socket
                    .send_to(&beacon_bytes, &broadcast_target)
                    .await
                {
                    tracing::debug!("UDP broadcast tick failed (network may be offline): {}", e);
                }
            }
        });
    }
}
