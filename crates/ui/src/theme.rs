use iced::Color;

// Numi-like palette: one flat dark surface with bright results and minimal chrome.

pub const BG: Color = Color::from_rgb(0.125, 0.133, 0.145); // #202225
pub const EDITOR_BG: Color = BG;
pub const GUTTER_BG: Color = BG;
pub const BOTTOM_BAR_BG: Color = BG;

pub const TEXT: Color = Color::from_rgb(0.949, 0.949, 0.949); // #f2f2f2
pub const TEXT_DIM: Color = Color::from_rgb(0.541, 0.561, 0.596); // #8a8f98
pub const GUTTER_TEXT: Color = TEXT_DIM;
pub const ACCENT: Color = Color::from_rgb(0.557, 0.827, 0.180); // #8ed32e
pub const RESULT: Color = ACCENT;
pub const RESULT_COMMAND: Color = Color::from_rgb(1.000, 0.584, 0.161); // #ff9529
pub const RESULT_FUNCTION: Color = Color::from_rgb(0.388, 0.800, 1.000); // #63ccff
pub const ERROR: Color = Color::from_rgb(1.000, 0.420, 0.420); // #ff6b6b
pub const DIVIDER: Color = Color::TRANSPARENT;
pub const COPIED_TEXT: Color = ACCENT;
