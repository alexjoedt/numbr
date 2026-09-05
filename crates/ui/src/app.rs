use iced::widget::text_editor;
use iced::{widget, Element, Task, Theme};

use numbr_core::{strip_comment, Engine, Scope, Value};
use std::sync::atomic::Ordering;
use tracing::{debug, trace, warn};

use crate::message::Message;
use crate::model::{CachedLine, Model};
use crate::persist;
use crate::view;

/// Top-level application state.
pub struct App {
    pub model: Model,
}

impl App {
    /// Create a new app, restoring the previously saved session if one exists.
    pub fn new() -> (Self, Task<Message>) {
        let mut model = Model {
            settings: persist::load_settings(),
            ..Model::default()
        };
        let focus_editor = widget::focus_next();

        if let Some(saved) = persist::load() {
            model.content = text_editor::Content::with_text(&saved);
            // Trigger an initial evaluation of the restored content.
            let text = model.content.text();
            let task = Task::perform(
                async move { blocking::unblock(move || evaluate_incremental(0, text, vec![])).await },
                |(generation, results, cache)| Message::EvalResults {
                    generation,
                    results,
                    cache,
                },
            );
            return (Self { model }, Task::batch([task, focus_editor]));
        }

        (Self { model }, focus_editor)
    }
}

pub fn app_update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::EditorAction(action) => {
            // Apply the editor action immediately so cursor/text updates without
            // waiting for evaluation — keeps the UI fully responsive.
            let is_edit = action.is_edit();
            app.model.content.perform(action);

            if !is_edit {
                return Task::none();
            }

            app.model.eval_generation = app.model.eval_generation.wrapping_add(1);
            // Clear copy highlight on any edit.
            app.model.copied_idx = None;

            let generation = app.model.eval_generation;
            let text = app.model.content.text();
            let cache = app.model.line_cache.clone();
            // Shared counter lets the async task abort before the blocking work
            // if a newer keystroke has already bumped the generation.
            let latest = app.model.latest_generation.clone();
            latest.store(generation, Ordering::Relaxed);

            debug!(
                generation,
                text_len = text.len(),
                "EditorAction: scheduling eval"
            );

            // Debounce: wait 150 ms after the last keystroke before evaluating
            // or persisting. After the timer fires, check whether another
            // keystroke has already superseded this generation — if so, skip the
            // blocking work entirely instead of running it just to discard it.
            Task::perform(
                async move {
                    async_io::Timer::after(std::time::Duration::from_millis(150)).await;
                    if latest.load(Ordering::Relaxed) != generation {
                        // A newer keystroke arrived — bail out without any work.
                        return Message::EvalCancelled { generation };
                    }
                    let (generation, results, cache) = blocking::unblock(move || {
                        if latest.load(Ordering::Relaxed) == generation {
                            if let Err(err) = persist::save(&text) {
                                warn!(%err, "failed to save session");
                            }
                        }
                        evaluate_incremental(generation, text, cache)
                    })
                    .await;
                    Message::EvalResults {
                        generation,
                        results,
                        cache,
                    }
                },
                |message| message,
            )
        }

        Message::EvalResults {
            generation,
            results,
            cache,
        } => {
            // Only apply results from the most recent evaluation; discard stale ones.
            if generation == app.model.eval_generation {
                debug!(
                    generation,
                    result_count = results.len(),
                    "EvalResults: applying (current)"
                );
                app.model.results = results;
                app.model.line_cache = cache;
            } else {
                debug!(
                    generation,
                    current = app.model.eval_generation,
                    "EvalResults: discarding stale result"
                );
            }
            Task::none()
        }

        Message::EvalCancelled { generation } => {
            debug!(generation, "EvalCancelled: cancelled before blocking work");
            Task::none()
        }

        Message::CopyResult(idx) => {
            let Some(value) = app.model.results.get(idx) else {
                return Task::none();
            };

            // Skip copying empty or error values.
            if matches!(value, Value::Err(_)) {
                return Task::none();
            }

            let text = value.to_string();
            if text.is_empty() {
                return Task::none();
            }

            // Write to clipboard and emit immediate visual feedback.
            Task::batch([
                iced::clipboard::write(text),
                Task::done(Message::CopiedFeedback(idx)),
            ])
        }

        Message::CopiedFeedback(idx) => {
            app.model.copied_idx = Some(idx);
            Task::none()
        }

        Message::ClearCopied => {
            app.model.copied_idx = None;
            Task::none()
        }

        Message::ToggleSettings => {
            app.model.settings_open = !app.model.settings_open;
            Task::none()
        }

        Message::SetFontSize(size) => {
            app.model.settings.font_size = size.clamp(10.0, 32.0);
            if let Err(err) = persist::save_settings(&app.model.settings) {
                warn!(%err, "failed to save settings");
            }
            Task::none()
        }

        Message::SetFontFamily(family) => {
            app.model.settings.font_family = family;
            if let Err(err) = persist::save_settings(&app.model.settings) {
                warn!(%err, "failed to save settings");
            }
            Task::none()
        }

        Message::SetFontWeight(weight) => {
            app.model.settings.font_weight = weight;
            if let Err(err) = persist::save_settings(&app.model.settings) {
                warn!(%err, "failed to save settings");
            }
            Task::none()
        }
    }
}

