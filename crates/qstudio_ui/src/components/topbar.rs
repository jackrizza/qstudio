use busbar::Aluminum;
use egui::{self, Align, Layout, Sense, TextStyle, ViewportCommand};
use egui_extras::{Size, StripBuilder};
use events::{Event, UiEvent};
use qstudio_tcp::Client;
use std::sync::Arc;
// In your TopBar struct:
pub struct TopBar {
    frontend_aluminum: Arc<Aluminum<(Client, Event)>>,
    title_buf: String,
    client: Client,
}

impl TopBar {
    pub fn new(frontend_aluminum: Arc<Aluminum<(Client, Event)>>, client: Client) -> Self {
        Self {
            frontend_aluminum,
            title_buf: String::new(),
            client,
        }
    }

    // You can generate suggestions however you like:
    fn suggest_items(&self, query: &str) -> Vec<String> {
        if query.len() <= 1 {
            return vec![];
        }
        // placeholder suggestions:
        vec![
            format!("Search for: {}", query),
            "Open Project…".to_string(),
            "New File".to_string(),
            "Settings".to_string(),
        ]
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        // Don’t start a window drag if any widget wants the pointer:
        if ctx.input(|i| i.pointer.any_down()) && !ctx.wants_pointer_input() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        egui::TopBottomPanel::top("top_panel")
            .exact_height(48.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::none()
                    .inner_margin(8.0)
                    .outer_margin(0.0)
                    .fill(theme::get_mode_theme(ctx).crust),
            )
            .resizable(false)
            .show(ctx, |ui| {
                egui_extras::StripBuilder::new(ui)
                    .size(egui_extras::Size::exact(90.0))   // left
                    .size(egui_extras::Size::remainder())   // center (flex)
                    .size(egui_extras::Size::exact(72.0))   // right
                    .horizontal(|mut strip| {
                        // ---------- LEFT ----------
                        strip.cell(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                            let mk = |rgb: (u8,u8,u8)| {
                                egui::Button::new("")
                                    .fill(egui::Color32::from_rgb(rgb.0, rgb.1, rgb.2))
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::BLACK))
                                    .rounding(9.0)
                            };
                            let bs = egui::vec2(18.0, 18.0);
                            ui.horizontal(|ui| {
                                let _ = ui.add_sized(bs, mk((252, 97, 92)));
                                let _ = ui.add_sized(bs, mk((255,189, 46)));
                                let _ = ui.add_sized(bs, mk(( 39,201, 63)));
                            });
                        });

                        // ---------- CENTER (omnibar + dropdown) ----------
                        strip.cell(|ui| {
                            // width = 1/3 of screen, clamp to nice bounds
                            let screen_w = ui.ctx().screen_rect().width();
                            let omnibar_w = (screen_w / 3.0).clamp(240.0, 640.0);
                            let h = ui.spacing().interact_size.y;

                            ui.horizontal_centered(|ui| {
                                // Text field
                                let edit = egui::TextEdit::singleline(&mut self.title_buf)
                                    .hint_text("QStudio")
                                    .font(egui::TextStyle::Heading)
                                    .frame(true);

                                let resp = ui.add_sized([omnibar_w, h], edit);

                                // ---- DROPDOWN (z-index on top) ----
                                if self.title_buf.chars().count() > 1 {
                                    let items = self.suggest_items(&self.title_buf);
                                    if !items.is_empty() {
                                        // Anchor dropdown to bottom-left of the text field, with a small gap.
                                        let r = resp.rect;
                                        let pos = egui::pos2(r.left() - 6.0, r.bottom() + 4.0);
                                        let max_h = ui.ctx().screen_rect().height() * 0.40;

                                        egui::Area::new(egui::Id::new("omnibar_dropdown"))
                                            .order(egui::Order::Tooltip) // topmost layer (like z-index: 9999)
                                            .fixed_pos(pos)
                                            .interactable(true)
                                            .show(ui.ctx(), |ui| {
                                                // Popup frame & exact width matching the text field:
                                                egui::Frame::popup(ui.style())
                                                    .rounding(0.0)
                                                    .inner_margin(6.0)
                                                    .fill(theme::get_mode_theme(ui.ctx()).crust)
                                                    .show(ui, |ui| {
                                                        ui.set_width(omnibar_w);     // match text field width
                                                        // Optional: cap height and scroll
                                                        egui::ScrollArea::vertical()
                                                            .max_height(max_h)
                                                            .show(ui, |ui| {
                                                                // Render your items:
                                                                for item in items {
                                                                    let clicked = ui
                                                                        .add(egui::SelectableLabel::new(false, item.clone()))
                                                                        .clicked();
                                                                    if clicked {
                                                                        // Apply selection:
                                                                        self.title_buf = item;
                                                                        // You can also clear focus or hide dropdown by clearing the buffer, etc.
                                                                    }
                                                                }
                                                            });
                                                    });

                                                // Optional close behaviors:
                                                // - Close on Esc:
                                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                                    // e.g., clear buffer to hide, or manage a bool flag
                                                    // self.dropdown_open = false;
                                                }
                                                // - Close on click outside:
                                                // If the pointer clicked elsewhere and not in rects, you can hide similarly.
                                            });
                                    }
                                }
                            });
                        });

                        // ---------- RIGHT ----------
                        strip.cell(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(
                                                egui_material_icons::icons::ICON_DISPLAY_EXTERNAL_INPUT
                                            ).size(20.0)
                                        )
                                        .fill(egui::Color32::TRANSPARENT),
                                    )
                                    .clicked()
                                {
                                    let _ = self.frontend_aluminum
                                        .frontend_tx
                                        .send(( self.client.clone(), Event::UiEvent(UiEvent::OpenNewWindow)));
                                }
                                ui.add_space(8.0);
                                let right_bar_toggle = ui.add(
                                    egui::Button::new(
                                        egui::RichText::new(
                                            egui_material_icons::icons::ICON_SIDE_NAVIGATION
                                        ).size(20.0)
                                    )
                                    .fill(egui::Color32::TRANSPARENT),
                                );

                                if right_bar_toggle.clicked() {
                                    let _ = self.frontend_aluminum
                                        .frontend_tx
                                        .send(( self.client.clone(), Event::UiEvent(UiEvent::ToggleRightBar)));
                                }
                            });
                        });
                    });
            });
    }
}
