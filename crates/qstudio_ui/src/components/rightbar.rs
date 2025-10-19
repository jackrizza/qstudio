use busbar::Aluminum;
use engine::output::Output;
use events::Event;
use std::sync::Arc;

use qstudio_tcp::Client;

pub struct RightBar {
    pub visible: bool,
    pub _init: bool,
    pub aluminum: Arc<Aluminum<(Client, Event)>>,
    pub widgets: Vec<Widget>,
    pub width: f32,
    _only_client: Client,
}

#[allow(dead_code)]
pub enum Widget {
    Placeholder,
    Widget { name: String, data: Output },
}

impl Widget {
    pub fn name(&self) -> String {
        match self {
            Widget::Placeholder => "Placeholder".to_string(),
            Widget::Widget { name, .. } => name.clone(),
        }
    }

    pub fn ui(&self, ui: &mut egui::Ui, width: f32, height: f32, columns: usize) {
        egui::Frame::new()
            .inner_margin(8.0)
            .outer_margin(16.0)
            .fill(theme::get_mode_theme(ui.ctx()).mantle)
            .shadow(egui::epaint::Shadow {
                offset: [16, 16],
                blur: 0,
                spread: 0,
                color: theme::get_mode_theme(ui.ctx()).base,
            })
            .show(ui, |ui| {
                let width = if columns > 1 {
                    width * 0.8 - 32.0 // account for margins and spacing
                } else {
                    width * 0.9 - 64.0 // account for margins
                };
                ui.set_width(width); // account for margins
                ui.set_min_height(height * 0.2);
                ui.vertical(|ui| {
                    ui.heading(self.name());
                    ui.separator();
                    match self {
                        Widget::Placeholder => {
                            ui.label("No widget available");
                        }
                        Widget::Widget { name, data } => {
                            ui.label(format!("Widget: {}", name));
                            ui.label(format!("Data: {:?}", data));
                        }
                    }
                });
            });
    }
}

impl RightBar {
    pub fn new(aluminum: Arc<Aluminum<(Client, Event)>>, only_client: Client) -> Self {
        RightBar {
            visible: false,
            _init: false,
            aluminum,
            widgets: vec![
                Widget::Placeholder,
                Widget::Placeholder,
                Widget::Placeholder,
                Widget::Placeholder,
                Widget::Placeholder,
            ],
            width: 0.0,
            _only_client: only_client,
        }
    }

    fn pump_snapshots(&mut self, ctx: &egui::Context) {
        let update = false;
        while let Ok((_client, ev)) = self.aluminum.widget_rx.try_recv() {
            log::info!("Right bar received event: {}", ev);
            // Handle events as needed
            if let Event::UiEvent(ui_event) = ev {
                match ui_event {
                    events::UiEvent::ToggleRightBar => {
                        self.visible = !self.visible;
                    }
                    _ => {
                        log::warn!("Unsupported UiEvent received in RightBar");
                    }
                }
            }
        }

        if update {
            // Handle any state updates if necessary
            ctx.request_repaint();
        }
    }

    pub fn ui_with(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("right_panel")
            .resizable(true)
            .min_width(300.0)
            .max_width(600.0)
            .frame(
                egui::Frame::NONE
                    .inner_margin(16.0)
                    .outer_margin(0.0)
                    .fill(theme::get_mode_theme(ctx).crust),
            )
            .show_animated(ctx, self.visible, |ui| {
                self.width = ui.available_width();
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0); // use some spacing

                let panel_w = ui.available_width();
                let panel_h = ui.available_height();
                let cols = if panel_w > 400.0 { 2 } else { 1 };

                // compute a per-cell width so each widget knows its space
                let spacing_x = ui.spacing().item_spacing.x;
                let total_spacing = if cols > 0 {
                    spacing_x * (cols as f32 - 1.0)
                } else {
                    0.0
                };
                let cell_w = (panel_w - total_spacing).max(0.0) / cols.max(1) as f32;

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show_viewport(ui, |ui, _| {
                        ui.set_min_width(panel_w);
                        ui.set_min_height(panel_h);

                        egui::Grid::new("rightbar_widgets_grid")
                            .num_columns(cols)
                            .spacing(egui::vec2(spacing_x, ui.spacing().item_spacing.y))
                            .show(ui, |ui| {
                                for (i, widget) in self.widgets.iter().enumerate() {
                                    ui.vertical(|ui| {
                                        ui.set_width(cell_w);
                                        widget.ui(ui, cell_w, panel_h, cols);
                                    });

                                    if (i + 1) % cols == 0 {
                                        ui.end_row();
                                    }
                                }

                                if self.widgets.len() % cols != 0 {
                                    ui.end_row();
                                }
                            });
                    });
            });
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        self.pump_snapshots(ctx);
        if self.visible {
            self.ui_with(ctx);
        } else {
            self.width = 0.0;
        }
    }
}
