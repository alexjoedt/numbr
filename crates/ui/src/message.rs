use numbr_core::Value;

use crate::model::{CachedLine, FontFamily, FontWeight};

#[derive(Debug, Clone)]
pub enum Message {
    EditorAction(iced::widget::text_editor::Action),
    /// Async evaluation completed. Only applied when `generation` matches the
    /// model's current generation (discards results from superseded keystrokes).
    EvalResults {
        generation: u64,
        results: Vec<Value>,
        cache: Vec<CachedLine>,
    },
    /// A debounced evaluation was superseded before it did any blocking work.
    EvalCancelled {
        generation: u64,
    },
    /// User clicked a result row — copy its text to the clipboard.
    CopyResult(usize),
    /// Brief visual feedback: highlight the copied line for one interaction cycle.
    CopiedFeedback(usize),
    /// Clear the copy-highlight state.
    ClearCopied,
    /// Toggle the settings panel open/closed.
    ToggleSettings,
    /// Set the editor/result font size.
    SetFontSize(f32),
    /// Set the editor/result font family.
    SetFontFamily(FontFamily),
    /// Set the editor/result font weight.
    SetFontWeight(FontWeight),
}
