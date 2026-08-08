use analyzer::fs::{MemFs, RealFs};
use analyzer::resolve::{Resolved, Resolver};

fn monorepo() -> MemFs {
    MemFs::from([
        ("pnpm-workspace.yaml", "packages:\n  - 'packages/*'\n"),
        ("package.json", r#"{"name":"root","private":true}"#),
        (
            "packages/ui/package.json",
            r#"{"name":"@acme/ui","main":"src/index.ts"}"#,
        ),
        ("packages/ui/src/index.ts", ""),
        ("packages/ui/src/Button.ts", ""),
        ("packages/app/package.json", r#"{"name":"@acme/app"}"#),
        ("packages/app/src/main.ts", ""),
    ])
}

#[test]
fn resolves_a_workspace_package_to_its_entry_file() {
    let fs = monorepo();
    let resolver = Resolver::new(&fs, "");
    assert_eq!(
        resolver.resolve("packages/app/src/main.ts", "@acme/ui"),
        Resolved::File("packages/ui/src/index.ts".into())
    );
}

#[test]
fn resolves_a_deep_import_into_a_workspace_package() {
    let fs = monorepo();
    let resolver = Resolver::new(&fs, "");
    assert_eq!(
        resolver.resolve("packages/app/src/main.ts", "@acme/ui/src/Button"),
        Resolved::File("packages/ui/src/Button.ts".into())
    );
}

#[test]
fn treats_a_node_builtin_as_external() {
    let fs = monorepo();
    let resolver = Resolver::new(&fs, "");
    assert_eq!(
        resolver.resolve("packages/app/src/main.ts", "node:fs/promises"),
        Resolved::External
    );
    assert_eq!(
        resolver.resolve("packages/app/src/main.ts", "fs"),
        Resolved::External
    );
}

#[test]
fn treats_an_unknown_package_as_external() {
    let fs = monorepo();
    let resolver = Resolver::new(&fs, "");
    assert_eq!(
        resolver.resolve("packages/app/src/main.ts", "react"),
        Resolved::External
    );
}

#[test]
fn reads_package_json_workspaces_as_well_as_pnpm() {
    let mut fs = monorepo();
    fs.insert("pnpm-workspace.yaml", "");
    fs.insert(
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    let resolver = Resolver::new(&fs, "");
    assert_eq!(
        resolver.resolve("packages/app/src/main.ts", "@acme/ui"),
        Resolved::File("packages/ui/src/index.ts".into())
    );
}

#[test]
fn the_ladder_tries_relative_before_paths() {
    let mut fs = monorepo();
    fs.insert(
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"./src/*":["packages/ui/src/*"]}}}"#,
    );
    fs.insert("packages/app/src/local.ts", "");
    let resolver = Resolver::new(&fs, "");
    assert_eq!(
        resolver.resolve("packages/app/src/main.ts", "./local"),
        Resolved::File("packages/app/src/local.ts".into())
    );
}

#[test]
fn the_monorepo_fixture_resolves_completely() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/monorepo");
    let fs = RealFs::new(root);
    let resolver = Resolver::new(&fs, "");
    assert_eq!(
        resolver.resolve("packages/app/src/main.ts", "@acme/ui"),
        Resolved::File("packages/ui/src/index.ts".into())
    );
}

#[test]
fn the_paths_fixture_resolves_completely() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/paths");
    let fs = RealFs::new(root);
    let resolver = Resolver::new(&fs, "");
    assert_eq!(
        resolver.resolve("src/main.ts", "@app/lib/format"),
        Resolved::File("src/lib/format.ts".into())
    );
    assert_eq!(
        resolver.resolve("src/main.ts", "@shared"),
        Resolved::File("src/shared/index.ts".into())
    );
}
