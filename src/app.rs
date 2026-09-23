use egui::{CentralPanel, SidePanel, TopBottomPanel};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::config::app_config::AppConfig;
use crate::database::database::Database;
use crate::discovery::{DiscoveredPeerEvent, PeerTracker};
use crate::identity::IdentityManager;
use crate::models::{Message, MessageStatus, Peer, PeerState, TrustStatus};
use crate::network::{ConnectionManager, NetworkEvent};
use crate::notifications::Notifier;
use crate::ui::{ChatView, Dialogs, PeerList, TopBar};

pub struct PingPongApp {
    config: AppConfig,
    identity: IdentityManager,
    db: Arc<Mutex<Connection>>,
    peer_tracker: PeerTracker,
    conn_mgr: ConnectionManager,
    notifier: Notifier,
    net_rx: Arc<Mutex<mpsc::Receiver<NetworkEvent>>>,
    discovered_rx: Arc<Mutex<mpsc::Receiver<DiscoveredPeerEvent>>>,

    // UI state
    search_query: String,
    selected_peer_fingerprint: Option<String>,
    active_messages: Vec<Message>,
    input_text: String,

    // Dialogs state
    settings_open: bool,
    settings_nickname_buf: String,

    identity_changed_open: bool,
    identity_changed_peer: Option<Peer>,

    manual_add_open: bool,
    manual_add_ip: String,
    manual_add_fingerprint: String,
}

impl PingPongApp {
    pub fn new(
        config: AppConfig,
        identity: IdentityManager,
        db: Arc<Mutex<Connection>>,
        peer_tracker: PeerTracker,
        conn_mgr: ConnectionManager,
        notifier: Notifier,
        net_rx: mpsc::Receiver<NetworkEvent>,
        discovered_rx: mpsc::Receiver<DiscoveredPeerEvent>,
    ) -> Self {
        let initial_nick = identity.nickname().unwrap_or_default();

        Self {
            config,
            identity,
            db,
            peer_tracker,
            conn_mgr,
            notifier,
            net_rx: Arc::new(Mutex::new(net_rx)),
            discovered_rx: Arc::new(Mutex::new(discovered_rx)),

            search_query: String::new(),
            selected_peer_fingerprint: None,
            active_messages: Vec::new(),
            input_text: String::new(),

            settings_open: false,
            settings_nickname_buf: initial_nick,

            identity_changed_open: false,
            identity_changed_peer: None,

            manual_add_open: false,
            manual_add_ip: String::new(),
            manual_add_fingerprint: String::new(),
        }
    }

    fn reload_active_messages(&mut self) {
        if let Some(ref fp) = self.selected_peer_fingerprint {
            let conn = self.db.lock().unwrap();
            match Database::get_messages_for_peer(&conn, fp) {
                Ok(msgs) => self.active_messages = msgs,
                Err(e) => error!("Failed to load messages for peer {}: {}", fp, e),
            }
        } else {
            self.active_messages.clear();
        }
    }
}

