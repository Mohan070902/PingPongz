use chrono::{Local, TimeZone};
use egui::{Color32, RichText, ScrollArea, Ui};

use crate::models::{Message, MessageDirection, MessageStatus, Peer, PeerState, MAX_MESSAGE_SIZE};

pub struct ChatView;

impl ChatView {
    /// FR-UI-03 & FR-UI-04: Conversation panel and message input box
    pub fn render(
        ui: &mut Ui,
        peer: Option<&Peer>,
        messages: &[Message],
        input_text: &mut String,
        on_send: &mut Option<String>,
        on_resend: &mut Option<String>,
    ) {
        let peer = match peer {
            Some(p) => p,
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Select a peer to start messaging").size(16.0).italics());
                });
                return;
            }
        };

        // Header: Active peer details and status
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading(peer.display_label());

                let (color, text) = match peer.state {
                    PeerState::Online => (Color32::from_rgb(50, 205, 50), "Online"),
                    PeerState::Connecting => (Color32::from_rgb(255, 165, 0), "Connecting..."),
                    PeerState::Discovered => (Color32::from_rgb(100, 149, 237), "Discovered"),
                    _ => (Color32::GRAY, "Offline"),
                };

                ui.colored_label(color, format!("● {}", text));
            });
            ui.separator();

            // Conversation history scroll area
            let input_area_height = 80.0;
            let available_height = ui.available_height() - input_area_height;

            ScrollArea::vertical()
                .max_height(available_height)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if messages.is_empty() {
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new("No messages yet. Send a greeting!").italics());
                        });
                        return;
                    }

                    for msg in messages {
                        ui.horizontal(|ui| {
                            let is_outbound = msg.direction == MessageDirection::Outbound;

                            if is_outbound {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        // Status icon and resend action
                                        match msg.status {
                                            MessageStatus::Pending => {
                                                ui.label(RichText::new("🕒").color(Color32::YELLOW));
                                            }
                                            MessageStatus::Sent => {
                                                ui.label(RichText::new("✓").color(Color32::GREEN));
                                            }
                                            MessageStatus::Failed => {
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new("✗").color(Color32::RED));
                                                    if ui.small_button("↺ Resend").clicked() {
                                                        *on_resend = Some(msg.msg_id.clone());
                                                    }
                                                });
                                            }
                                        }

                                        let time_str = Local
                                            .timestamp_millis_opt(msg.timestamp_ms)
                                            .single()
                                            .map(|dt| dt.format("%H:%M:%S").to_string())
                                            .unwrap_or_default();

                                        ui.label(
                                            RichText::new(time_str)
                                                .size(10.0)
                                                .color(Color32::DARK_GRAY),
                                        );

                                        // Outbound bubble
                                        egui::Frame::none()
                                            .fill(Color32::from_rgb(30, 80, 140))
                                            .rounding(8.0)
                                            .inner_margin(8.0)
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(&msg.content)
                                                        .color(Color32::WHITE),
                                                );
                                            });
                                    },
                                );
                            } else {
                                // Inbound bubble
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(60, 60, 65))
                                    .rounding(8.0)
                                    .inner_margin(8.0)
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(&msg.content).color(Color32::WHITE),
                                        );
                                    });

                                let time_str = Local
                                    .timestamp_millis_opt(msg.timestamp_ms)
                                    .single()
                                    .map(|dt| dt.format("%H:%M:%S").to_string())
                                    .unwrap_or_default();

                                ui.label(
                                    RichText::new(time_str)
                                        .size(10.0)
                                        .color(Color32::DARK_GRAY),
                                );
                            }
                        });
                    }
                });

            ui.separator();

            // FR-UI-04: Bottom message input box with Send button
            ui.horizontal(|ui| {
                let bytes_len = input_text.as_bytes().len();
                let is_oversized = bytes_len > MAX_MESSAGE_SIZE;
                let is_offline = peer.state == PeerState::Offline;

                let response = ui.add_sized(
                    [ui.available_width() - 140.0, 36.0],
                    egui::TextEdit::singleline(input_text).hint_text("Type a message (Enter to send)..."),
                );

                let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                let counter_color = if is_oversized {
                    Color32::RED
                } else {
                    Color32::GRAY
                };

                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!("{}/{} B", bytes_len, MAX_MESSAGE_SIZE))
                            .size(10.0)
                            .color(counter_color),
                    );

                    let send_enabled = !input_text.trim().is_empty() && !is_oversized && !is_offline;

                    if ui.add_enabled(send_enabled, egui::Button::new("Send ▶")).clicked()
                        || (enter_pressed && send_enabled)
                    {
                        let text = input_text.trim().to_string();
                        if !text.is_empty() {
                            *on_send = Some(text);
                            input_text.clear();
                        }
                    }
                });
            });

            // FR-MSG-08: Offline peer warning
            if peer.state == PeerState::Offline {
                ui.label(
                    RichText::new("⚠️ Peer is offline. Sending is disabled (offline messaging not supported in v1.0).")
                        .color(Color32::YELLOW)
                        .size(11.0),
                );
            }
        });
    }
}