/// Incremental evaluation: find the first changed line, restore the scope
/// snapshot from just before it, and only re-evaluate from that point onwards.
///
/// Lines above the first dirty line are reused verbatim from `cache` — their
/// text and scope contributions have not changed.
fn evaluate_incremental(
    generation: u64,
    text: String,
    cache: Vec<CachedLine>,
) -> (u64, Vec<Value>, Vec<CachedLine>) {
    let new_lines: Vec<&str> = text.lines().map(strip_comment).collect();

    // First index where the new text diverges from the cache (or the shorter
    // of the two lengths, which covers pure insertions and deletions at the end).
    let first_dirty = new_lines
        .iter()
        .zip(cache.iter())
        .position(|(new, cached)| *new != cached.text)
        .unwrap_or_else(|| new_lines.len().min(cache.len()));

    debug!(
        generation,
        total_lines = new_lines.len(),
        cached_lines = cache.len(),
        first_dirty,
        "evaluate_incremental: starting"
    );

    // Seed the engine from the scope snapshot just before the dirty region.
    // If the change is at line 0, start with an empty scope.
    let initial_scope = if first_dirty == 0 {
        Scope::default()
    } else {
        cache[first_dirty - 1].scope_after.clone()
    };

    let mut engine = Engine::with_scope(initial_scope);

    // Reuse cached entries above the dirty line; re-evaluate from it onwards.
    let mut new_cache: Vec<CachedLine> = cache[..first_dirty].to_vec();
    for line in &new_lines[first_dirty..] {
        let result = engine.evaluate_line(line);
        let scope_after = engine.scope().clone();
        trace!(line, result = %result, "evaluate_incremental: line result");
        new_cache.push(CachedLine {
            text: (*line).to_owned(),
            result,
            scope_after,
        });
    }

    let results: Vec<Value> = new_cache.iter().map(|c| c.result.clone()).collect();
    (generation, results, new_cache)
}

pub fn app_view(app: &App) -> Element<'_, Message> {
    view::view(&app.model)
}

