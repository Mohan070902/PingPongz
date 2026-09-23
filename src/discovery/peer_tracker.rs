use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use crate::database::database::Database;
use crate::models::{Peer, PeerState, TrustStatus};

pub const MAX_TRACKED_PEERS: usize = 200;

#[derive(Clone)]
pub struct PeerTracker {
    peers: Arc<Mutex<HashMap<String, Peer>>>,
}

impl PeerTracker {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Load persisted peers from database on startup
    pub fn load_from_db(&self, conn: &Connection) -> Result<(), rusqlite::Error> {
        let db_peers = Database::get_all_peers(conn)?;
        let mut map = self.peers.lock().unwrap();
        for mut peer in db_peers {
            // Persisted peers start in Offline state on startup until discovered
            peer.state = PeerState::Offline;
            map.insert(peer.fingerprint.clone(), peer);
        }
        info!("Loaded {} peers from database", map.len());
        Ok(())
    }

    /// Record a discovered peer (via mDNS, UDP, or manual entry)
    pub fn register_peer(
        &self,
        conn: &Connection,
        fingerprint: String,
        hostname: String,
        ip: String,
        port: u16,
    ) -> Result<Option<Peer>, rusqlite::Error> {
        let mut map = self.peers.lock().unwrap();

        // Check if peer already known
        if let Some(existing) = map.get_mut(&fingerprint) {
            existing.last_ip = ip.clone();
            existing.tcp_port = port;
            existing.hostname = hostname.clone();
            existing.last_seen = chrono::Utc::now().timestamp_millis();
            if existing.state == PeerState::Offline {
                existing.state = PeerState::Discovered;
            }
            Database::upsert_peer(conn, existing)?;
            return Ok(Some(existing.clone()));
        }

        // FR-ID-08: Check if known hostname reappears with different fingerprint
        let mut identity_changed_from = None;
        for (_fp, existing) in map.iter() {
            if existing.hostname == hostname && existing.fingerprint != fingerprint {
                warn!(
                    "Identity change detected for hostname '{}': old '{}', new '{}'",
                    hostname, existing.fingerprint, fingerprint
                );
                identity_changed_from = Some(existing.fingerprint.clone());
                break;
            }
        }

        // FR-PD-07: Support tracking up to 200 discovered peers
        if map.len() >= MAX_TRACKED_PEERS {
            // Evict oldest offline peer if at capacity
            let oldest_offline = map
                .iter()
                .filter(|(_, p)| p.state == PeerState::Offline)
                .min_by_key(|(_, p)| p.last_seen)
                .map(|(fp, _)| fp.clone());

            if let Some(evict_fp) = oldest_offline {
                map.remove(&evict_fp);
                info!("Evicted oldest offline peer {} to maintain 200 peer limit", evict_fp);
            } else {
                warn!("Peer limit reached (200) and all peers are active. Ignoring new discovery.");
                return Ok(None);
            }
        }

        let mut new_peer = Peer::new(fingerprint.clone(), hostname, ip, port);
        if let Some(old_fp) = identity_changed_from {
            new_peer.trust_status = TrustStatus::IdentityChanged;
            new_peer.previous_fingerprint = Some(old_fp);
        }

        Database::upsert_peer(conn, &new_peer)?;
        map.insert(fingerprint.clone(), new_peer.clone());

        info!(
            "Registered new peer: {} [{}] at {}:{}",
            new_peer.hostname, new_peer.fingerprint, new_peer.last_ip, new_peer.tcp_port
        );

        Ok(Some(new_peer))
    }

    pub fn get_peer(&self, fingerprint: &str) -> Option<Peer> {
        self.peers.lock().unwrap().get(fingerprint).cloned()
    }

    pub fn set_peer_state(&self, conn: &Connection, fingerprint: &str, state: PeerState) {
        let mut map = self.peers.lock().unwrap();
        if let Some(peer) = map.get_mut(fingerprint) {
            peer.state = state;
            peer.last_seen = chrono::Utc::now().timestamp_millis();
            let _ = Database::update_peer_state(conn, fingerprint, state);
        }
    }

    /// FR-ID-09: User response to Identity Changed warning
    pub fn resolve_identity_change(
        &self,
        conn: &Connection,
        fingerprint: &str,
        trust: bool,
    ) -> Result<(), rusqlite::Error> {
        let mut map = self.peers.lock().unwrap();
        if let Some(peer) = map.get_mut(fingerprint) {
            if trust {
                peer.trust_status = TrustStatus::Trusted;
                info!("User trusted new identity for peer {}", fingerprint);
            } else {
                peer.trust_status = TrustStatus::Blocked;
                peer.state = PeerState::Offline;
                info!("User blocked new identity for peer {}", fingerprint);
            }
            Database::update_peer_trust_status(conn, fingerprint, peer.trust_status)?;
        }
        Ok(())
    }

    /// FR-UI-05: Sort peer list: Online first, then Offline, alphabetically within each group
    pub fn get_sorted_peers(&self) -> Vec<Peer> {
        let map = self.peers.lock().unwrap();
        let mut peer_list: Vec<Peer> = map.values().cloned().collect();

        peer_list.sort_by(|a, b| {
            let a_is_online = a.state == PeerState::Online || a.state == PeerState::Connecting;
            let b_is_online = b.state == PeerState::Online || b.state == PeerState::Connecting;

            match (a_is_online, b_is_online) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    let a_name = a.nickname.as_deref().unwrap_or(&a.hostname);
                    let b_name = b.nickname.as_deref().unwrap_or(&b.hostname);
                    a_name.to_lowercase().cmp(&b_name.to_lowercase())
                }
            }
        });

        peer_list
    }
}
