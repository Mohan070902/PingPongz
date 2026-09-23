use egui::Ui;

pub struct TopBar;

impl TopBar {
    /// FR-UI-01: Top bar containing Search, Settings, and Profile controls
    pub fn render(
        ui: &mut Ui,
        search_query: &mut String,
        profile_label: &str,
        open_settings: &mut bool,
        open_manual_add: &mut bool,
    ) {
        ui.horizontal(|ui| {
            ui.heading("Rubix – PingPongzzz");
            ui.separator();

            // Search filter box
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(search_query)
                    .hint_text("Search peers...")
                    .desired_width(180.0),
            );

            ui.separator();

            // Manual Add Peer button (FR-PD-06)
            if ui.button("+ Add Peer").clicked() {
                *open_manual_add = true;
            }

            // Settings button
            if ui.button("⚙ Settings").clicked() {
                *open_settings = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(profile_label)
                        .strong()
                        .color(egui::Color32::from_rgb(100, 200, 255)),
                );
                ui.label("Identity: ");
            });
        });
    }
}
