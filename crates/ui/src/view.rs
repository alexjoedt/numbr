use iced::advanced::text::highlighter::{Format, Highlighter};
use iced::widget::text::LineHeight;
use iced::widget::{button, column, container, row, scrollable, text, text_editor, Space};
use iced::{Background, Border, Color, Element, Length, Pixels};
use std::ops::Range;

const EDGE: f32 = 12.0; // top/bottom inset — must match text_editor top padding
const WINDOW_PADDING: f32 = 34.0;
const ROW_SPACING: f32 = 10.0;
const COLUMN_GAP: f32 = 30.0;
const BOTTOM_BAR_H: f32 = 30.0;
const SETTINGS_PANEL_H: f32 = 136.0;

use crate::message::Message;
use crate::model::{FontFamily, FontWeight, Model};
use crate::theme;
use numbr_core::Value;

fn format_result_value(value: &Value) -> String {
    match value {
        Value::Decimal(d) => group_thousands(&d.to_string()),
        Value::Float(_) | Value::Integer(_) => group_thousands(&value.to_string()),
        Value::Unit { amount, unit } => format!(
            "{} {unit}",
            group_thousands(&amount.normalize().to_string())
        ),
        _ => value.to_string(),
    }
}

fn group_thousands(value: &str) -> String {
    let (sign, unsigned) = value
        .strip_prefix('-')
        .map_or(("", value), |rest| ("-", rest));
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));

    let mut grouped = String::with_capacity(value.len() + integer.len() / 3);
    grouped.push_str(sign);

    for (idx, ch) in integer.chars().enumerate() {
        if idx > 0 && (integer.len() - idx) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(ch);
    }

    if !fraction.is_empty() {
        grouped.push(',');
        grouped.push_str(fraction);
    }

    grouped
}

pub fn view(model: &Model) -> Element<'_, Message> {
    let font_size = model.settings.font_size;
    // Line height scales with font size to keep rows aligned.
    let line_h = (font_size * 1.58).round();
    let font = model.settings.to_iced_font();
    let text_content = model.content.text();
    let line_count = text_content.lines().count().max(1);
    let (cursor_line, cursor_col) = model.content.cursor_position();

    // Build the result column.
    let results: Vec<Element<Message>> = model
        .results
        .iter()
        .enumerate()
        .map(|(idx, v)| result_row(idx, v, model.copied_idx == Some(idx), font_size, line_h))
        .collect();

    let result_col = scrollable(column(results).spacing(0).padding(iced::Padding {
        top: EDGE,
        right: 0.0,
        bottom: EDGE,
        left: 0.0,
    }))
    .height(Length::Fill)
    .width(Length::FillPortion(2));

    let editor = text_editor(&model.content)
        .on_action(Message::EditorAction)
        .line_height(LineHeight::Absolute(Pixels(line_h)))
        .padding(iced::Padding {
            top: EDGE,
            bottom: EDGE,
            left: 10.0,
            right: 4.0,
        })
        .style(|_, _| text_editor::Style {
            background: iced::Background::Color(Color::TRANSPARENT),
            border: iced::Border {
                color: iced::Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            icon: iced::Color::TRANSPARENT,
            placeholder: theme::DIVIDER,
            value: theme::TEXT,
            selection: theme::ACCENT,
        })
        .highlight_with::<ResultLineHighlighter>((), highlight_format)
        .size(font_size)
        .font(font)
        .height(Length::Fill);

    let editor_col = container(editor)
        .width(Length::FillPortion(3))
        .height(Length::Fill);

    let main_content = row![
        editor_col,
        Space::with_width(Length::Fixed(COLUMN_GAP)),
        result_col,
    ]
    .height(Length::Fill);

    let main_content = container(main_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: WINDOW_PADDING,
            bottom: WINDOW_PADDING,
            left: WINDOW_PADDING,
            right: WINDOW_PADDING,
        });

    // --- Bottom bar ---
    let settings_btn_label = if model.settings_open {
        "Close"
    } else {
        "Settings"
    };
    let settings_btn = button(
        text(settings_btn_label)
            .size(12)
            .color(if model.settings_open {
                theme::ACCENT
            } else {
                theme::TEXT
            }),
    )
    .on_press(Message::ToggleSettings)
    .style(|_, _| button::Style {
        background: None,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        text_color: theme::TEXT,
        shadow: iced::Shadow::default(),
    })
    .padding(iced::Padding {
        top: 4.0,
        bottom: 4.0,
        left: 10.0,
        right: 10.0,
    });

    let status_pos = text(format!("L {} : C {}", cursor_line + 1, cursor_col + 1))
        .size(11)
        .color(theme::TEXT_DIM);

    let status_lines = text(format!("{} lines", line_count))
        .size(11)
        .color(theme::TEXT_DIM);

    let bottom_bar = container(
        row![
            Space::with_width(Length::Fixed(10.0)),
            status_pos,
            Space::with_width(Length::Fixed(16.0)),
            status_lines,
            Space::with_width(Length::Fill),
            settings_btn,
        ]
        .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(BOTTOM_BAR_H))
    .style(|_| iced::widget::container::Style::default());

    // --- Settings panel (shown when open) ---
    let mut root = column![main_content];

    if model.settings_open {
        root = root.push(settings_panel(model));
    }

    root = root.push(bottom_bar);

    container(root)
        .style(|_| iced::widget::container::Style::default())
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[derive(Debug, Clone, Copy)]
enum ResultHighlight {
    Command,
    Function,
}

