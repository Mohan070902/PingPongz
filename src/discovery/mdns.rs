use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::discovery::udp::DiscoveredPeerEvent;

pub struct MdnsDiscoveryService;

impl MdnsDiscoveryService {
    /// FR-PD-01: Advertise self and discover peers using mDNS service type _rubix._tcp.local
    pub fn start(
        service_type: &str,
        tcp_port: u16,
        fingerprint: String,
        hostname: String,
        peer_tx: mpsc::Sender<DiscoveredPeerEvent>,
    ) {
        let mdns = match ServiceDaemon::new() {
            Ok(d) => Arc::new(d),
            Err(e) => {
                warn!("mDNS daemon failed to initialize: {}. Continuing with UDP fallback.", e);
                return;
            }
        };

        // 1. Register self
        let full_service_type = if service_type.ends_with('.') {
            service_type.to_string()
        } else {
            format!("{}.", service_type)
        };

        let instance_name = format!("rubix-{}", fingerprint);
        let host_name = format!("{}.local.", hostname.replace(' ', "-"));

        let mut properties = HashMap::new();
        properties.insert("fingerprint".to_string(), fingerprint.clone());
        properties.insert("hostname".to_string(), hostname);

        let service_info = ServiceInfo::new(
            &full_service_type,
            &instance_name,
            &host_name,
            "",
            tcp_port,
            properties,
        );

        match service_info {
            Ok(info) => {
                if let Err(e) = mdns.register(info) {
                    warn!("mDNS registration failed: {}", e);
                } else {
                    info!("mDNS registered service: {} on port {}", instance_name, tcp_port);
                }
            }
            Err(e) => {
                warn!("mDNS ServiceInfo creation failed: {}", e);
            }
        }

        // 2. Browse for peers
        let mdns_browse = Arc::clone(&mdns);
        let my_fingerprint = fingerprint;

        tokio::task::spawn_blocking(move || {
            let receiver = match mdns_browse.browse(&full_service_type) {
                Ok(r) => r,
                Err(e) => {
                    warn!("mDNS browse failed: {}", e);
                    return;
                }
            };

            while let Ok(event) = receiver.recv() {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let mut peer_fingerprint = String::new();
                    let mut peer_hostname = String::new();

                    for property in info.get_properties().iter() {
                        let key = property.key();
                        let val = property.val_str();
                        if key == "fingerprint" {
                            peer_fingerprint = val.to_string();
                        } else if key == "hostname" {
                            peer_hostname = val.to_string();
                        }
                    }

                    if peer_fingerprint.is_empty() || peer_fingerprint == my_fingerprint {
                        continue;
                    }

                    if peer_hostname.is_empty() {
                        peer_hostname = info.get_hostname().trim_end_matches('.').to_string();
                    }

                    let port = info.get_port();
                    for ip in info.get_addresses() {
                        if ip.is_ipv4() {
                            let event = DiscoveredPeerEvent {
                                fingerprint: peer_fingerprint.clone(),
                                hostname: peer_hostname.clone(),
                                ip: ip.to_string(),
                                tcp_port: port,
                            };
                            let _ = peer_tx.blocking_send(event);
                            break;
                        }
                    }
                }
            }
        });
    }
}
