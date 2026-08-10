use analyzer::fs::RealFs;
use analyzer::graph::build_graph;
use analyzer::reach::reverse_reach;

fn reach(fixture: &str, changed: &[&str]) -> analyzer::reach::Reached {
    let root = format!("{}/tests/fixtures/{fixture}", env!("CARGO_MANIFEST_DIR"));
    let graph = build_graph(&RealFs::new(root), "");
    reverse_reach(
        &graph,
        &changed.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
}

#[test]
fn a_changed_file_reaches_itself_at_depth_zero() {
    let reached = reach("plain", &["src/util/round.ts"]);
    assert_eq!(reached.depths.get("src/util/round.ts"), Some(&0));
}

#[test]
fn reaches_transitive_importers_with_increasing_depth() {
    let reached = reach("plain", &["src/util/round.ts"]);
    assert_eq!(reached.depths.get("src/pricing/total.ts"), Some(&1));
    assert_eq!(reached.depths.get("src/pricing/index.ts"), Some(&2));
    assert_eq!(reached.depths.get("src/index.ts"), Some(&3));
}

#[test]
fn does_not_reach_an_unrelated_file() {
    let reached = reach("plain", &["src/util/round.ts"]);
    assert!(!reached.depths.contains_key("src/orphan.ts"));
}

#[test]
fn an_orphan_reaches_only_itself() {
    let reached = reach("plain", &["src/orphan.ts"]);
    assert_eq!(reached.depths.len(), 1);
}

#[test]
fn a_cycle_terminates_and_visits_each_node_once() {
    // If this hangs or overflows, the traversal is recursive. Fix the
    // traversal, never the fixture.
    let reached = reach("cyclic", &["src/a.ts"]);
    assert!(reached.depths.contains_key("src/b.ts"));
    assert!(reached.depths.contains_key("src/c.ts"));
    assert!(reached.depths.contains_key("src/entry.ts"));
    assert_eq!(reached.depths.get("src/a.ts"), Some(&0));
}

#[test]
fn records_the_shortest_depth_when_several_paths_reach_a_file() {
    let reached = reach("cyclic", &["src/c.ts"]);
    // entry -> a -> b -> c, and a -> b -> c via the cycle; a is depth 2 either way.
    assert_eq!(reached.depths.get("src/a.ts"), Some(&2));
}

#[test]
fn several_changed_files_union_their_reach() {
    let reached = reach("plain", &["src/util/round.ts", "src/orphan.ts"]);
    assert_eq!(reached.depths.get("src/orphan.ts"), Some(&0));
    assert_eq!(reached.depths.get("src/index.ts"), Some(&3));
}

#[test]
fn a_changed_file_not_in_the_graph_is_ignored_not_fatal() {
    // A PR that deletes a file, or changes a .md, hands us paths with no node.
    let reached = reach("plain", &["docs/README.md", "src/util/round.ts"]);
    assert!(reached.depths.contains_key("src/index.ts"));
    assert!(!reached.depths.contains_key("docs/README.md"));
}

#[test]
fn an_empty_changed_set_reaches_nothing() {
    assert!(reach("plain", &[]).depths.is_empty());
}
