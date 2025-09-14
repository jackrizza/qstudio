use super::sidebar::SideBar;

pub struct LeftBar {
    pub sidebar: SideBar,
}

impl LeftBar {
    pub fn new() -> Self {
        LeftBar {
            sidebar: SideBar::new(),
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
            )
            .show(ctx, |ui| {
                self.sidebar.ui(ui);
            });
    }
}
