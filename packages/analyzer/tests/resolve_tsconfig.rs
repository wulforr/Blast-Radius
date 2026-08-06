use analyzer::fs::MemFs;
use analyzer::resolve::tsconfig::{load_tsconfig, resolve_paths};
use analyzer::resolve::Resolved;

fn fs_with(configs: &[(&str, &str)]) -> MemFs {
    let mut fs = MemFs::from([
        ("src/lib/format.ts", ""),
        ("src/shared/index.ts", ""),
        ("src/main.ts", ""),
    ]);
    for (path, body) in configs {
        fs.insert(path, body);
    }
    fs
}

#[test]
fn resolves_a_wildcard_mapping() {
    let fs = fs_with(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
    )]);
    let config = load_tsconfig(&fs, "").expect("config");
    assert_eq!(
        resolve_paths(&fs, &config, "", "@app/lib/format"),
        Some(Resolved::File("src/lib/format.ts".into()))
    );
}

#[test]
fn resolves_an_exact_non_wildcard_mapping() {
    let fs = fs_with(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@shared":["src/shared/index.ts"]}}}"#,
    )]);
    let config = load_tsconfig(&fs, "").expect("config");
    assert_eq!(
        resolve_paths(&fs, &config, "", "@shared"),
        Some(Resolved::File("src/shared/index.ts".into()))
    );
}

#[test]
fn tries_each_candidate_until_one_exists() {
    let fs = fs_with(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["nowhere/*","src/*"]}}}"#,
    )]);
    let config = load_tsconfig(&fs, "").expect("config");
    assert_eq!(
        resolve_paths(&fs, &config, "", "@app/lib/format"),
        Some(Resolved::File("src/lib/format.ts".into()))
    );
}

#[test]
fn inherits_paths_through_extends() {
    let fs = fs_with(&[
        (
            "tsconfig.base.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
        ),
        ("tsconfig.json", r#"{"extends":"./tsconfig.base.json"}"#),
    ]);
    let config = load_tsconfig(&fs, "").expect("config");
    assert_eq!(
        resolve_paths(&fs, &config, "", "@app/lib/format"),
        Some(Resolved::File("src/lib/format.ts".into()))
    );
}

#[test]
fn a_child_paths_block_replaces_the_parent_entirely() {
    // tsc does a shallow merge of compilerOptions, so a redefined `paths`
    // wins outright. Merging key-by-key would resolve specifiers tsc rejects.
    let fs = fs_with(&[
        (
            "tsconfig.base.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
        ),
        (
            "tsconfig.json",
            r#"{"extends":"./tsconfig.base.json","compilerOptions":{"paths":{"@only/*":["src/*"]}}}"#,
        ),
    ]);
    let config = load_tsconfig(&fs, "").expect("config");
    assert_eq!(
        resolve_paths(&fs, &config, "", "@only/lib/format"),
        Some(Resolved::File("src/lib/format.ts".into()))
    );
    assert_eq!(resolve_paths(&fs, &config, "", "@app/lib/format"), None);
}

#[test]
fn parses_a_config_with_comments_and_trailing_commas() {
    let fs = fs_with(&[(
        "tsconfig.json",
        r#"{
            // the base url matters
            "compilerOptions": {
                "baseUrl": ".",
                /* block comment */
                "paths": { "@app/*": ["src/*"], },
            },
        }"#,
    )]);
    let config = load_tsconfig(&fs, "").expect("real tsconfigs contain comments");
    assert_eq!(
        resolve_paths(&fs, &config, "", "@app/lib/format"),
        Some(Resolved::File("src/lib/format.ts".into()))
    );
}

#[test]
fn falls_back_to_jsconfig() {
    let fs = fs_with(&[(
        "jsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
    )]);
    assert!(load_tsconfig(&fs, "").is_some());
}

#[test]
fn returns_none_for_a_specifier_no_mapping_matches() {
    let fs = fs_with(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
    )]);
    let config = load_tsconfig(&fs, "").expect("config");
    assert_eq!(resolve_paths(&fs, &config, "", "react"), None);
}

#[test]
fn a_cyclic_extends_chain_terminates() {
    let fs = fs_with(&[
        ("tsconfig.json", r#"{"extends":"./b.json"}"#),
        ("b.json", r#"{"extends":"./tsconfig.json"}"#),
    ]);
    let _ = load_tsconfig(&fs, ""); // must return, not hang or overflow
}
