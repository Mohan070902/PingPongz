use egui::{Color32, RichText, ScrollArea, Ui};

use crate::models::{Peer, PeerState, TrustStatus};

pub struct PeerList;

impl PeerList {
    /// FR-UI-02 & FR-UI-05: Left panel listing all known peers, sorted Online first then Offline
    pub fn render(
        ui: &mut Ui,
        peers: &[Peer],
        search_query: &str,
        selected_peer_fp: &Option<String>,
        on_select_peer: &mut Option<String>,
        on_resolve_identity_change: &mut Option<String>,
    ) {
        ui.vertical(|ui| {
            ui.label(RichText::new("PEERS").strong().size(13.0));
            ui.separator();

            ScrollArea::vertical().show(ui, |ui| {
                if peers.is_empty() {
                    ui.label(RichText::new("No peers discovered yet...").italics());
                    return;
                }

                let query = search_query.trim().to_lowercase();

                for peer in peers {
                    let display_name = peer.display_label();
                    if !query.is_empty()
                        && !display_name.to_lowercase().contains(&query)
                        && !peer.last_ip.contains(&query)
                    {
                        continue;
                    }

                    let is_selected = selected_peer_fp.as_deref() == Some(&peer.fingerprint);

                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            // Status indicator dot
                            let dot_color = match peer.state {
                                PeerState::Online => Color32::from_rgb(50, 205, 50),
                                PeerState::Connecting => Color32::from_rgb(255, 165, 0),
                                PeerState::Discovered => Color32::from_rgb(100, 149, 237),
                                _ => Color32::from_rgb(128, 128, 128),
                            };

                            ui.colored_label(dot_color, "●");

                            let label = if let Some(ref nick) = peer.nickname {
                                format!("{} ({})", nick, peer.hostname)
                            } else {
                                peer.hostname.clone()
                            };

                            let mut text = RichText::new(label).strong();
                            if is_selected {
                                text = text.color(Color32::from_rgb(255, 215, 0));
                            }

                            if ui.selectable_label(is_selected, text).clicked() {
                                *on_select_peer = Some(peer.fingerprint.clone());
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("[{}]", peer.fingerprint))
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if peer.trust_status == TrustStatus::IdentityChanged {
                                    if ui
                                        .button(
                                            RichText::new("⚠️ Changed")
                                                .size(10.0)
                                                .color(Color32::RED),
                                        )
                                        .clicked()
                                    {
                                        *on_resolve_identity_change = Some(peer.fingerprint.clone());
                                    }
                                } else if peer.trust_status == TrustStatus::Blocked {
                                    ui.label(
                                        RichText::new("Blocked")
                                            .size(10.0)
                                            .color(Color32::RED),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new(format!("{}:{}", peer.last_ip, peer.tcp_port))
                                            .size(10.0)
                                            .color(Color32::DARK_GRAY),
                                    );
                                }
                            });
                        });
                    });
                }
            });
        });
    }
}