fn highlight_format(highlight: &ResultHighlight, _theme: &iced::Theme) -> Format<iced::Font> {
    Format {
        color: Some(match highlight {
            ResultHighlight::Command => theme::RESULT_COMMAND,
            ResultHighlight::Function => theme::RESULT_FUNCTION,
        }),
        font: None,
    }
}

struct ResultLineHighlighter {
    current_line: usize,
}

impl Highlighter for ResultLineHighlighter {
    type Settings = ();
    type Highlight = ResultHighlight;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Self::Highlight)>;

    fn new(_settings: &Self::Settings) -> Self {
        Self { current_line: 0 }
    }

    fn update(&mut self, _new_settings: &Self::Settings) {
        self.current_line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        result_line_highlights(line).into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

fn result_line_highlights(line: &str) -> Vec<(Range<usize>, ResultHighlight)> {
    let Some(command_start) = line.find("result") else {
        return Vec::new();
    };
    if !line[..command_start].trim().is_empty() {
        return Vec::new();
    }

    let command_end = command_start + "result".len();
    let mut highlights = vec![(command_start..command_end, ResultHighlight::Command)];

    let rest = &line[command_end..];
    let Some(colon_offset) = rest.find(':') else {
        return highlights;
    };

    highlights.push((
        command_end + colon_offset..command_end + colon_offset + 1,
        ResultHighlight::Command,
    ));

    let function_text = &rest[colon_offset + 1..];
    let function_start =
        command_end + colon_offset + 1 + function_text.len() - function_text.trim_start().len();
    let function_len = line[function_start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .map(char::len_utf8)
        .sum::<usize>();

    if function_len > 0 {
        highlights.push((
            function_start..function_start + function_len,
            ResultHighlight::Function,
        ));
    }

    highlights
}

fn settings_panel(model: &Model) -> Element<'_, Message> {
    let font_size = model.settings.font_size;

    // --- Font Size row ---
    let size_dec = button(text("-").size(14).color(theme::TEXT))
        .on_press(Message::SetFontSize(font_size - 1.0))
        .style(|_, _| button::Style {
            background: None,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 4.0.into(),
            },
            text_color: theme::TEXT,
            shadow: iced::Shadow::default(),
        })
        .padding(iced::Padding {
            top: 2.0,
            bottom: 2.0,
            left: 8.0,
            right: 8.0,
        });

    let size_inc = button(text("+").size(14).color(theme::TEXT))
        .on_press(Message::SetFontSize(font_size + 1.0))
        .style(|_, _| button::Style {
            background: None,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 4.0.into(),
            },
            text_color: theme::TEXT,
            shadow: iced::Shadow::default(),
        })
        .padding(iced::Padding {
            top: 2.0,
            bottom: 2.0,
            left: 8.0,
            right: 8.0,
        });

    let size_label = text(format!("{}px", font_size as u32))
        .size(13)
        .color(theme::TEXT);

    let size_row = row![
        text("Font Size").size(12).color(theme::TEXT),
        Space::with_width(Length::Fill),
        size_dec,
        container(size_label)
            .width(Length::Fixed(44.0))
            .align_x(iced::alignment::Horizontal::Center),
        size_inc,
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    // --- Font Family row ---
    let family_buttons: Vec<Element<Message>> = FontFamily::all()
        .iter()
        .map(|family| {
            let selected = &model.settings.font_family == family;
            button(text(family.label()).size(12))
                .on_press(Message::SetFontFamily(*family))
                .style(move |_, _| button::Style {
                    background: None,
                    border: Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 4.0.into(),
                    },
                    text_color: if selected { theme::ACCENT } else { theme::TEXT },
                    shadow: iced::Shadow::default(),
                })
                .padding(iced::Padding {
                    top: 3.0,
                    bottom: 3.0,
                    left: 8.0,
                    right: 8.0,
                })
                .into()
        })
        .collect();

    let family_row = row(
        std::iter::once(text("Font").size(12).color(theme::TEXT).into())
            .chain(std::iter::once(Space::with_width(Length::Fill).into()))
            .chain(family_buttons)
            .collect::<Vec<_>>(),
    )
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    // --- Font Weight row ---
    let weight_buttons: Vec<Element<Message>> = FontWeight::all()
        .iter()
        .map(|weight| {
            let selected = &model.settings.font_weight == weight;
            button(text(weight.label()).size(12))
                .on_press(Message::SetFontWeight(*weight))
                .style(move |_, _| button::Style {
                    background: None,
                    border: Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 4.0.into(),
                    },
                    text_color: if selected { theme::ACCENT } else { theme::TEXT },
                    shadow: iced::Shadow::default(),
                })
                .padding(iced::Padding {
                    top: 3.0,
                    bottom: 3.0,
                    left: 8.0,
                    right: 8.0,
                })
                .into()
        })
        .collect();

    let weight_row = row(
        std::iter::once(text("Weight").size(12).color(theme::TEXT).into())
            .chain(std::iter::once(Space::with_width(Length::Fill).into()))
            .chain(weight_buttons)
            .collect::<Vec<_>>(),
    )
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let panel_content = column![size_row, family_row, weight_row]
        .spacing(ROW_SPACING)
        .padding(iced::Padding {
            top: 10.0,
            bottom: 10.0,
            left: 14.0,
            right: 14.0,
        });

    let bg_color = model.settings.background.to_iced_color();
    container(panel_content)
        .width(Length::Fill)
        .height(Length::Fixed(SETTINGS_PANEL_H))
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(bg_color)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Build a single result row.
///
/// - Empty strings and error values are rendered as plain text (non-interactive).
/// - Valid results become a `button` that fires `Message::CopyResult(idx)`.
/// - The `copied` flag swaps the result text for a minimal copied indicator.
fn result_row(
    idx: usize,
    value: &Value,
    copied: bool,
    font_size: f32,
    line_h: f32,
) -> Element<'static, Message> {
    let is_error = matches!(value, Value::Err(_));
    let is_empty = matches!(value, Value::Str(s) if s.is_empty());

    let color = if is_error {
        theme::ERROR
    } else if copied {
        theme::COPIED_TEXT
    } else {
        theme::RESULT
    };

    let small_size = (font_size * 0.87).max(11.0);
    let label = if copied {
        text("copied")
            .color(color)
            .size(small_size)
            .align_x(iced::alignment::Horizontal::Right)
    } else {
        text(format_result_value(value))
            .color(color)
            .size(font_size)
            .align_x(iced::alignment::Horizontal::Right)
    };

    let inner = container(label)
        .width(Length::Fill)
        .height(Length::Fixed(line_h))
        .align_y(iced::alignment::Vertical::Center)
        .padding(iced::Padding {
            top: 0.0,
            bottom: 0.0,
            left: 4.0,
            right: 14.0,
        });

    if is_empty || is_error {
        inner.into()
    } else {
        button(inner)
            .on_press(Message::CopyResult(idx))
            .style(move |_, _status| button::Style {
                background: None,
                border: Border {
                    color: iced::Color::TRANSPARENT,
                    width: 0.0,
                    radius: 4.0.into(),
                },
                text_color: color,
                shadow: iced::Shadow::default(),
            })
            .width(Length::Fill)
            .padding(0)
            .into()
    }
}
