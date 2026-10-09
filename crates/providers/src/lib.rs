//! External data providers for numbr.
//!
//! Currently an exchange-rate source: the [`RateProvider`] trait and [`OfflineProvider`],
//! which reads a local JSON cache. Not wired into the engine or the app yet.
#![forbid(unsafe_code)]

pub mod rates;
pub use rates::{OfflineProvider, RateError, RateProvider};
