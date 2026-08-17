use analyzer::parse::{parse_imports, ImportKind};

fn specifiers(source: &str) -> Vec<String> {
    parse_imports(source, "t.ts")
        .imports
        .iter()
        .filter_map(|i| i.specifier.clone())
        .collect()
}

#[test]
fn finds_static_imports() {
    let source = r#"
        import a from './a'
        import { b } from "./b"
        import * as c from './c'
        import './side-effect'
        import type { T } from './types'
    "#;
    assert_eq!(
        specifiers(source),
        ["./a", "./b", "./c", "./side-effect", "./types"]
    );
}

#[test]
fn finds_export_from() {
    let source = "export { x } from './x'\nexport * from './y'";
    let parsed = parse_imports(source, "t.ts");
    assert_eq!(specifiers(source), ["./x", "./y"]);
    assert!(parsed
        .imports
        .iter()
        .all(|i| matches!(i.kind, ImportKind::ExportFrom)));
}

#[test]
fn finds_require_with_a_literal_argument() {
    let parsed = parse_imports("const a = require('./a')", "t.cjs");
    assert_eq!(parsed.imports.len(), 1);
    assert!(matches!(parsed.imports[0].kind, ImportKind::Require));
}

#[test]
fn a_computed_require_is_a_gap_not_silence() {
    // Mirrors the dynamic-import gap: a `require` whose target is not
    // statically knowable is a graph hole that must be counted, never
    // dropped. Spec §4: "Record everything else as a dynamic gap."
    for source in ["const a = require(name)", "const b = require()"] {
        let parsed = parse_imports(source, "t.cjs");
        assert_eq!(parsed.imports.len(), 1, "source: {source}");
        assert!(
            matches!(parsed.imports[0].kind, ImportKind::DynamicExpression),
            "source: {source}"
        );
        assert!(parsed.imports[0].specifier.is_none(), "source: {source}");
        assert!(parsed.imports[0].line > 0, "source: {source}");
    }
}

#[test]
fn distinguishes_literal_from_computed_dynamic_imports() {
    let source = r#"
        const known = () => import('./known')
        const which = cond ? './a' : './b'
        const unknown = () => import(which)
    "#;
    let parsed = parse_imports(source, "t.ts");
    let kinds: Vec<_> = parsed.imports.iter().map(|i| &i.kind).collect();
    assert!(kinds
        .iter()
        .any(|k| matches!(k, ImportKind::DynamicLiteral)));
    assert!(kinds
        .iter()
        .any(|k| matches!(k, ImportKind::DynamicExpression)));

    let gap = parsed
        .imports
        .iter()
        .find(|i| matches!(i.kind, ImportKind::DynamicExpression))
        .expect("gap");
    assert!(
        gap.specifier.is_none(),
        "a computed import has no knowable specifier"
    );
    assert!(
        gap.line > 0,
        "a gap must carry a line number so it can be reported"
    );
}

#[test]
fn records_line_numbers() {
    let parsed = parse_imports("\n\nimport a from './a'", "t.ts");
    assert_eq!(parsed.imports[0].line, 3);
}

#[test]
fn parses_tsx_and_jsx() {
    let parsed = parse_imports(
        "import React from 'react'\nexport const A = () => <div />",
        "t.tsx",
    );
    assert!(!parsed.parse_failed);
    assert_eq!(parsed.imports.len(), 1);
}

#[test]
fn a_syntax_error_is_reported_not_panicked() {
    let parsed = parse_imports("import { from './broken", "t.ts");
    assert!(parsed.parse_failed);
}

#[test]
fn ignores_a_specifier_in_a_comment_or_string() {
    let parsed = parse_imports(
        "// import x from './fake'\nconst s = \"import y from './also-fake'\"",
        "t.ts",
    );
    assert!(parsed.imports.is_empty());
}
