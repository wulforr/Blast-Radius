//! The wasm-bindgen surface.
//!
//! Everything here is a thin wrapper. No analysis logic lives in this module —
//! if a function here does more than convert between JSON and a Rust type, it
//! belongs in a sibling module that the Rust tests can reach.

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
