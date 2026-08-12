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

/// Input to [`analyze`]: the tree to walk and the changed files to reach from.
///
/// `root` scopes analysis to a subdirectory of the filesystem (`""` for the
/// whole tree); `changed` paths are relative to that same root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRequest {
    pub root: String,
    pub changed: Vec<String>,
}

/// One reached module and the depth it was reached at. Depth 0 is a changed
/// file itself; depth 1 is a direct importer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReachedFile {
    pub path: String,
    pub depth: u32,
}

/// Graph health counters. These ride along in every result so a caller can
/// see that resolution largely failed instead of reading a confidently wrong
/// blast radius — see `CLAUDE.md` hard constraint 3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStats {
    pub files: u32,
    pub edges: u32,
    pub unresolved: u32,
    pub dynamic_gaps: u32,
    pub parse_failures: u32,
}

/// The full answer: what the diff reaches, how healthy the graph behind that
/// answer is, and everything that could not be resolved. Sorted and
/// deterministic — serialising the same analysis twice yields the same bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResult {
    pub reached: Vec<ReachedFile>,
    pub stats: GraphStats,
    pub unresolved: Vec<graph::UnresolvedRef>,
    pub dynamic_gaps: Vec<graph::DynamicGap>,
}

/// Build the import graph under the request's root and return the set of
/// modules the changed files transitively reach, shallowest first.
#[must_use]
pub fn analyze(fs: &dyn fs::FileSystem, request: &AnalyzeRequest) -> AnalyzeResult {
    let graph = graph::build_graph(fs, &request.root);
    let reached = reach::reverse_reach(&graph, &request.changed);

    let mut reached: Vec<ReachedFile> = reached
        .depths
        .into_iter()
        .map(|(path, depth)| ReachedFile { path, depth })
        .collect();
    reached.sort_by(|a, b| (a.depth, &a.path).cmp(&(b.depth, &b.path)));

    let edges: u32 = graph
        .forward
        .iter()
        .map(|targets| targets.len() as u32)
        .sum();

    AnalyzeResult {
        reached,
        stats: GraphStats {
            files: graph.files.len() as u32,
            edges,
            unresolved: graph.unresolved.len() as u32,
            dynamic_gaps: graph.dynamic_gaps.len() as u32,
            parse_failures: graph.parse_failures.len() as u32,
        },
        unresolved: graph.unresolved,
        dynamic_gaps: graph.dynamic_gaps,
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
