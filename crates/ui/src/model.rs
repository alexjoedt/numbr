use iced::widget::text_editor;
use numbr_core::{Scope, Value};
use serde::{Deserialize, Serialize};
use std::sync::{atomic::AtomicU64, Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontFamily {
    SystemMonospace,
    JetBrainsMono,
    FiraCode,
    SourceCodePro,
    DejaVuSansMono,
}

impl FontFamily {
    pub fn all() -> &'static [FontFamily] {
        &[
            FontFamily::SystemMonospace,
            FontFamily::JetBrainsMono,
            FontFamily::FiraCode,
            FontFamily::SourceCodePro,
            FontFamily::DejaVuSansMono,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            FontFamily::SystemMonospace => "System",
            FontFamily::JetBrainsMono => "JetBrains Mono",
            FontFamily::FiraCode => "Fira Code",
            FontFamily::SourceCodePro => "Source Code Pro",
            FontFamily::DejaVuSansMono => "DejaVu Sans Mono",
        }
    }

    pub fn to_iced_font(&self) -> iced::Font {
        match self {
            FontFamily::SystemMonospace => iced::Font::MONOSPACE,
            FontFamily::JetBrainsMono => iced::Font {
                family: iced::font::Family::Name("JetBrains Mono"),
                ..iced::Font::MONOSPACE
            },
            FontFamily::FiraCode => iced::Font {
                family: iced::font::Family::Name("Fira Code"),
                ..iced::Font::MONOSPACE
            },
            FontFamily::SourceCodePro => iced::Font {
                family: iced::font::Family::Name("Source Code Pro"),
                ..iced::Font::MONOSPACE
            },
            FontFamily::DejaVuSansMono => iced::Font {
                family: iced::font::Family::Name("DejaVu Sans Mono"),
                ..iced::Font::MONOSPACE
            },
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            FontFamily::SystemMonospace => "SystemMonospace",
            FontFamily::JetBrainsMono => "JetBrainsMono",
            FontFamily::FiraCode => "FiraCode",
            FontFamily::SourceCodePro => "SourceCodePro",
            FontFamily::DejaVuSansMono => "DejaVuSansMono",
        }
    }
}

impl std::str::FromStr for FontFamily {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "JetBrainsMono" => FontFamily::JetBrainsMono,
            "FiraCode" => FontFamily::FiraCode,
            "SourceCodePro" => FontFamily::SourceCodePro,
            "DejaVuSansMono" => FontFamily::DejaVuSansMono,
            _ => FontFamily::SystemMonospace,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontWeight {
    Normal,
    Medium,
    Bold,
}

impl FontWeight {
    pub fn all() -> &'static [FontWeight] {
        &[FontWeight::Normal, FontWeight::Medium, FontWeight::Bold]
    }

    pub fn label(&self) -> &'static str {
        match self {
            FontWeight::Normal => "Normal",
            FontWeight::Medium => "Medium",
            FontWeight::Bold => "Bold",
        }
    }

    pub fn to_iced_weight(&self) -> iced::font::Weight {
        match self {
            FontWeight::Normal => iced::font::Weight::Normal,
            FontWeight::Medium => iced::font::Weight::Medium,
            FontWeight::Bold => iced::font::Weight::Bold,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundConfig {
    /// Background base color as a hex string, e.g. `"#202225"` or `"#fff"`.
    #[serde(default = "BackgroundConfig::default_color")]
    pub color: String,
    /// Opacity in the range `[0.0, 1.0]`. `1.0` is fully opaque.
    #[serde(default = "BackgroundConfig::default_transparency")]
    pub transparency: f32,
}

impl BackgroundConfig {
    fn default_color() -> String {
        String::from("#202225")
    }

    fn default_transparency() -> f32 {
        1.0
    }

    /// Parse the hex color and apply the configured transparency as an `iced::Color`.
    pub fn to_iced_color(&self) -> iced::Color {
        let hex = self.color.trim_start_matches('#');
        let (r, g, b) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0x20);
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0x22);
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0x25);
                (r, g, b)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0x20);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0x22);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0x25);
                (r, g, b)
            }
            _ => (0x20, 0x22, 0x25),
        };
        let alpha = self.transparency.clamp(0.0, 1.0);
        iced::Color::from_rgba8(r, g, b, alpha)
    }
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        BackgroundConfig {
            color: BackgroundConfig::default_color(),
            transparency: BackgroundConfig::default_transparency(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub font_size: f32,
    pub font_family: FontFamily,
    pub font_weight: FontWeight,
    #[serde(default)]
    pub background: BackgroundConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            font_size: 15.0,
            font_family: FontFamily::SystemMonospace,
            font_weight: FontWeight::Normal,
            background: BackgroundConfig::default(),
        }
    }
}

impl Settings {
    /// Returns an `iced::Font` that combines the selected family and weight.
    pub fn to_iced_font(&self) -> iced::Font {
        let base = self.font_family.to_iced_font();
        iced::Font {
            weight: self.font_weight.to_iced_weight(),
            ..base
        }
    }
}

/// Cached state for a single evaluated line.
#[derive(Debug, Clone)]
pub struct CachedLine {
    /// The exact text of the line at evaluation time.
    pub text: String,
    pub result: Value,
    /// Scope snapshot after this line was evaluated. Used to seed incremental
    /// re-evaluation from any line without replaying all preceding lines.
    pub scope_after: Scope,
}

#[derive(Debug)]
pub struct Model {
    pub content: text_editor::Content,
    pub results: Vec<Value>,
    /// Monotonically incremented on each editor change.
    pub eval_generation: u64,
    /// Shared with async eval tasks so they can check before doing blocking work.
    /// When a newer keystroke arrives before the 150 ms timer fires on an older
    /// task, the older task sees the mismatch and skips the blocking eval.
    pub latest_generation: Arc<AtomicU64>,
    /// Per-line evaluation cache for incremental re-evaluation.
    pub line_cache: Vec<CachedLine>,
    /// Index of the most recently copied result line, shown with visual feedback.
    /// Cleared when the editor content changes.
    pub copied_idx: Option<usize>,
    /// Current font/size settings.
    pub settings: Settings,
    /// Whether the settings panel is open.
    pub settings_open: bool,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            content: text_editor::Content::default(),
            results: Vec::new(),
            eval_generation: 0,
            latest_generation: Arc::new(AtomicU64::new(0)),
            line_cache: Vec::new(),
            copied_idx: None,
            settings: Settings::default(),
            settings_open: false,
        }
    }
}


