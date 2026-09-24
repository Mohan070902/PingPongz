use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

use crate::crypto::noise::{NoiseHandshake, NoiseSession};
use crate::crypto::CryptoKeys;
use crate::discovery::PeerTracker;
use crate::identity::IdentityManager;
use crate::models::{Message, MessageStatus};
use crate::network::framed::{FramedReader, FramedWriter};
use crate::network::protocol::{
    TransportPacket, WireMessagePayload, PACKET_HANDSHAKE_1, PACKET_HANDSHAKE_2,
    PACKET_HANDSHAKE_3, PACKET_TRANSPORT,
};

pub const MAX_ACTIVE_CONNECTIONS: usize = 10;
pub const HEARTBEAT_INTERVAL_SECS: u64 = 10;
pub const MISSED_HEARTBEATS_LIMIT: u32 = 3;
pub const INACTIVITY_TIMEOUT_SECS: u64 = 300; // 5 minutes

struct ActiveConn {
    tx: mpsc::Sender<TransportPacket>,
    last_activity: Instant,
    missed_heartbeats: u32,
}

pub enum NetworkEvent {
    MessageReceived(Message),
    MessageStatusUpdated(String, MessageStatus),
    PeerOnline(String),
    PeerOffline(String),
}

#[derive(Clone)]
pub struct ConnectionManager {
    identity: IdentityManager,
    peer_tracker: PeerTracker,
    connections: Arc<Mutex<HashMap<String, ActiveConn>>>,
    event_tx: mpsc::Sender<NetworkEvent>,
}

impl ConnectionManager {
    pub fn new(
        identity: IdentityManager,
        peer_tracker: PeerTracker,
        event_tx: mpsc::Sender<NetworkEvent>,
    ) -> Self {
        Self {
            identity,
            peer_tracker,
            connections: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        }
    }

