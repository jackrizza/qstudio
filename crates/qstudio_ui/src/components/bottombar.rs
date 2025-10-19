pub struct BottomBar;

impl BottomBar {
    pub fn new() -> Self {
        BottomBar
    }

    pub fn _ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom_bar")
            .frame(egui::Frame::NONE.inner_margin(8.0).outer_margin(0.0).fill(
                if ctx.style().visuals.dark_mode {
                    theme::GITHUB_DARK.crust
                } else {
                    theme::GITHUB_LIGHT.crust
                },
            ))
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_max_height(18.0);

                let text_color = if ui.visuals().dark_mode {
                    theme::GITHUB_DARK.text
                } else {
                    theme::GITHUB_LIGHT.text
                };

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(5.0, 0.0);

                    ui.label(
                        egui::RichText::new("Status: All systems operational")
                            .color(text_color)
                            .size(12.0)
                            .strong(),
                    );
                    // ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
                    //     ui.label("Ln 1, Col 1");
                    // });
                });
            });
    }
}
