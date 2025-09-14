
pub struct TopBar;

impl TopBar {
    pub fn new() -> Self {
        Self {}
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel")
            .frame(
                egui::Frame::none()
                    .inner_margin(8.0)
                    .outer_margin(0.0)
                    .fill(theme::GITHUB_DARK.crust),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::LEFT), |ui| {
                        // Windows-style close, min, max buttons with stoplight colors
                        let button_size = egui::vec2(18.0, 18.0);
                        let spacing = 6.0;

                        let (close_color, min_color, max_color) = (
                            egui::Color32::from_rgb(252, 97, 92),  // Red
                            egui::Color32::from_rgb(255, 189, 46), // Yellow
                            egui::Color32::from_rgb(39, 201, 63),  // Green
                        );

                        let close = ui.add_sized(
                            button_size,
                            egui::Button::new("")
                                .fill(close_color)
                                .stroke(egui::Stroke::new(1.0, egui::Color32::BLACK))
                                .rounding(9.0),
                        );
                        if close.clicked() {
                            // _frame.close();
                        }

                        ui.add_space(spacing);

                        let min = ui.add_sized(
                            button_size,
                            egui::Button::new("")
                                .fill(min_color)
                                .stroke(egui::Stroke::new(1.0, egui::Color32::BLACK))
                                .rounding(9.0),
                        );
                        if min.clicked() {
                            // _frame.set_minimized(true);
                        }

                        ui.add_space(spacing);

                        let max = ui.add_sized(
                            button_size,
                            egui::Button::new("")
                                .fill(max_color)
                                .stroke(egui::Stroke::new(1.0, egui::Color32::BLACK))
                                .rounding(9.0),
                        );
                        if max.clicked() {
                            // if _frame.info().window_info.maximized {
                            //     _frame.set_maximized(false);
                            // } else {
                            //     _frame.set_maximized(true);
                            // }
                        }

                        ui.add_space(12.0);
                    });
                    ui.with_layout(
                        egui::Layout::top_down_justified(egui::Align::Center),
                        |ui| ui.heading("Q Studio"),
                    );
                    // Add an icon (using egui_material_icons, for example)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new(
                                    egui_material_icons::icons::ICON_SIDE_NAVIGATION,
                                )
                                .size(20.0),
                            ))
                            .clicked()
                        {
                            // Handle button click here
                        }
                    })
                });
            });
    }
}