    /// Start the background tasks: TCP Listener, 10s Heartbeat loop, and 5-min Inactivity reaper
    pub fn start(&self, tcp_port: u16) {
        let mgr = self.clone();

        // 1. TCP Listener
        tokio::spawn(async move {
            let addr = format!("0.0.0.0:{}", tcp_port);
            let listener = match TcpListener::bind(&addr).await {
                Ok(l) => {
                    info!("TCP listener bound on {}", addr);
                    l
                }
                Err(e) => {
                    error!("Failed to bind TCP listener on {}: {}", addr, e);
                    return;
                }
            };

            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        info!("Incoming connection from {}", peer_addr);
                        let mgr_clone = mgr.clone();
                        tokio::spawn(async move {
                            if let Err(e) = mgr_clone.handle_incoming_connection(stream).await {
                                warn!("Error handling incoming connection from {}: {}", peer_addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("TCP accept error: {}", e);
                    }
                }
            }
        });

        // 2. Heartbeat loop (every 10s) and Inactivity reaper (5 mins)
        let mgr_hb = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
            loop {
                interval.tick().await;
                mgr_hb.tick_heartbeat_and_inactivity().await;
            }
        });
    }

    /// FR-PC-07 & FR-PC-08 & FR-PC-05
    async fn tick_heartbeat_and_inactivity(&self) {
        let mut conns = self.connections.lock().await;
        let mut to_drop = Vec::new();

        let now = Instant::now();

        for (fp, conn) in conns.iter_mut() {
            // FR-PC-05: Inactivity timeout (5 minutes)
            if now.duration_since(conn.last_activity) > Duration::from_secs(INACTIVITY_TIMEOUT_SECS) {
                info!("Closing idle connection for peer {} after 5 minutes of inactivity", fp);
                to_drop.push(fp.clone());
                continue;
            }

            // FR-PC-07: Send heartbeat ping every 10s
            conn.missed_heartbeats += 1;
            if conn.missed_heartbeats > MISSED_HEARTBEATS_LIMIT {
                // FR-PC-08: Mark peer offline after 3 consecutive missed heartbeats
                warn!("Peer {} missed {} consecutive heartbeats. Marking offline.", fp, MISSED_HEARTBEATS_LIMIT);
                to_drop.push(fp.clone());
            } else {
                let _ = conn.tx.send(TransportPacket::Ping).await;
            }
        }

        for fp in to_drop {
            conns.remove(&fp);
            let _ = self.event_tx.send(NetworkEvent::PeerOffline(fp)).await;
        }
    }

    /// FR-PC-06: LRU Eviction when 11th connection is required
    async fn ensure_connection_capacity(&self, conns: &mut HashMap<String, ActiveConn>) {
        if conns.len() >= MAX_ACTIVE_CONNECTIONS {
            // Find LRU active connection
            if let Some((lru_fp, _)) = conns
                .iter()
                .min_by_key(|(_, c)| c.last_activity)
                .map(|(k, c)| (k.clone(), c.last_activity))
            {
                info!("LRU eviction: closing connection for peer {} to stay within limit of 10", lru_fp);
                conns.remove(&lru_fp);
                let _ = self.event_tx.send(NetworkEvent::PeerOffline(lru_fp)).await;
            }
        }
    }

    /// FR-PC-04: Open a connection to a peer only when the user opens that peer's chat window
    pub async fn connect_to_peer_if_needed(
        &self,
        peer_fingerprint: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        {
            let conns = self.connections.lock().await;
            if conns.contains_key(peer_fingerprint) {
                return Ok(true); // Already active
            }
        }

        let peer = match self.peer_tracker.get_peer(peer_fingerprint) {
            Some(p) => p,
            None => return Err(format!("Peer not found: {}", peer_fingerprint).into()),
        };

        let target_addr = format!("{}:{}", peer.last_ip, peer.tcp_port);
        info!("Initiating on-demand connection to {} at {}", peer_fingerprint, target_addr);

        let stream = match tokio::time::timeout(
            Duration::from_secs(4),
            TcpStream::connect(&target_addr),
        )
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(format!("TCP connect error to {}: {}", target_addr, e).into()),
            Err(_) => return Err(format!("TCP connect timeout to {}", target_addr).into()),
        };

        self.handle_outbound_connection(stream, peer_fingerprint).await?;
        Ok(true)
    }

    /// Perform initiator Noise handshake
    async fn handle_outbound_connection(
        &self,
        stream: TcpStream,
        expected_fingerprint: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (reader, writer) = stream.into_split();
        let mut framed_reader = FramedReader::new(reader);
        let mut framed_writer = FramedWriter::new(writer);

        let my_x25519_priv = self.identity.x25519_secret_bytes();
        let mut handshake = NoiseHandshake::new_initiator(&my_x25519_priv)?;

        // Step 1: Initiator -> Responder (msg 1: -> e)
        let msg1 = handshake.write_message(&[])?;
        let mut packet1 = vec![PACKET_HANDSHAKE_1];
        packet1.extend_from_slice(&msg1);
        framed_writer.write_frame(&packet1).await?;

        // Step 2: Responder -> Initiator (msg 2: <- e, ee, s, es)
        let packet2 = framed_reader.read_frame().await?;
        if packet2.is_empty() || packet2[0] != PACKET_HANDSHAKE_2 {
            return Err("Expected Handshake 2 packet from responder".into());
        }
        let remote_payload = handshake.read_message(&packet2[1..])?;

        // Remote payload carries remote Ed25519 pubkey (32 bytes) + signature (64 bytes)
        if remote_payload.len() < 96 {
            return Err("Responder payload missing identity keys".into());
        }
        let mut remote_ed25519 = [0u8; 32];
        remote_ed25519.copy_from_slice(&remote_payload[0..32]);
        let remote_fp = CryptoKeys::fingerprint_from_bytes(&remote_ed25519);

        if remote_fp != expected_fingerprint {
            return Err(format!(
                "Fingerprint mismatch! Expected {}, got {}",
                expected_fingerprint, remote_fp
            )
            .into());
        }

        // Step 3: Initiator -> Responder (msg 3: -> s, se)
        // Include our Ed25519 pubkey (32 bytes) + signature over our X25519 pubkey (64 bytes)
        let my_ed25519 = self.identity.ed25519_verifying_key_bytes();
        let my_x25519_pub = self.identity.x25519_public_bytes();
        let sig = self.identity.sign(&my_x25519_pub);

        let mut my_payload = Vec::with_capacity(96);
        my_payload.extend_from_slice(&my_ed25519);
        my_payload.extend_from_slice(&sig);

        let msg3 = handshake.write_message(&my_payload)?;
        let mut packet3 = vec![PACKET_HANDSHAKE_3];
        packet3.extend_from_slice(&msg3);
        framed_writer.write_frame(&packet3).await?;

        let session = handshake.into_session()?;
        info!("Noise_XX handshake completed as initiator with peer {}", remote_fp);

        self.spawn_session_loop(remote_fp, session, framed_reader, framed_writer).await;
        Ok(())
    }

    /// Perform responder Noise handshake
    async fn handle_incoming_connection(
        &self,
        stream: TcpStream,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (reader, writer) = stream.into_split();
        let mut framed_reader = FramedReader::new(reader);
        let mut framed_writer = FramedWriter::new(writer);

        let my_x25519_priv = self.identity.x25519_secret_bytes();
        let mut handshake = NoiseHandshake::new_responder(&my_x25519_priv)?;

        // Step 1: Read Handshake 1 (-> e)
        let packet1 = framed_reader.read_frame().await?;
        if packet1.is_empty() || packet1[0] != PACKET_HANDSHAKE_1 {
            return Err("Expected Handshake 1 packet".into());
        }
        handshake.read_message(&packet1[1..])?;

        // Step 2: Write Handshake 2 (<- e, ee, s, es)
        let my_ed25519 = self.identity.ed25519_verifying_key_bytes();
        let my_x25519_pub = self.identity.x25519_public_bytes();
        let sig = self.identity.sign(&my_x25519_pub);

        let mut my_payload = Vec::with_capacity(96);
        my_payload.extend_from_slice(&my_ed25519);
        my_payload.extend_from_slice(&sig);

        let msg2 = handshake.write_message(&my_payload)?;
        let mut packet2 = vec![PACKET_HANDSHAKE_2];
        packet2.extend_from_slice(&msg2);
        framed_writer.write_frame(&packet2).await?;

        // Step 3: Read Handshake 3 (-> s, se)
        let packet3 = framed_reader.read_frame().await?;
        if packet3.is_empty() || packet3[0] != PACKET_HANDSHAKE_3 {
            return Err("Expected Handshake 3 packet".into());
        }
        let remote_payload = handshake.read_message(&packet3[1..])?;

        if remote_payload.len() < 96 {
            return Err("Initiator payload missing identity keys".into());
        }
        let mut remote_ed25519 = [0u8; 32];
        remote_ed25519.copy_from_slice(&remote_payload[0..32]);
        let remote_fp = CryptoKeys::fingerprint_from_bytes(&remote_ed25519);

        let session = handshake.into_session()?;
        info!("Noise_XX handshake completed as responder with peer {}", remote_fp);

        self.spawn_session_loop(remote_fp, session, framed_reader, framed_writer).await;
        Ok(())
    }

    /// Register session and start read/write loops
    async fn spawn_session_loop(
        &self,
        peer_fingerprint: String,
        mut session: NoiseSession,
        mut reader: FramedReader,
        mut writer: FramedWriter,
    ) {
        let (out_tx, mut out_rx) = mpsc::channel::<TransportPacket>(32);

        {
            let mut conns = self.connections.lock().await;
            self.ensure_connection_capacity(&mut conns).await;
            conns.insert(
                peer_fingerprint.clone(),
                ActiveConn {
                    tx: out_tx.clone(),
                    last_activity: Instant::now(),
                    missed_heartbeats: 0,
                },
            );
        }

        let _ = self
            .event_tx
            .send(NetworkEvent::PeerOnline(peer_fingerprint.clone()))
            .await;

        let conns_clone = Arc::clone(&self.connections);
        let event_tx_clone = self.event_tx.clone();
        let fp_clone = peer_fingerprint.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Outgoing packet to encrypt and send
                    Some(packet) = out_rx.recv() => {
                        let serialized = match packet.to_bytes() {
                            Ok(b) => b,
                            Err(e) => {
                                error!("Failed to serialize transport packet: {}", e);
                                continue;
                            }
                        };

                        let encrypted = match session.encrypt(&serialized) {
                            Ok(e) => e,
                            Err(e) => {
                                error!("Noise encryption failed: {}", e);
                                break;
                            }
                        };

                        let mut frame = vec![PACKET_TRANSPORT];
                        frame.extend_from_slice(&encrypted);

                        if let Err(e) = writer.write_frame(&frame).await {
                            error!("TCP write failed to peer {}: {}", fp_clone, e);
                            break;
                        }
                    }

                    // Incoming packet to read and decrypt
                    result = reader.read_frame() => {
                        match result {
                            Ok(frame) => {
                                if frame.is_empty() || frame[0] != PACKET_TRANSPORT {
                                    continue;
                                }

                                let decrypted = match session.decrypt(&frame[1..]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        error!("Noise decryption failed from {}: {}", fp_clone, e);
                                        break;
                                    }
                                };

                                let packet = match TransportPacket::from_bytes(&decrypted) {
                                    Ok(p) => p,
                                    Err(e) => {
                                        error!("Failed to parse transport packet from {}: {}", fp_clone, e);
                                        continue;
                                    }
                                };

                                // Update last activity and reset missed heartbeats
                                {
                                    let mut conns = conns_clone.lock().await;
                                    if let Some(conn) = conns.get_mut(&fp_clone) {
                                        conn.last_activity = Instant::now();
                                        conn.missed_heartbeats = 0;
                                    }
                                }

                                match packet {
                                    TransportPacket::Ping => {
                                        // Respond with Pong
                                        let _ = out_tx.send(TransportPacket::Pong).await;
                                    }
                                    TransportPacket::Pong => {
                                        // Acknowledged, activity already updated
                                    }
                                    TransportPacket::Message(wire_msg) => {
                                        if let Ok(msg) = Message::new_inbound(
                                            wire_msg.msg_id,
                                            fp_clone.clone(),
                                            wire_msg.sender_pubkey,
                                            wire_msg.content,
                                            wire_msg.timestamp_ms,
                                        ) {
                                            let _ = event_tx_clone
                                                .send(NetworkEvent::MessageReceived(msg))
                                                .await;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                info!("Connection closed for peer {}: {}", fp_clone, e);
                                break;
                            }
                        }
                    }
                }
            }

            // Cleanup on disconnect
            {
                let mut conns = conns_clone.lock().await;
                conns.remove(&fp_clone);
            }
            let _ = event_tx_clone
                .send(NetworkEvent::PeerOffline(fp_clone))
                .await;
        });
    }

    /// FR-MSG-07 & FR-MSG-08: Send message immediately if online; reject immediately if offline
    pub async fn send_message(
        &self,
        peer_fingerprint: &str,
        message: Message,
    ) -> Result<(), &'static str> {
        let msg_id = message.msg_id.clone();

        // If not yet connected, attempt on-demand connection first
        let is_connected = {
            let conns = self.connections.lock().await;
            conns.contains_key(peer_fingerprint)
        };

        if !is_connected {
            let _ = self.connect_to_peer_if_needed(peer_fingerprint).await;
        }

        let conns = self.connections.lock().await;
        if let Some(conn) = conns.get(peer_fingerprint) {
            let payload = WireMessagePayload {
                msg_id: message.msg_id,
                sender_pubkey: message.sender_pubkey,
                timestamp_ms: message.timestamp_ms,
                content: message.content,
            };

            let res = conn
                .tx
                .send(TransportPacket::Message(payload))
                .await
                .map_err(|_| "Failed to write to peer channel");

            let status = match res {
                Ok(()) => MessageStatus::Sent,
                Err(_) => MessageStatus::Failed,
            };

            let _ = self
                .event_tx
                .send(NetworkEvent::MessageStatusUpdated(msg_id, status))
                .await;

            res
        } else {
            // FR-MSG-08: If the recipient peer is Offline, reject the send immediately
            // with no queueing, no offline storage, and no auto-retry.
            let _ = self
                .event_tx
                .send(NetworkEvent::MessageStatusUpdated(
                    msg_id,
                    MessageStatus::Failed,
                ))
                .await;
            Err("Recipient peer is offline")
        }
    }

    pub async fn is_peer_online(&self, peer_fingerprint: &str) -> bool {
        self.connections.lock().await.contains_key(peer_fingerprint)
    }
}
