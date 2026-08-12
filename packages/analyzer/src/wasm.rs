//! The wasm-bindgen surface.
//!
//! Everything here is a thin wrapper. No analysis logic lives in this module —
//! if a function here does more than convert between JSON and a Rust type, it
//! belongs in a sibling module that the Rust tests can reach.

use std::collections::BTreeMap;

use serde::Deserialize;
use wasm_bindgen::prelude::*;

/// Installs a panic hook that prints a Rust backtrace to the JS console.
///
/// Called once by the Action before anything else. Without it a panic in the
/// wasm module surfaces as `unreachable executed`, which tells a consumer
/// nothing.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Returns `{"name":..., "version":...}` as a JSON string.
#[wasm_bindgen(js_name = analyzerInfo)]
#[must_use]
pub fn analyzer_info_json() -> String {
    // A struct of two &'static str cannot fail to serialise; the fallback keeps
    // the `unwrap_used` lint satisfied without a panic path in shipped wasm.
    serde_json::to_string(&crate::analyzer_info()).unwrap_or_else(|_| "{}".to_owned())
}

/// Input to [`analyze_json`]: the changed files plus the file contents to
/// analyse, keyed by root-relative path.
///
/// This deliberately differs from [`crate::AnalyzeRequest`], which points at
/// a directory the host reads itself. `wasm32-unknown-unknown` has no
/// filesystem — every `std::fs` call inside the module fails — so a `RealFs`
/// rooted at `request.root` would always see an empty tree. The Action walks
/// the checkout in JavaScript and ships `{ path: contents }` here instead;
/// the analysis itself is the same [`crate::analyze`] the Rust tests call.
#[derive(Deserialize)]
struct AnalyzeJsonRequest {
    #[serde(default)]
    changed: Vec<String>,
    #[serde(default)]
    files: BTreeMap<String, String>,
}

/// Runs the analysis over the given file contents and returns the
/// [`crate::AnalyzeResult`] as JSON.
///
/// Takes `{"changed": [...], "files": {"path": "contents"}}`. Result paths
/// are relative to whatever base the caller used for the `files` keys.
///
/// A malformed request returns a JSON error object, never a panic — a panic
/// across the wasm boundary surfaces in a consumer's CI as an opaque
/// `unreachable executed`.
#[wasm_bindgen(js_name = analyzeJson)]
#[must_use]
pub fn analyze_json(input: &str) -> String {
    let request: AnalyzeJsonRequest = match serde_json::from_str(input) {
        Ok(request) => request,
        Err(error) => return error_json(&error.to_string()),
    };
    let filesystem = crate::fs::MemFs::from_map(request.files);
    let scoped = crate::AnalyzeRequest {
        root: String::new(),
        changed: request.changed,
    };
    let result = crate::analyze(&filesystem, &scoped);
    serde_json::to_string(&result).unwrap_or_else(|_| error_json("serialise failed"))
}

fn error_json(message: &str) -> String {
    serde_json::to_string(&serde_json::json!({ "error": message }))
        .unwrap_or_else(|_| r#"{"error":"unknown"}"#.to_owned())
}
