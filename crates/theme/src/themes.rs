use crate::egui::Color32;

/// The colors for a theme variant.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Theme {
    pub rosewater: Color32,
    pub flamingo: Color32,
    pub pink: Color32,
    pub mauve: Color32,
    pub red: Color32,
    pub maroon: Color32,
    pub peach: Color32,
    pub yellow: Color32,
    pub green: Color32,
    pub teal: Color32,
    pub sky: Color32,
    pub sapphire: Color32,
    pub blue: Color32,
    pub lavender: Color32,
    pub text: Color32,
    pub subtext1: Color32,
    pub subtext0: Color32,
    pub overlay2: Color32,
    pub overlay1: Color32,
    pub overlay0: Color32,
    pub surface2: Color32,
    pub surface1: Color32,
    pub surface0: Color32,
    pub base: Color32,
    pub mantle: Color32,
    pub crust: Color32,
}
pub const GITHUB_LIGHT: Theme = Theme {
    rosewater: Color32::from_rgb(255, 243, 245),
    flamingo: Color32::from_rgb(255, 230, 230),
    pink: Color32::from_rgb(255, 215, 235),
    mauve: Color32::from_rgb(205, 140, 255),
    red: Color32::from_rgb(255, 87, 87),
    maroon: Color32::from_rgb(200, 55, 65),
    peach: Color32::from_rgb(255, 171, 96),
    yellow: Color32::from_rgb(255, 212, 0),
    green: Color32::from_rgb(34, 197, 94),
    teal: Color32::from_rgb(0, 204, 204),
    sky: Color32::from_rgb(85, 172, 238),
    sapphire: Color32::from_rgb(56, 139, 253),
    blue: Color32::from_rgb(36, 114, 200),
    lavender: Color32::from_rgb(194, 196, 255),
    text: Color32::from_rgb(36, 41, 46),        // fg.default
    subtext1: Color32::from_rgb(88, 96, 105),   // fg.muted
    subtext0: Color32::from_rgb(110, 118, 129), // fg.subtle
    overlay2: Color32::from_rgb(175, 184, 193),
    overlay1: Color32::from_rgb(208, 215, 222),
    overlay0: Color32::from_rgb(230, 235, 240),
    surface2: Color32::from_rgb(242, 245, 248),
    surface1: Color32::from_rgb(246, 248, 250),
    surface0: Color32::from_rgb(255, 255, 255), // canvas.default
    base: Color32::from_rgb(255, 255, 255),
    mantle: Color32::from_rgb(246, 248, 250), // canvas.subtle
    crust: Color32::from_rgb(240, 240, 240),
};

pub const GITHUB_DARK: Theme = Theme {
    rosewater: Color32::from_rgb(255, 228, 225),
    flamingo: Color32::from_rgb(255, 204, 204),
    pink: Color32::from_rgb(250, 175, 230),
    mauve: Color32::from_rgb(180, 140, 255),
    red: Color32::from_rgb(248, 81, 73),
    maroon: Color32::from_rgb(200, 70, 80),
    peach: Color32::from_rgb(255, 150, 100),
    yellow: Color32::from_rgb(255, 205, 68),
    green: Color32::from_rgb(74, 222, 128),
    teal: Color32::from_rgb(112, 255, 255),
    sky: Color32::from_rgb(103, 190, 255),
    sapphire: Color32::from_rgb(56, 180, 255),
    blue: Color32::from_rgb(88, 166, 255),
    lavender: Color32::from_rgb(160, 170, 255),
    text: Color32::from_rgb(201, 209, 217),     // fg.default
    subtext1: Color32::from_rgb(139, 148, 158), // fg.muted
    subtext0: Color32::from_rgb(110, 118, 129), // fg.subtle
    overlay2: Color32::from_rgb(65, 71, 78),
    overlay1: Color32::from_rgb(48, 54, 61),
    overlay0: Color32::from_rgb(38, 44, 51),
    surface2: Color32::from_rgb(33, 38, 45),
    surface1: Color32::from_rgb(22, 27, 34), // canvas.subtle
    surface0: Color32::from_rgb(13, 17, 23), // canvas.default
    base: Color32::from_rgb(13, 17, 23),
    mantle: Color32::from_rgb(22, 27, 34),
    crust: Color32::from_rgb(0, 0, 0),
};
