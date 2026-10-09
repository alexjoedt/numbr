//! Iced GUI of numbr, a natural-language calculator for Linux/Wayland.
//!
//! Renders the editor and the result column, evaluates the buffer line by line with
//! `numbr-core` (incrementally, from the first changed line on), and persists the session
//! and settings. [`app::run`] starts the application; `numbr-app` calls it.
#![forbid(unsafe_code)]

pub mod app;
pub mod message;
pub mod model;
pub mod persist;
pub mod theme;
pub mod view;
