use analyzer::fs::RealFs;
use analyzer::{analyze, AnalyzeRequest};

fn run(fixture: &str, changed: &[&str]) -> analyzer::AnalyzeResult {
    let root = format!("{}/tests/fixtures/{fixture}", env!("CARGO_MANIFEST_DIR"));
    analyze(
        &RealFs::new(root),
        &AnalyzeRequest {
            root: String::new(),
            changed: changed.iter().map(|s| s.to_string()).collect(),
        },
    )
}

#[test]
fn returns_reached_files_sorted_by_depth_then_path() {
    let result = run("plain", &["src/util/round.ts"]);
    let depths: Vec<u32> = result.reached.iter().map(|r| r.depth).collect();
    let mut sorted = depths.clone();
    sorted.sort();
    assert_eq!(
        depths, sorted,
        "shallowest first — depth is what makes the output readable"
    );
}

#[test]
fn reports_graph_stats() {
    let result = run("plain", &["src/util/round.ts"]);
    assert_eq!(result.stats.files, 5);
    assert!(result.stats.edges > 0);
    assert_eq!(result.stats.unresolved, 0);
}

#[test]
fn surfaces_dynamic_gaps_in_the_result() {
    let result = run("cyclic", &["src/a.ts"]);
    assert_eq!(result.stats.dynamic_gaps, 1);
    assert_eq!(result.dynamic_gaps.len(), 1);
}

#[test]
fn serialises_to_stable_json() {
    let a = serde_json::to_string(&run("plain", &["src/util/round.ts"])).expect("serialises");
    let b = serde_json::to_string(&run("plain", &["src/util/round.ts"])).expect("serialises");
    assert_eq!(a, b);
}

#[test]
fn a_repository_with_no_sources_is_empty_not_an_error() {
    let fs = analyzer::fs::MemFs::from([("README.md", "# hi")]);
    let result = analyze(
        &fs,
        &AnalyzeRequest {
            root: String::new(),
            changed: vec!["README.md".into()],
        },
    );
    assert!(result.reached.is_empty());
    assert_eq!(result.stats.files, 0);
}

#[test]
fn a_scoped_root_keeps_paths_relative_to_the_scope() {
    // `root` scopes analysis to a subdirectory; `changed` is relative to that
    // same root, and so is everything in the result. Fs-relative paths here
    // would silently yield an empty reach.
    let root = format!("{}/tests/fixtures/plain", env!("CARGO_MANIFEST_DIR"));
    let result = analyze(
        &RealFs::new(root),
        &AnalyzeRequest {
            root: "src/pricing".into(),
            changed: vec!["total.ts".into()],
        },
    );
    assert_eq!(result.stats.files, 2);
    let paths: Vec<&str> = result.reached.iter().map(|r| r.path.as_str()).collect();
    assert!(
        paths.contains(&"total.ts"),
        "changed file at depth 0: {paths:?}"
    );
    assert!(
        paths.contains(&"index.ts"),
        "direct importer at depth 1: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.starts_with("src/")),
        "no fs-relative paths may leak: {paths:?}"
    );
}

#[test]
fn exposes_forward_edges_matching_the_stats_count() {
    let result = run("plain", &["src/util/round.ts"]);
    assert!(result.stats.edges > 0);
    assert_eq!(result.edges.len(), result.stats.edges as usize);
}

#[test]
fn edges_point_from_importer_to_imported() {
    let result = run("plain", &["src/util/round.ts"]);
    assert!(
        result.edges.contains(&(
            "src/index.ts".to_string(),
            "src/pricing/index.ts".to_string()
        )),
        "unexpected edges: {:?}",
        result.edges
    );
    assert!(
        result.edges.contains(&(
            "src/pricing/total.ts".to_string(),
            "src/util/round.ts".to_string()
        )),
        "unexpected edges: {:?}",
        result.edges
    );
}