pub fn run() -> iced::Result {
    iced::application("numbr", app_update, app_view)
        .theme(|_| Theme::Dark)
        .style(|app: &App, _theme| iced::application::Appearance {
            background_color: app.model.settings.background.to_iced_color(),
            text_color: iced::Color::WHITE,
        })
        .window(iced::window::Settings {
            size: iced::Size::new(640.0, 460.0),
            position: iced::window::Position::Centered,
            decorations: false,
            transparent: true,
            platform_specific: iced::window::settings::PlatformSpecific {
                application_id: String::from("numbr"),
                ..Default::default()
            },
            ..Default::default()
        })
        .run_with(App::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full evaluation of `text` with no cache to reuse.
    fn from_scratch(text: &str) -> Vec<Value> {
        evaluate_incremental(0, text.to_owned(), Vec::new()).1
    }

    fn cache_for(text: &str) -> Vec<CachedLine> {
        evaluate_incremental(0, text.to_owned(), Vec::new()).2
    }

    /// Evaluate `edited` against the cache built from `initial` and return the
    /// results, having first asserted they match a from-scratch evaluation.
    fn incremental_after(initial: &str, edited: &str) -> Vec<Value> {
        let cache = cache_for(initial);
        let (_, results, _) = evaluate_incremental(1, edited.to_owned(), cache);
        assert_eq!(
            results,
            from_scratch(edited),
            "incremental != from scratch for:\n{edited}"
        );
        results
    }

    /// A cache whose results are replaced by sentinels. Any surviving sentinel
    /// proves the entry was reused rather than recomputed; the `scope_after`
    /// snapshots stay intact so later lines still evaluate correctly.
    fn poisoned(text: &str) -> Vec<CachedLine> {
        cache_for(text)
            .into_iter()
            .enumerate()
            .map(|(idx, line)| CachedLine {
                result: Value::Str(format!("cached-{idx}")),
                ..line
            })
            .collect()
    }

    #[test]
    fn test_evaluate_incremental_cold_start_matches_a_fresh_engine() {
        let text = "1 + 1\nx = 10\nx * 2";
        let (_, results, cache) = evaluate_incremental(0, text.to_owned(), Vec::new());

        let mut engine = Engine::new();
        let expected: Vec<Value> = text
            .lines()
            .map(|line| engine.evaluate_line(strip_comment(line)))
            .collect();

        assert_eq!(results, expected, "cold start must match a fresh engine");
        assert_eq!(cache.len(), 3, "one cache entry per input line");
    }

    #[test]
    fn test_evaluate_incremental_reevaluating_unchanged_text_is_a_no_op() {
        let text = "x = 10\nx * 2\nline2 + 1";
        let (_, first, cache) = evaluate_incremental(0, text.to_owned(), Vec::new());
        let (_, second, second_cache) = evaluate_incremental(1, text.to_owned(), cache);

        assert_eq!(
            second, first,
            "unchanged text must produce the same results"
        );
        assert_eq!(second_cache.len(), first.len(), "cache length is stable");
    }

    #[test]
    fn test_evaluate_incremental_reuses_lines_above_an_unchanged_prefix() {
        let text = "x = 10\nx * 2\nx + 5";
        let (_, results, _) = evaluate_incremental(1, text.to_owned(), poisoned(text));

        assert_eq!(
            results,
            vec![
                Value::Str("cached-0".to_owned()),
                Value::Str("cached-1".to_owned()),
                Value::Str("cached-2".to_owned()),
            ],
            "every line is clean, so every cached result survives"
        );
    }

    #[test]
    fn test_evaluate_incremental_edit_on_line_zero_recomputes_everything() {
        let results = incremental_after("x = 10\nx * 2", "x = 20\nx * 2");

        assert_eq!(
            results,
            vec![Value::Integer(20), Value::Integer(40)],
            "the scope restarts from Scope::default() when line 0 is dirty"
        );
    }

    #[test]
    fn test_evaluate_incremental_edit_on_line_zero_discards_the_cache() {
        let (_, results, _) =
            evaluate_incremental(1, "x = 20\nx * 2".to_owned(), poisoned("x = 10\nx * 2"));

        assert_eq!(
            results,
            vec![Value::Integer(20), Value::Integer(40)],
            "no sentinel may survive an edit on the first line"
        );
    }

    #[test]
    fn test_evaluate_incremental_edit_mid_document_keeps_the_prefix() {
        let initial = "x = 10\n1 + 1\nx * 2";
        let (_, results, _) =
            evaluate_incremental(1, "x = 10\n2 + 2\nx * 2".to_owned(), poisoned(initial));

        assert_eq!(
            results,
            vec![
                Value::Str("cached-0".to_owned()),
                Value::Integer(4),
                Value::Integer(20),
            ],
            "line 0 is reused; the edit and everything below it is recomputed"
        );
    }

    #[test]
    fn test_evaluate_incremental_appending_a_line_keeps_earlier_results() {
        let initial = "x = 10\nx * 2";
        let results = incremental_after(initial, "x = 10\nx * 2\nx + 5");

        assert_eq!(
            results,
            vec![Value::Integer(10), Value::Integer(20), Value::Integer(15)],
            "an append leaves the existing lines alone"
        );

        let (_, cached_results, cache) =
            evaluate_incremental(1, "x = 10\nx * 2\nx + 5".to_owned(), poisoned(initial));
        assert_eq!(
            cached_results[..2],
            [
                Value::Str("cached-0".to_owned()),
                Value::Str("cached-1".to_owned())
            ],
            "the appended line does not dirty the lines above it"
        );
        assert_eq!(cache.len(), 3, "the cache grows by exactly one entry");
    }

    #[test]
    fn test_evaluate_incremental_deleting_the_last_line_drops_one_entry() {
        let initial = "x = 10\nx * 2\nx + 5";
        let (_, results, cache) =
            evaluate_incremental(1, "x = 10\nx * 2".to_owned(), cache_for(initial));

        assert_eq!(results, from_scratch("x = 10\nx * 2"));
        assert_eq!(cache.len(), 2, "the cache shrinks by exactly one entry");
        assert_eq!(
            cache.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
            ["x = 10", "x * 2"],
            "no stale entry survives the truncation"
        );
    }

    #[test]
    fn test_evaluate_incremental_invalidates_variable_dependents() {
        let results = incremental_after("x = 10\nx * 2", "x = 20\nx * 2");

        assert_eq!(
            results[1],
            Value::Integer(40),
            "line 1 must follow the new value of x"
        );
    }

    #[test]
    fn test_evaluate_incremental_invalidates_line_references() {
        let results = incremental_after("100\nline1 + 10", "200\nline1 + 10");

        assert_eq!(
            results,
            vec![Value::Integer(200), Value::Integer(210)],
            "line references resolve against the recomputed line"
        );
    }

    #[test]
    fn test_evaluate_incremental_editing_only_a_comment_is_not_dirty() {
        let initial = "2 + 2 # note\nx = 1";
        let (_, results, _) =
            evaluate_incremental(1, "2 + 2 # other note\nx = 1".to_owned(), poisoned(initial));

        assert_eq!(
            results[0],
            Value::Str("cached-0".to_owned()),
            "comments are stripped before the dirty check, so the line is clean"
        );
    }

    #[test]
    fn test_evaluate_incremental_returns_the_generation_unchanged() {
        let (cold, _, cache) = evaluate_incremental(7, "1 + 1".to_owned(), Vec::new());
        let (warm, _, _) = evaluate_incremental(42, "1 + 2".to_owned(), cache);

        assert_eq!(cold, 7, "generation is passed through on a cold start");
        assert_eq!(warm, 42, "generation is passed through on a warm start");
    }
}
