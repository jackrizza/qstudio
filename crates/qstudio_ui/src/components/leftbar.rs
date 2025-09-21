use busbar::Aluminum;

use super::sidebar::SideBar;
use events::Event;
use qstudio_tcp::Client;
use std::sync::Arc;

pub struct LeftBar {
    aluminum: Arc<Aluminum<(Client, Event)>>,
    pub sidebar: SideBar,
    client: Client,
}

impl LeftBar {
    pub fn new(aluminum: Arc<Aluminum<(Client, Event)>>, client: Client) -> Self {
        LeftBar {
            sidebar: SideBar::new(Arc::clone(&aluminum), client.clone()),
            aluminum,
            client,
        }
    }
    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("left_panel")
            .resizable(false)
            .min_width(self.sidebar.width())
            .max_width(self.sidebar.width())
            .frame(
                egui::Frame::none()
                    .inner_margin(0.0)
                    .outer_margin(0.0)
                    .fill(theme::get_mode_theme(ctx).crust),
            )
            .show_animated(ctx, true, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                self.sidebar.ui(ui, Arc::clone(&self.aluminum));
            });
    }
}
