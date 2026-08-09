use analyzer::fs::RealFs;
use analyzer::graph::build_graph;

fn fixture(name: &str) -> analyzer::graph::Graph {
    let root = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    build_graph(&RealFs::new(root), "")
}

#[test]
fn indexes_every_source_file() {
    let graph = fixture("plain");
    assert_eq!(graph.files.len(), 5, "five .ts files in the plain fixture");
    assert!(graph.id_of("src/orphan.ts").is_some());
}

#[test]
fn records_forward_edges() {
    let graph = fixture("plain");
    let index = graph.id_of("src/index.ts").expect("index");
    let pricing = graph.id_of("src/pricing/index.ts").expect("pricing");
    assert!(graph.forward[index.0 as usize].contains(&pricing));
}

#[test]
fn records_the_matching_reverse_edge() {
    let graph = fixture("plain");
    let round = graph.id_of("src/util/round.ts").expect("round");
    let total = graph.id_of("src/pricing/total.ts").expect("total");
    assert!(graph.reverse[round.0 as usize].contains(&total));
}

#[test]
fn forward_and_reverse_hold_the_same_edge_count() {
    let graph = fixture("plain");
    let forward: usize = graph.forward.iter().map(|s| s.len()).sum();
    let reverse: usize = graph.reverse.iter().map(|s| s.len()).sum();
    assert_eq!(
        forward, reverse,
        "every edge must appear in both directions"
    );
}

#[test]
fn the_plain_fixture_resolves_every_import() {
    let graph = fixture("plain");
    assert!(
        graph.unresolved.is_empty(),
        "unresolved: {:?}",
        graph.unresolved
    );
}

#[test]
fn counts_a_computed_dynamic_import_as_a_gap() {
    let graph = fixture("cyclic");
    assert_eq!(graph.dynamic_gaps.len(), 1);
    assert_eq!(graph.dynamic_gaps[0].file, "src/dynamic.ts");
}

#[test]
fn a_literal_dynamic_import_becomes_a_real_edge() {
    let graph = fixture("cyclic");
    let dynamic = graph.id_of("src/dynamic.ts").expect("dynamic");
    let c = graph.id_of("src/c.ts").expect("c");
    assert!(graph.forward[dynamic.0 as usize].contains(&c));
}

#[test]
fn an_external_import_creates_no_node() {
    let graph = fixture("monorepo");
    assert!(graph.id_of("node:fs/promises").is_none());
    assert!(!graph.files.iter().any(|f| f.contains("node:")));
}

#[test]
fn output_is_deterministic_across_runs() {
    assert_eq!(fixture("plain").files, fixture("plain").files);
}
