//! Blast Radius analyzer.
//!
//! Builds an import graph for a TypeScript/JavaScript project and computes the
//! reverse-reachable set of a list of changed files.
//!
//! **This crate knows nothing about GitHub.** It takes a directory and a list of
//! changed file paths, and returns a graph plus a reached set. That boundary is
//! what makes it testable in Rust alone — see `CLAUDE.md`.
//!
//! The wasm surface lives in [`wasm`] and is a thin JSON-in/JSON-out wrapper
//! over the same functions the Rust tests call directly.

// A panic inside the wasm module surfaces in a consumer's CI as an opaque
// `unreachable executed`, and `CLAUDE.md` forbids failing anyone's CI. Deny
// the easy routes to one. These are crate-level attributes rather than a
// `[lints]` table in Cargo.toml so they apply to the library only — integration
// tests are separate crates and are expected to use `expect()` and indexing.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::indexing_slicing)]

use serde::{Deserialize, Serialize};

/// Identity of the analyzer build, logged by the Action so a surprising result
/// can be traced back to a specific wasm artifact.
///
/// The fields are owned `String`s rather than `&'static str`. Every type that
/// crosses the wasm boundary must round-trip through `serde_json`, and
/// `Deserialize` for a borrowed field would tie the value's lifetime to the
/// JSON buffer it came from. Owned fields keep boundary types self-contained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzerInfo {
    pub name: String,
    pub version: String,
}

/// The name reported over the wasm boundary. Deliberately not the crate name:
/// `analyzer` is meaningless in a consumer's CI log.
const ANALYZER_NAME: &str = "blast-radius-analyzer";

/// Returns the analyzer's identity.
#[must_use]
pub fn analyzer_info() -> AnalyzerInfo {
    AnalyzerInfo {
        name: ANALYZER_NAME.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub mod discover;
pub mod fs;
pub mod graph;
pub mod parse;
pub mod reach;
pub mod resolve;