impl eframe::App for PingPongApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let is_window_focused = ctx.input(|i| i.focused);

        // 1. Process discovered peers (from mDNS, UDP, or manual entry)
        if let Ok(mut rx) = self.discovered_rx.try_lock() {
            while let Ok(peer_event) = rx.try_recv() {
                let conn = self.db.lock().unwrap();
                match self.peer_tracker.register_peer(
                    &conn,
                    peer_event.fingerprint.clone(),
                    peer_event.hostname.clone(),
                    peer_event.ip,
                    peer_event.tcp_port,
                ) {
                    Ok(Some(peer)) => {
                        // Check if identity changed warning should trigger (FR-ID-08)
                        if peer.trust_status == TrustStatus::IdentityChanged {
                            self.identity_changed_peer = Some(peer);
                            self.identity_changed_open = true;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => error!("Failed to register peer {}: {}", peer_event.fingerprint, e),
                }
            }
        }

        // 2. Process incoming network events (messages, online/offline state)
        if let Ok(mut rx) = self.net_rx.try_lock() {
            while let Ok(event) = rx.try_recv() {
                match event {
                    NetworkEvent::MessageReceived(msg) => {
                        // Persist to database (FR-DB-04)
                        {
                            let conn = self.db.lock().unwrap();
                            if let Err(e) = Database::insert_message(&conn, &msg) {
                                error!("Failed to save incoming message: {}", e);
                            }
                        }

                        // Determine peer display name
                        let peer_name = self
                            .peer_tracker
                            .get_peer(&msg.peer_fingerprint)
                            .map(|p| p.display_label())
                            .unwrap_or_else(|| msg.peer_fingerprint.clone());

                        // Update active conversation view if open (FR-NOT-01)
                        if self.selected_peer_fingerprint.as_deref()
                            == Some(&msg.peer_fingerprint)
                        {
                            self.active_messages.push(msg.clone());
                        }

                        // Trigger desktop notification if backgrounded (FR-NOT-02, FR-NOT-03)
                        self.notifier.on_message_received(
                            &peer_name,
                            &msg.peer_fingerprint,
                            &msg.content,
                            is_window_focused,
                        );
                    }
                    NetworkEvent::PeerOnline(fp) => {
                        let conn = self.db.lock().unwrap();
                        self.peer_tracker
                            .set_peer_state(&conn, &fp, PeerState::Online);
                    }
                    NetworkEvent::PeerOffline(fp) => {
                        let conn = self.db.lock().unwrap();
                        self.peer_tracker
                            .set_peer_state(&conn, &fp, PeerState::Offline);
                    }
                }
            }
        }

        // Actions to execute after UI closures
        let mut peer_to_select: Option<String> = None;
        let mut identity_change_to_resolve: Option<String> = None;
        let mut text_to_send: Option<String> = None;
        let mut msg_id_to_resend: Option<String> = None;
        let mut save_nickname = false;
        let mut reset_identity = false;
        let mut trust_identity = false;
        let mut block_identity = false;
        let mut add_peer_manually = false;

        // Render Top Bar (FR-UI-01)
        TopBottomPanel::top("top_bar").show(ctx, |ui| {
            TopBar::render(
                ui,
                &mut self.search_query,
                &self.identity.display_label(),
                &mut self.settings_open,
                &mut self.manual_add_open,
            );
        });

        // Render Left Panel (Peer list, FR-UI-02 & FR-UI-05)
        SidePanel::left("peer_list_panel")
            .default_width(260.0)
            .width_range(200.0..=360.0)
            .show(ctx, |ui| {
                let sorted_peers = self.peer_tracker.get_sorted_peers();
                PeerList::render(
                    ui,
                    &sorted_peers,
                    &self.search_query,
                    &self.selected_peer_fingerprint,
                    &mut peer_to_select,
                    &mut identity_change_to_resolve,
                );
            });

        // Render Central Panel (Active conversation, FR-UI-03 & FR-UI-04)
        CentralPanel::default().show(ctx, |ui| {
            let active_peer = self
                .selected_peer_fingerprint
                .as_ref()
                .and_then(|fp| self.peer_tracker.get_peer(fp));

            ChatView::render(
                ui,
                active_peer.as_ref(),
                &self.active_messages,
                &mut self.input_text,
                &mut text_to_send,
                &mut msg_id_to_resend,
            );
        });

        // Dialogs
        Dialogs::render_settings(
            ctx,
            &mut self.settings_open,
            &self.identity.nickname().unwrap_or_default(),
            &self.identity.hostname(),
            &self.identity.fingerprint(),
            &mut self.settings_nickname_buf,
            &mut save_nickname,
            &mut reset_identity,
        );

        if let Some(ref peer) = self.identity_changed_peer {
            let old_fp = peer.previous_fingerprint.as_deref().unwrap_or("UNKNOWN");
            Dialogs::render_identity_changed(
                ctx,
                &mut self.identity_changed_open,
                &peer.hostname,
                old_fp,
                &peer.fingerprint,
                &mut trust_identity,
                &mut block_identity,
            );
        }

        Dialogs::render_manual_add(
            ctx,
            &mut self.manual_add_open,
            &mut self.manual_add_ip,
            &mut self.manual_add_fingerprint,
            &mut add_peer_manually,
        );

        // Handle Peer Selection & On-Demand Connection (FR-PC-04)
        if let Some(fp) = peer_to_select {
            self.selected_peer_fingerprint = Some(fp.clone());
            self.reload_active_messages();

            // Open connection on-demand when chat window opens
            let conn_mgr = self.conn_mgr.clone();
            tokio::spawn(async move {
                let _ = conn_mgr.connect_to_peer_if_needed(&fp).await;
            });
        }

        // Handle Identity Change Alert Click from Peer List
        if let Some(fp) = identity_change_to_resolve {
            if let Some(peer) = self.peer_tracker.get_peer(&fp) {
                self.identity_changed_peer = Some(peer);
                self.identity_changed_open = true;
            }
        }

        // Handle Message Send (FR-MSG-02, FR-MSG-04, FR-MSG-05, FR-MSG-06, FR-MSG-07, FR-MSG-08)
        if let Some(content) = text_to_send {
            if let Some(ref peer_fp) = self.selected_peer_fingerprint {
                let sender_pub = self.identity.ed25519_verifying_key_bytes();
                let sender_pub_hex = hex::encode(sender_pub);

                match Message::new_outbound(peer_fp.clone(), sender_pub_hex, content) {
                    Ok(mut msg) => {
                        // FR-MSG-04: Mark message as PENDING while awaiting TCP write
                        msg.status = MessageStatus::Pending;
                        {
                            let conn = self.db.lock().unwrap();
                            let _ = Database::insert_message(&conn, &msg);
                        }
                        self.active_messages.push(msg.clone());

                        let conn_mgr = self.conn_mgr.clone();
                        let db_clone = Arc::clone(&self.db);
                        let msg_clone = msg.clone();
                        let target_fp = peer_fp.clone();

                        tokio::spawn(async move {
                            // FR-MSG-07 & FR-MSG-08
                            match conn_mgr.send_message(&target_fp, msg_clone.clone()).await {
                                Ok(()) => {
                                    // FR-MSG-05: Mark message SENT once TCP write succeeds
                                    info!("Message {} sent successfully to {}", msg_clone.msg_id, target_fp);
                                    let conn = db_clone.lock().unwrap();
                                    let _ = Database::update_message_status(
                                        &conn,
                                        &msg_clone.msg_id,
                                        MessageStatus::Sent,
                                    );
                                }
                                Err(e) => {
                                    // FR-MSG-06: Mark message FAILED on TCP write failure or offline
                                    warn!("Message send failed to {}: {}", target_fp, e);
                                    let conn = db_clone.lock().unwrap();
                                    let _ = Database::update_message_status(
                                        &conn,
                                        &msg_clone.msg_id,
                                        MessageStatus::Failed,
                                    );
                                }
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Message creation error: {}", e);
                    }
                }
            }
        }

        // Handle Resend (FR-MSG-09)
        if let Some(msg_id) = msg_id_to_resend {
            if let Some(ref peer_fp) = self.selected_peer_fingerprint {
                if let Some(existing) = self.active_messages.iter_mut().find(|m| m.msg_id == msg_id) {
                    existing.status = MessageStatus::Pending;
                    let msg_to_send = existing.clone();
                    let target_fp = peer_fp.clone();
                    let conn_mgr = self.conn_mgr.clone();
                    let db_clone = Arc::clone(&self.db);

                    tokio::spawn(async move {
                        match conn_mgr.send_message(&target_fp, msg_to_send.clone()).await {
                            Ok(()) => {
                                let conn = db_clone.lock().unwrap();
                                let _ = Database::update_message_status(
                                    &conn,
                                    &msg_to_send.msg_id,
                                    MessageStatus::Sent,
                                );
                            }
                            Err(_) => {
                                let conn = db_clone.lock().unwrap();
                                let _ = Database::update_message_status(
                                    &conn,
                                    &msg_to_send.msg_id,
                                    MessageStatus::Failed,
                                );
                            }
                        }
                    });
                }
            }
        }

        // Handle Settings: Save Nickname (FR-ID-06)
        if save_nickname {
            let nick_opt = if self.settings_nickname_buf.trim().is_empty() {
                None
            } else {
                Some(self.settings_nickname_buf.trim().to_string())
            };
            let conn = self.db.lock().unwrap();
            let _ = self.identity.set_nickname(&conn, nick_opt);
        }

        // Handle Settings: Reset Identity (FR-ID-10)
        if reset_identity {
            let conn = self.db.lock().unwrap();
            match self.identity.reset_identity(&conn) {
                Ok(new_fp) => {
                    info!("Reset identity complete. New fingerprint: {}", new_fp);
                }
                Err(e) => error!("Failed to reset identity: {}", e),
            }
        }

        // Handle Identity Change Modal Decisions (FR-ID-09)
        if let Some(ref peer) = self.identity_changed_peer {
            if trust_identity {
                let conn = self.db.lock().unwrap();
                let _ = self.peer_tracker.resolve_identity_change(&conn, &peer.fingerprint, true);
                self.identity_changed_peer = None;
            } else if block_identity {
                let conn = self.db.lock().unwrap();
                let _ = self.peer_tracker.resolve_identity_change(&conn, &peer.fingerprint, false);
                self.identity_changed_peer = None;
            }
        }

        // Handle Manual Add Peer (FR-PD-06)
        if add_peer_manually {
            let ip = self.manual_add_ip.trim().to_string();
            let fp = self.manual_add_fingerprint.trim().to_uppercase();
            let conn = self.db.lock().unwrap();
            let _ = self.peer_tracker.register_peer(
                &conn,
                fp.clone(),
                format!("Manual-{}", &fp[0..std::cmp::min(6, fp.len())]),
                ip,
                self.config.tcp_port,
            );
            self.manual_add_ip.clear();
            self.manual_add_fingerprint.clear();
        }

        // Keep UI reactive to background network events
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}