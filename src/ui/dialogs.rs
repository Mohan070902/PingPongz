use egui::{Color32, Context, RichText, Window};

pub struct Dialogs;

impl Dialogs {
    /// FR-ID-06 & FR-ID-10: Settings Modal
    pub fn render_settings(
        ctx: &Context,
        open: &mut bool,
        _current_nickname: &str,
        hostname: &str,
        fingerprint: &str,
        new_nickname_buf: &mut String,
        on_save_nickname: &mut bool,
        on_reset_identity: &mut bool,
    ) {
        if !*open {
            return;
        }

        Window::new("Settings")
            .open(open)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.heading("Device Identity");
                ui.label(format!("Hostname: {}", hostname));
                ui.label(format!("Fingerprint: {}", fingerprint));

                ui.separator();
                ui.heading("User Profile");
                ui.horizontal(|ui| {
                    ui.label("Nickname:");
                    ui.text_edit_singleline(new_nickname_buf);
                    if ui.button("Save").clicked() {
                        *on_save_nickname = true;
                    }
                });

                ui.separator();
                ui.heading("Security Actions");
                ui.label(
                    RichText::new("Resetting identity generates new cryptographic keys. Existing message history will be preserved.")
                        .size(11.0)
                        .color(Color32::GRAY),
                );

                if ui
                    .button(RichText::new("⚠️ Reset Identity").color(Color32::RED))
                    .clicked()
                {
                    *on_reset_identity = true;
                }
            });
    }

    /// FR-ID-08 & FR-ID-09: Identity Changed Alert Modal
    pub fn render_identity_changed(
        ctx: &Context,
        open: &mut bool,
        peer_hostname: &str,
        old_fingerprint: &str,
        new_fingerprint: &str,
        on_trust: &mut bool,
        on_block: &mut bool,
    ) {
        if !*open {
            return;
        }

        let mut should_close = false;

        Window::new("⚠️ Identity Changed Warning")
            .open(open)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "Warning: The device '{}' has reappeared with a different cryptographic fingerprint!",
                        peer_hostname
                    ))
                    .strong()
                    .color(Color32::RED),
                );

                ui.add_space(8.0);
                ui.label(format!("Previous Fingerprint: {}", old_fingerprint));
                ui.label(format!("New Fingerprint:      {}", new_fingerprint));

                ui.add_space(8.0);
                ui.label(
                    "This could indicate the peer reinstalled the application or a potential impersonation attempt.",
                );

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    // FR-ID-09: Present two options: Trust New Identity or Block
                    if ui
                        .button(RichText::new("Trust New Identity").color(Color32::GREEN))
                        .clicked()
                    {
                        *on_trust = true;
                        should_close = true;
                    }

                    if ui
                        .button(RichText::new("Block Peer").color(Color32::RED))
                        .clicked()
                    {
                        *on_block = true;
                        should_close = true;
                    }
                });
            });

        if should_close {
            *open = false;
        }
    }

    /// FR-PD-06: Manual Add Peer Modal
    pub fn render_manual_add(
        ctx: &Context,
        open: &mut bool,
        ip_buf: &mut String,
        fingerprint_buf: &mut String,
        on_add: &mut bool,
    ) {
        if !*open {
            return;
        }

        let mut should_close = false;

        Window::new("Add Peer Manually")
            .open(open)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.label("Enter peer LAN IP address and expected fingerprint:");
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label("IP Address:   ");
                    ui.text_edit_singleline(ip_buf);
                });

                ui.horizontal(|ui| {
                    ui.label("Fingerprint: ");
                    ui.text_edit_singleline(fingerprint_buf);
                });

                ui.add_space(10.0);
                let valid = !ip_buf.trim().is_empty() && fingerprint_buf.trim().len() >= 12;

                ui.horizontal(|ui| {
                    if ui.add_enabled(valid, egui::Button::new("Connect")).clicked() {
                        *on_add = true;
                        should_close = true;
                    }

                    if ui.button("Cancel").clicked() {
                        should_close = true;
                    }
                });
            });

        if should_close {
            *open = false;
        }
    }
}
