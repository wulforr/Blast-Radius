use analyzer::fs::MemFs;
use analyzer::resolve::relative::resolve_relative;
use analyzer::resolve::Resolved;

fn fs() -> MemFs {
    MemFs::from([
        ("src/index.ts", ""),
        ("src/sibling.ts", ""),
        ("src/widget.tsx", ""),
        ("src/legacy.js", ""),
        ("src/util/index.ts", ""),
        ("src/util/round.ts", ""),
        ("src/exact.json", ""),
        ("src/both.ts", ""),
        ("src/both.js", ""),
    ])
}

fn resolve(importer: &str, specifier: &str) -> Option<Resolved> {
    resolve_relative(&fs(), importer, specifier)
}

#[test]
fn returns_none_for_a_bare_specifier() {
    assert!(resolve("src/index.ts", "react").is_none());
    assert!(resolve("src/index.ts", "@acme/ui").is_none());
}

#[test]
fn resolves_a_sibling_by_adding_an_extension() {
    assert_eq!(
        resolve("src/index.ts", "./sibling"),
        Some(Resolved::File("src/sibling.ts".into()))
    );
}

#[test]
fn resolves_tsx_and_js_when_ts_is_absent() {
    assert_eq!(
        resolve("src/index.ts", "./widget"),
        Some(Resolved::File("src/widget.tsx".into()))
    );
    assert_eq!(
        resolve("src/index.ts", "./legacy"),
        Some(Resolved::File("src/legacy.js".into()))
    );
}

#[test]
fn resolves_a_directory_through_its_index() {
    assert_eq!(
        resolve("src/index.ts", "./util"),
        Some(Resolved::File("src/util/index.ts".into()))
    );
}

#[test]
fn resolves_a_parent_relative_specifier() {
    assert_eq!(
        resolve("src/util/round.ts", "../sibling"),
        Some(Resolved::File("src/sibling.ts".into()))
    );
}

#[test]
fn maps_a_js_specifier_onto_a_ts_file() {
    // TypeScript's ESM output requires importing './round.js' for round.ts.
    // A resolver that trusts the extension literally gets every modern TS
    // project wrong.
    assert_eq!(
        resolve("src/index.ts", "./util/round.js"),
        Some(Resolved::File("src/util/round.ts".into()))
    );
}

#[test]
fn prefers_ts_over_js_when_both_exist() {
    assert_eq!(
        resolve("src/index.ts", "./both"),
        Some(Resolved::File("src/both.ts".into()))
    );
}

#[test]
fn resolves_an_exact_path_that_already_has_an_extension() {
    assert_eq!(
        resolve("src/index.ts", "./exact.json"),
        Some(Resolved::File("src/exact.json".into()))
    );
}

#[test]
fn reports_a_missing_relative_target_as_unresolved_not_none() {
    // None means "not my rung". Unresolved means "mine, and it is broken" —
    // the difference is what makes the unresolved count meaningful.
    assert_eq!(
        resolve("src/index.ts", "./does-not-exist"),
        Some(Resolved::Unresolved)
    );
}
