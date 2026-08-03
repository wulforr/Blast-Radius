use analyzer::fs::MemFs;
use analyzer::discover::discover;

fn fs() -> MemFs {
    MemFs::from([
        ("src/index.ts", "export {}"),
        ("src/deep/nested/thing.tsx", "export {}"),
        ("src/legacy.js", "module.exports = {}"),
        ("src/module.mjs", "export {}"),
        ("src/common.cjs", "module.exports = {}"),
        ("src/styles.css", "body{}"),
        ("src/types.d.ts", "declare const x: number"),
        ("README.md", "# hi"),
        ("node_modules/left-pad/index.js", "module.exports = {}"),
        ("dist/bundle.js", "!function(){}()"),
        (".next/static/chunk.js", "!function(){}()"),
        ("target/debug/thing.js", "!function(){}()"),
    ])
}

#[test]
fn finds_analysable_sources_only() {
    assert_eq!(
        discover(&fs(), ""),
        vec![
            "src/common.cjs",
            "src/deep/nested/thing.tsx",
            "src/index.ts",
            "src/legacy.js",
            "src/module.mjs",
        ]
    );
}

#[test]
fn skips_declaration_files() {
    // A .d.ts contains no runtime imports worth attributing a blast radius to.
    assert!(!discover(&fs(), "").contains(&"src/types.d.ts".to_string()));
}

#[test]
fn skips_ignored_directories() {
    let found = discover(&fs(), "");
    for ignored in ["node_modules", "dist", ".next", "target"] {
        assert!(
            !found.iter().any(|p| p.starts_with(ignored)),
            "{ignored} should not be walked"
        );
    }
}

#[test]
fn output_is_sorted_and_deterministic() {
    assert_eq!(discover(&fs(), ""), discover(&fs(), ""));
    let found = discover(&fs(), "");
    let mut sorted = found.clone();
    sorted.sort();
    assert_eq!(found, sorted);
}

#[test]
fn can_be_scoped_to_a_subdirectory() {
    assert_eq!(discover(&fs(), "src/deep"), vec!["src/deep/nested/thing.tsx"]);
}
