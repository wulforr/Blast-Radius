# Blast Radius Phase 1: Analyzer Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Given a directory and a list of changed files, return the set of modules those changes transitively reach, plus an honest account of what could not be resolved.

**Architecture:** A pure Rust library. It walks a source tree, parses each file's imports with `oxc`, resolves every specifier to a concrete path through a specified ladder, builds forward and reverse edge lists, and does a breadth-first walk over the reverse edges from the changed set. A thin `wasm-bindgen` layer exposes one JSON-in, JSON-out function.

**Tech Stack:** Rust 2021 (MSRV 1.82) · `oxc_parser` 0.149 · `serde` · wasm-pack → `wasm32-unknown-unknown`

**Spec:** `docs/SPEC.md` — read it alongside this plan. **Section 0 first** (why this is an Action, not an App) and **section 4** (the resolution ladder, which is normative).

**Covers:** Spec milestones M0–M4.
**Explicitly out of scope for this phase:** the JS Action wrapper (M5), classification into routes/tests/public API (M6), untested-path detection (M7), the landing page (M8), the Worker and gallery (M9), and Marketplace publishing (M10). None of them are touched here.

## Global Constraints

- **The analyzer knows nothing about GitHub.** No octokit, no webhook types, no PR concepts. It takes a directory and a list of changed paths. This boundary is what makes it testable in plain Rust, and `CLAUDE.md` requires it.
- **Never report a confident blast radius over a broken graph.** Unresolved specifiers and dynamic-import gaps are counted and returned in every result. A caller must be able to see that resolution largely failed.
- **Traversal must tolerate cycles.** Real codebases have them, and a stack overflow inside someone's CI is the worst bug this project could ship. Every traversal is iterative with a visited set — never recursive.
- **`unsafe_code = "forbid"`**, already set in `packages/analyzer/Cargo.toml`. Do not relax it.
- **No panics in library code.** `clippy::unwrap_used`, `expect_used` and `indexing_slicing` are declared as inner attributes in `src/lib.rs` so they bind to the library only. Tests may use `expect()` freely.
- **Deterministic output.** Iterate `BTreeMap`/`BTreeSet`, never `HashMap`, anywhere a result is serialised. A graph that serialises differently between runs makes snapshot tests useless and diffs unreadable.
- Performance budget from spec §4: under 30 seconds for a 5,000-file repository. Not measured until Task 10; do not prematurely optimise before it.

## Interface conventions

- A file is identified by a `FileId(u32)`, an index into `Graph::files`. Paths cross the API boundary as strings; everything internal uses ids.
- All paths stored in the graph are **relative to the analysis root**, normalised to forward slashes. Absolute paths never enter the graph — they would leak the runner's directory layout into committed snapshots.
- Anything that reads the filesystem takes a `&dyn FileSystem` (Task 3). No module calls `std::fs` directly. This is what lets every resolver test run against an in-memory tree with no temp directories.

## File structure

```
packages/analyzer/
  src/
    lib.rs           public API, lint attributes, analyze() entry point
    fs.rs            FileSystem trait, real + in-memory implementations
    discover.rs      walk a tree, filter to analysable source files
    parse.rs         oxc -> ImportRef list, dynamic gap detection
    resolve/
      mod.rs         the ladder, in the order spec §4 mandates
      relative.rs    relative specifiers, extension candidates, index files
      tsconfig.rs    tsconfig/jsconfig paths, including extends chains
      workspace.rs   pnpm-workspace.yaml and package.json#workspaces
    graph.rs         Graph, forward/reverse edges, unresolved accounting
    reach.rs         reverse-reachability BFS with depth
    wasm.rs          wasm-bindgen surface (exists; extended in Task 10)
  tests/
    fixtures/plain/ paths/ monorepo/ cyclic/
    discover.rs parse.rs resolve_relative.rs resolve_tsconfig.rs
    resolve_workspace.rs graph.rs reach.rs analyze.rs
```

---

### Task 1: Workspace and wasm scaffold — ALREADY DONE

Completed before this plan was written. Recorded here so the numbering matches the milestone list.

- Commit `8e48800` — MIT licence and gitignore
- Commit `69398b1` — cargo workspace with wasm-bindgen surface
- Commit `b3e0e71` — CI builds and smoke-tests `analyzer.wasm` on every push
- Commit `3b489c4` — README stub

Two decisions in that scaffold worth not undoing:

- `crate-type = ["cdylib", "rlib"]`, with `wasm-bindgen` behind `cfg(target_arch = "wasm32")`. `cargo test` on the host therefore never pulls in the bindgen dependency tree, which is precisely what keeps the analyzer testable as plain Rust.
- The `wasm-opt` feature flags in `[package.metadata.wasm-pack.profile.release]`. Without them the build fails with `[wasm-validator error] Bulk memory operations require bulk memory`, because the pinned `wasm-opt` does not enable post-MVP features that current rustc emits unconditionally.

- [x] Done.

---

### Task 2: Fixture repositories

**Files:**
- Create: `packages/analyzer/tests/fixtures/{plain,paths,monorepo,cyclic}/…` exactly as listed below
- Test: `packages/analyzer/tests/fixtures.rs`

**Interfaces:**
- Produces: four fixture trees, referenced by every later task.

Fixtures come first because every subsequent test asserts against them. Create them exactly — an implementer inventing their own contents makes later assertions meaningless.

- [ ] **Step 1: Create the `plain` fixture**

```
tests/fixtures/plain/package.json
  {"name":"plain","version":"0.0.0","private":true}

tests/fixtures/plain/src/index.ts
  import { total } from './pricing'
  export function checkout(): number { return total() }

tests/fixtures/plain/src/pricing/index.ts
  export { total } from './total'

tests/fixtures/plain/src/pricing/total.ts
  import { round } from '../util/round.js'
  export function total(): number { return round(1.005) }

tests/fixtures/plain/src/util/round.ts
  export function round(value: number): number { return Math.round(value * 100) / 100 }

tests/fixtures/plain/src/orphan.ts
  export const unusedByAnyone = true
```

Three things are deliberate here. `./pricing` resolves through a directory index. `'../util/round.js'` uses a `.js` specifier for a `.ts` file, which is what TypeScript's ESM output requires and which a naive resolver gets wrong. And `orphan.ts` has no importers, so a reverse walk from it must reach nothing but itself.

- [ ] **Step 2: Create the `paths` fixture**

```
tests/fixtures/paths/package.json
  {"name":"paths-fixture","private":true}

tests/fixtures/paths/tsconfig.base.json
  {"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"],"@shared":["src/shared/index.ts"]}}}

tests/fixtures/paths/tsconfig.json
  {"extends":"./tsconfig.base.json"}

tests/fixtures/paths/src/main.ts
  import { format } from '@app/lib/format'
  import { CONFIG } from '@shared'
  export const label = format(CONFIG.name)

tests/fixtures/paths/src/lib/format.ts
  export function format(value: string): string { return value.trim() }

tests/fixtures/paths/src/shared/index.ts
  export const CONFIG = { name: 'paths-fixture' }
```

`@shared` is a non-wildcard mapping to an exact file, which is a distinct code path from the `@app/*` wildcard and is routinely missed.

- [ ] **Step 3: Create the `monorepo` fixture**

```
tests/fixtures/monorepo/pnpm-workspace.yaml
  packages:
    - 'packages/*'

tests/fixtures/monorepo/package.json
  {"name":"monorepo-root","private":true}

tests/fixtures/monorepo/packages/ui/package.json
  {"name":"@acme/ui","version":"0.0.0","main":"src/index.ts"}

tests/fixtures/monorepo/packages/ui/src/index.ts
  export { Button } from './Button'

tests/fixtures/monorepo/packages/ui/src/Button.ts
  export const Button = 'button'

tests/fixtures/monorepo/packages/app/package.json
  {"name":"@acme/app","version":"0.0.0","dependencies":{"@acme/ui":"workspace:*"}}

tests/fixtures/monorepo/packages/app/src/main.ts
  import { Button } from '@acme/ui'
  import { readFile } from 'node:fs/promises'
  export { Button, readFile }
```

`node:fs/promises` must be classified external and not traversed — the test for that lives in Task 7.

- [ ] **Step 4: Create the `cyclic` fixture**

```
tests/fixtures/cyclic/package.json
  {"name":"cyclic","private":true}

tests/fixtures/cyclic/src/entry.ts
  import './a'

tests/fixtures/cyclic/src/a.ts
  import './b'
  export const a = 1

tests/fixtures/cyclic/src/b.ts
  import './c'
  export const b = 2

tests/fixtures/cyclic/src/c.ts
  import './a'
  export const c = 3

tests/fixtures/cyclic/src/dynamic.ts
  const which = Math.random() > 0.5 ? './a' : './b'
  export const load = () => import(which)
  export const known = () => import('./c')
```

`dynamic.ts` carries one unresolvable dynamic import and one literal one, so the gap accounting has something real to count.

- [ ] **Step 5: Write a test that the fixtures exist and are well-formed**

Create `packages/analyzer/tests/fixtures.rs`:

```rust
use std::path::Path;

const FIXTURES: [&str; 4] = ["plain", "paths", "monorepo", "cyclic"];

#[test]
fn every_fixture_exists_and_has_a_package_json() {
    for name in FIXTURES {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        assert!(root.is_dir(), "missing fixture: {name}");
        assert!(root.join("package.json").is_file(), "fixture {name} has no package.json");
    }
}

#[test]
fn fixture_json_parses() {
    for name in FIXTURES {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        for entry in walk_json(&root) {
            let text = std::fs::read_to_string(&entry).expect("readable");
            serde_json::from_str::<serde_json::Value>(&text)
                .unwrap_or_else(|e| panic!("{} is not valid json: {e}", entry.display()));
        }
    }
}

fn walk_json(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).expect("readable dir").flatten() {
            let path = entry.path();
            if path.is_dir() { stack.push(path); }
            else if path.extension().is_some_and(|e| e == "json") { out.push(path); }
        }
    }
    out
}
```

- [ ] **Step 6: Run it**

Run: `cargo test -p analyzer --test fixtures`
Expected: 2 passed. If a fixture JSON file fails to parse, fix the fixture.

- [ ] **Step 7: Commit**

```bash
git add packages/analyzer/tests/fixtures packages/analyzer/tests/fixtures.rs
git commit -m "test: add plain, paths, monorepo and cyclic fixture repositories"
```

---

### Task 3: The filesystem seam and source discovery

**Files:**
- Create: `packages/analyzer/src/fs.rs`, `packages/analyzer/src/discover.rs`
- Modify: `packages/analyzer/src/lib.rs`
- Test: `packages/analyzer/tests/discover.rs`

**Interfaces:**
- Produces:
  - `trait FileSystem { fn read(&self, path: &str) -> Option<String>; fn exists(&self, path: &str) -> bool; fn read_dir(&self, path: &str) -> Vec<DirEntry>; }`
  - `struct DirEntry { name: String, is_dir: bool }`
  - `struct RealFs { root: PathBuf }`, `struct MemFs { files: BTreeMap<String, String> }`
  - `discover(fs: &dyn FileSystem, root: &str) -> Vec<String>` — relative, forward-slashed, sorted

Every later module takes `&dyn FileSystem`. That single seam is why the resolver tests need no temp directories.

- [ ] **Step 1: Write the failing test**

Create `packages/analyzer/tests/discover.rs`:

```rust
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
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p analyzer --test discover`
Expected: compile error — `analyzer::fs` and `analyzer::discover` do not exist.

- [ ] **Step 3: Implement the filesystem seam**

Create `packages/analyzer/src/fs.rs`:

```rust
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// Every module that touches files goes through this. Nothing calls `std::fs`
/// directly, which is what lets the resolver suite run entirely in memory.
pub trait FileSystem {
    fn read(&self, path: &str) -> Option<String>;
    fn exists(&self, path: &str) -> bool;
    fn read_dir(&self, path: &str) -> Vec<DirEntry>;
}

pub struct RealFs {
    root: PathBuf,
}

impl RealFs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn full(&self, path: &str) -> PathBuf {
        if path.is_empty() { self.root.clone() } else { self.root.join(path) }
    }
}

impl FileSystem for RealFs {
    fn read(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(self.full(path)).ok()
    }

    fn exists(&self, path: &str) -> bool {
        self.full(path).exists()
    }

    fn read_dir(&self, path: &str) -> Vec<DirEntry> {
        let Ok(entries) = std::fs::read_dir(self.full(path)) else {
            return Vec::new();
        };
        let mut out: Vec<DirEntry> = entries
            .flatten()
            .map(|entry| DirEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: entry.path().is_dir(),
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

#[derive(Default)]
pub struct MemFs {
    files: BTreeMap<String, String>,
}

impl MemFs {
    pub fn from<const N: usize>(entries: [(&str, &str); N]) -> Self {
        Self {
            files: entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    pub fn insert(&mut self, path: &str, contents: &str) {
        self.files.insert(path.to_string(), contents.to_string());
    }
}

impl FileSystem for MemFs {
    fn read(&self, path: &str) -> Option<String> {
        self.files.get(path).cloned()
    }

    fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
            || self.files.keys().any(|k| k.starts_with(&format!("{path}/")))
    }

    fn read_dir(&self, path: &str) -> Vec<DirEntry> {
        let prefix = if path.is_empty() { String::new() } else { format!("{path}/") };
        let mut names: BTreeMap<String, bool> = BTreeMap::new();
        for key in self.files.keys() {
            let Some(rest) = key.strip_prefix(&prefix) else { continue };
            if rest.is_empty() { continue }
            match rest.split_once('/') {
                Some((head, _)) => { names.insert(head.to_string(), true); }
                None => { names.insert(rest.to_string(), false); }
            }
        }
        names.into_iter().map(|(name, is_dir)| DirEntry { name, is_dir }).collect()
    }
}

/// Normalise a path for storage in the graph: forward slashes, no leading `./`.
pub fn normalise(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    text.strip_prefix("./").unwrap_or(&text).to_string()
}
```

- [ ] **Step 4: Implement discovery**

Create `packages/analyzer/src/discover.rs`:

```rust
use crate::fs::FileSystem;

const SOURCE_EXTENSIONS: [&str; 6] = ["ts", "tsx", "js", "jsx", "mjs", "cjs"];

/// Directories never worth walking. Not configurable in phase 1 — every one of
/// these is either generated output or a dependency tree, and walking them
/// turns a 30-second analysis into a several-minute one.
const IGNORED_DIRS: [&str; 10] = [
    "node_modules", "dist", "build", "out", "coverage",
    ".next", ".nuxt", ".output", ".git", "target",
];

pub fn discover(fs: &dyn FileSystem, root: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_string()];

    while let Some(dir) = stack.pop() {
        for entry in fs.read_dir(&dir) {
            let path = if dir.is_empty() { entry.name.clone() } else { format!("{dir}/{}", entry.name) };

            if entry.is_dir {
                if !IGNORED_DIRS.contains(&entry.name.as_str()) {
                    stack.push(path);
                }
                continue;
            }

            if is_analysable(&entry.name) {
                found.push(path);
            }
        }
    }

    found.sort();
    found
}

fn is_analysable(name: &str) -> bool {
    // A .d.ts declares types and contains no runtime imports worth attributing
    // a blast radius to.
    if name.ends_with(".d.ts") || name.ends_with(".d.mts") || name.ends_with(".d.cts") {
        return false;
    }
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| SOURCE_EXTENSIONS.contains(&ext))
}
```

- [ ] **Step 5: Export the modules**

In `packages/analyzer/src/lib.rs`, add `pub mod fs;` and `pub mod discover;`.

- [ ] **Step 6: Run the test**

Run: `cargo test -p analyzer --test discover`
Expected: 5 passed.

- [ ] **Step 7: Commit**

```bash
git add packages/analyzer/src/fs.rs packages/analyzer/src/discover.rs \
        packages/analyzer/src/lib.rs packages/analyzer/tests/discover.rs
git commit -m "feat(analyzer): add filesystem seam and source discovery"
```

---

### Task 4: Import extraction

**Files:**
- Create: `packages/analyzer/src/parse.rs`
- Test: `packages/analyzer/tests/parse.rs`

**Interfaces:**
- Produces:
  - `enum ImportKind { Static, ExportFrom, Require, DynamicLiteral, DynamicExpression }`
  - `struct ImportRef { specifier: Option<String>, kind: ImportKind, line: u32 }` — `specifier` is `None` exactly when `kind` is `DynamicExpression`
  - `struct ParsedFile { imports: Vec<ImportRef>, parse_failed: bool }`
  - `parse_imports(source: &str, path: &str) -> ParsedFile`

A `DynamicExpression` is a **gap**, not an error. It is recorded with its line number and reported, because pretending a computed import does not exist is how a blast radius becomes quietly wrong.

- [ ] **Step 1: Write the failing test**

Create `packages/analyzer/tests/parse.rs`:

```rust
use analyzer::parse::{parse_imports, ImportKind};

fn specifiers(source: &str) -> Vec<String> {
    parse_imports(source, "t.ts").imports.iter().filter_map(|i| i.specifier.clone()).collect()
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
    assert_eq!(specifiers(source), ["./a", "./b", "./c", "./side-effect", "./types"]);
}

#[test]
fn finds_export_from() {
    let source = "export { x } from './x'\nexport * from './y'";
    let parsed = parse_imports(source, "t.ts");
    assert_eq!(specifiers(source), ["./x", "./y"]);
    assert!(parsed.imports.iter().all(|i| matches!(i.kind, ImportKind::ExportFrom)));
}

#[test]
fn finds_require_with_a_literal_argument() {
    let parsed = parse_imports("const a = require('./a')", "t.cjs");
    assert_eq!(parsed.imports.len(), 1);
    assert!(matches!(parsed.imports[0].kind, ImportKind::Require));
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
    assert!(kinds.iter().any(|k| matches!(k, ImportKind::DynamicLiteral)));
    assert!(kinds.iter().any(|k| matches!(k, ImportKind::DynamicExpression)));

    let gap = parsed.imports.iter().find(|i| matches!(i.kind, ImportKind::DynamicExpression)).expect("gap");
    assert!(gap.specifier.is_none(), "a computed import has no knowable specifier");
    assert!(gap.line > 0, "a gap must carry a line number so it can be reported");
}

#[test]
fn records_line_numbers() {
    let parsed = parse_imports("\n\nimport a from './a'", "t.ts");
    assert_eq!(parsed.imports[0].line, 3);
}

#[test]
fn parses_tsx_and_jsx() {
    let parsed = parse_imports("import React from 'react'\nexport const A = () => <div />", "t.tsx");
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
    let parsed = parse_imports("// import x from './fake'\nconst s = \"import y from './also-fake'\"", "t.ts");
    assert!(parsed.imports.is_empty());
}
```

The last test is the reason this uses a real parser rather than a regex, and it is worth having explicitly.

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p analyzer --test parse`
Expected: compile error — `analyzer::parse` does not exist.

- [ ] **Step 3: Add the oxc dependencies**

In `packages/analyzer/Cargo.toml`, add to `[dependencies]`:

```toml
oxc_allocator = { workspace = true }
oxc_ast = { workspace = true }
oxc_ast_visit = { workspace = true }
oxc_parser = { workspace = true }
oxc_span = { workspace = true }
```

- [ ] **Step 4: Implement the parser**

Create `packages/analyzer/src/parse.rs`. Determine `SourceType` from the file extension, parse, then walk the AST collecting import records.

```rust
use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, Expression, Program, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
    Static,
    ExportFrom,
    Require,
    DynamicLiteral,
    /// `import(someVariable)` — the specifier is not knowable statically.
    /// Recorded as a gap rather than dropped.
    DynamicExpression,
}

#[derive(Debug, Clone)]
pub struct ImportRef {
    pub specifier: Option<String>,
    pub kind: ImportKind,
    pub line: u32,
}

#[derive(Debug, Default)]
pub struct ParsedFile {
    pub imports: Vec<ImportRef>,
    pub parse_failed: bool,
}

pub fn parse_imports(source: &str, path: &str) -> ParsedFile {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_default();
    let result = Parser::new(&allocator, source, source_type).parse();

    let mut out = ParsedFile {
        imports: Vec::new(),
        parse_failed: !result.errors.is_empty(),
    };

    collect(&result.program, source, &mut out.imports);
    out
}

fn line_of(source: &str, offset: u32) -> u32 {
    let end = (offset as usize).min(source.len());
    // 1-based, matching every editor and every CI annotation format.
    source.get(..end).map_or(1, |s| s.matches('\n').count() as u32 + 1)
}
```

Then implement `collect`: walk top-level `Statement`s for `ImportDeclaration`, `ExportNamedDeclaration` and `ExportAllDeclaration` with a `source`, and walk all expressions for `ImportExpression` and `CallExpression` where the callee is `require`. Use `oxc_ast_visit::Visit` for the expression walk rather than hand-rolling recursion — a hand-rolled walk will miss imports inside nested functions, class bodies and template literals.

The exact `oxc_ast_visit` trait method names shift between oxc releases. Check the docs for the version pinned in `Cargo.toml` (0.149) and follow them; do not copy a snippet from an older release.

- [ ] **Step 5: Export and run**

Add `pub mod parse;` to `src/lib.rs`.

Run: `cargo test -p analyzer --test parse`
Expected: 8 passed.

- [ ] **Step 6: Commit**

```bash
git add packages/analyzer/src/parse.rs packages/analyzer/src/lib.rs \
        packages/analyzer/Cargo.toml packages/analyzer/tests/parse.rs
git commit -m "feat(analyzer): extract static, require and dynamic imports with oxc"
```

---

### Task 5: Relative resolution

**Files:**
- Create: `packages/analyzer/src/resolve/mod.rs`, `packages/analyzer/src/resolve/relative.rs`
- Test: `packages/analyzer/tests/resolve_relative.rs`

**Interfaces:**
- Produces:
  - `enum Resolved { File(String), External, Unresolved }`
  - `resolve_relative(fs: &dyn FileSystem, importer: &str, specifier: &str) -> Option<Resolved>` — `None` means "not a relative specifier, try the next rung"

**Rung one of the ladder in spec §4.** Extension candidates are tried in this order: exact, `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`, then `/index.*` in the same order.

- [ ] **Step 1: Write the failing test**

Create `packages/analyzer/tests/resolve_relative.rs`:

```rust
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
    assert_eq!(resolve("src/index.ts", "./sibling"), Some(Resolved::File("src/sibling.ts".into())));
}

#[test]
fn resolves_tsx_and_js_when_ts_is_absent() {
    assert_eq!(resolve("src/index.ts", "./widget"), Some(Resolved::File("src/widget.tsx".into())));
    assert_eq!(resolve("src/index.ts", "./legacy"), Some(Resolved::File("src/legacy.js".into())));
}

#[test]
fn resolves_a_directory_through_its_index() {
    assert_eq!(resolve("src/index.ts", "./util"), Some(Resolved::File("src/util/index.ts".into())));
}

#[test]
fn resolves_a_parent_relative_specifier() {
    assert_eq!(resolve("src/util/round.ts", "../sibling"), Some(Resolved::File("src/sibling.ts".into())));
}

#[test]
fn maps_a_js_specifier_onto_a_ts_file() {
    // TypeScript's ESM output requires importing './round.js' for round.ts.
    // A resolver that trusts the extension literally gets every modern TS
    // project wrong.
    assert_eq!(resolve("src/index.ts", "./util/round.js"), Some(Resolved::File("src/util/round.ts".into())));
}

#[test]
fn prefers_ts_over_js_when_both_exist() {
    assert_eq!(resolve("src/index.ts", "./both"), Some(Resolved::File("src/both.ts".into())));
}

#[test]
fn resolves_an_exact_path_that_already_has_an_extension() {
    assert_eq!(resolve("src/index.ts", "./exact.json"), Some(Resolved::File("src/exact.json".into())));
}

#[test]
fn reports_a_missing_relative_target_as_unresolved_not_none() {
    // None means "not my rung". Unresolved means "mine, and it is broken" —
    // the difference is what makes the unresolved count meaningful.
    assert_eq!(resolve("src/index.ts", "./does-not-exist"), Some(Resolved::Unresolved));
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p analyzer --test resolve_relative`
Expected: compile error.

- [ ] **Step 3: Implement**

Create `packages/analyzer/src/resolve/mod.rs`:

```rust
pub mod relative;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    File(String),
    /// A package, a node: builtin, or anything outside the analysis root.
    /// Recorded as a leaf, never traversed.
    External,
    /// This rung owns the specifier and could not find a target.
    Unresolved,
}

pub const EXTENSIONS: [&str; 6] = ["ts", "tsx", "js", "jsx", "mjs", "cjs"];
```

Create `packages/analyzer/src/resolve/relative.rs` with a `try_candidates` helper that, given a base path with no extension, tries the exact path, then each extension, then `index.<ext>` for each extension, returning the first that `fs.exists`. Strip a trailing `.js`/`.mjs`/`.cjs`/`.jsx` from the specifier before generating candidates, so the TypeScript ESM case works — but try the exact path first, so a genuine `.js` file still wins.

Normalise `..` segments by joining the importer's directory with the specifier and collapsing the result manually; do not use `Path::canonicalize`, which touches the real filesystem and would break `MemFs`.

- [ ] **Step 4: Run the test**

Run: `cargo test -p analyzer --test resolve_relative`
Expected: 9 passed.

- [ ] **Step 5: Commit**

```bash
git add packages/analyzer/src/resolve packages/analyzer/src/lib.rs \
        packages/analyzer/tests/resolve_relative.rs
git commit -m "feat(analyzer): resolve relative specifiers, extensions and index files"
```

---

### Task 6: tsconfig paths resolution

**Files:**
- Create: `packages/analyzer/src/resolve/tsconfig.rs`
- Test: `packages/analyzer/tests/resolve_tsconfig.rs`

**Interfaces:**
- Produces:
  - `struct TsConfig { base_url: Option<String>, paths: BTreeMap<String, Vec<String>> }`
  - `load_tsconfig(fs: &dyn FileSystem, root: &str) -> Option<TsConfig>` — follows `extends`
  - `resolve_paths(fs: &dyn FileSystem, config: &TsConfig, root: &str, specifier: &str) -> Option<Resolved>`

**Two behaviours to get right, both routinely wrong in naive implementations.**

`extends` is a shallow merge of `compilerOptions`. A child that defines its own `paths` **replaces** the parent's entirely — it does not merge key by key. Implement the replace semantics and test it, because silently merging produces resolutions that `tsc` itself would not make.

`tsconfig.json` is JSON with comments and trailing commas in practice. A strict `serde_json` parse will fail on real repositories. Strip comments and trailing commas before parsing, and have a test with both.

- [ ] **Step 1: Write the failing test**

Create `packages/analyzer/tests/resolve_tsconfig.rs`:

```rust
use analyzer::fs::MemFs;
use analyzer::resolve::tsconfig::{load_tsconfig, resolve_paths};
use analyzer::resolve::Resolved;

fn fs_with(configs: &[(&str, &str)]) -> MemFs {
    let mut fs = MemFs::from([
        ("src/lib/format.ts", ""),
        ("src/shared/index.ts", ""),
        ("src/main.ts", ""),
    ]);
    for (path, body) in configs { fs.insert(path, body); }
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
        ("tsconfig.base.json", r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#),
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
        ("tsconfig.base.json", r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#),
        ("tsconfig.json", r#"{"extends":"./tsconfig.base.json","compilerOptions":{"paths":{"@only/*":["src/*"]}}}"#),
    ]);
    let config = load_tsconfig(&fs, "").expect("config");
    assert_eq!(resolve_paths(&fs, &config, "", "@only/lib/format"), Some(Resolved::File("src/lib/format.ts".into())));
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
    let fs = fs_with(&[("jsconfig.json", r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#)]);
    assert!(load_tsconfig(&fs, "").is_some());
}

#[test]
fn returns_none_for_a_specifier_no_mapping_matches() {
    let fs = fs_with(&[("tsconfig.json", r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#)]);
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
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p analyzer --test resolve_tsconfig`
Expected: compile error.

- [ ] **Step 3: Implement**

Write a small `strip_jsonc` function (state machine over the characters, tracking whether it is inside a string, a line comment, or a block comment, then remove commas immediately before `}` or `]`). Follow `extends` with a visited set and a depth cap of 16.

Longest-prefix-wins when several patterns match a specifier, matching tsc.

- [ ] **Step 4: Run the test**

Run: `cargo test -p analyzer --test resolve_tsconfig`
Expected: 9 passed.

- [ ] **Step 5: Commit**

```bash
git add packages/analyzer/src/resolve/tsconfig.rs packages/analyzer/src/resolve/mod.rs \
        packages/analyzer/tests/resolve_tsconfig.rs
git commit -m "feat(analyzer): resolve tsconfig paths through extends chains"
```

---

### Task 7: Workspace package resolution and the full ladder

**Files:**
- Create: `packages/analyzer/src/resolve/workspace.rs`
- Modify: `packages/analyzer/src/resolve/mod.rs`
- Test: `packages/analyzer/tests/resolve_workspace.rs`

**Interfaces:**
- Produces:
  - `struct Workspace { packages: BTreeMap<String, String> }` — package name → entry file, relative to root
  - `load_workspace(fs: &dyn FileSystem, root: &str) -> Workspace`
  - `struct Resolver { … }` with `Resolver::new(fs, root)` and `resolve(&self, importer: &str, specifier: &str) -> Resolved`

`Resolver::resolve` is the whole ladder from spec §4, in order: relative → tsconfig paths → workspace packages → external. It is the only entry point Task 8 uses.

- [ ] **Step 1: Write the failing test**

Create `packages/analyzer/tests/resolve_workspace.rs`:

```rust
use analyzer::fs::{MemFs, RealFs};
use analyzer::resolve::{Resolved, Resolver};

fn monorepo() -> MemFs {
    MemFs::from([
        ("pnpm-workspace.yaml", "packages:\n  - 'packages/*'\n"),
        ("package.json", r#"{"name":"root","private":true}"#),
        ("packages/ui/package.json", r#"{"name":"@acme/ui","main":"src/index.ts"}"#),
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
    assert_eq!(resolver.resolve("packages/app/src/main.ts", "node:fs/promises"), Resolved::External);
    assert_eq!(resolver.resolve("packages/app/src/main.ts", "fs"), Resolved::External);
}

#[test]
fn treats_an_unknown_package_as_external() {
    let fs = monorepo();
    let resolver = Resolver::new(&fs, "");
    assert_eq!(resolver.resolve("packages/app/src/main.ts", "react"), Resolved::External);
}

#[test]
fn reads_package_json_workspaces_as_well_as_pnpm() {
    let mut fs = monorepo();
    fs.insert("pnpm-workspace.yaml", "");
    fs.insert("package.json", r#"{"name":"root","workspaces":["packages/*"]}"#);
    let resolver = Resolver::new(&fs, "");
    assert_eq!(
        resolver.resolve("packages/app/src/main.ts", "@acme/ui"),
        Resolved::File("packages/ui/src/index.ts".into())
    );
}

#[test]
fn the_ladder_tries_relative_before_paths() {
    let mut fs = monorepo();
    fs.insert("tsconfig.json", r#"{"compilerOptions":{"baseUrl":".","paths":{"./src/*":["packages/ui/src/*"]}}}"#);
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
    assert_eq!(resolver.resolve("src/main.ts", "@app/lib/format"), Resolved::File("src/lib/format.ts".into()));
    assert_eq!(resolver.resolve("src/main.ts", "@shared"), Resolved::File("src/shared/index.ts".into()));
}
```

The last two tests run the whole ladder against the real fixture trees, which is what catches a resolver that passes in `MemFs` but not on disk.

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p analyzer --test resolve_workspace`
Expected: compile error.

- [ ] **Step 3: Implement**

For `pnpm-workspace.yaml`, do **not** add a YAML dependency for phase 1 — the file shape in practice is a `packages:` key followed by `- 'glob'` lines. Parse those two forms directly and note the limitation in a comment. Expand each glob by listing directories and reading each one's `package.json` for its `name` and `main`. Default the entry to `src/index.ts`, then `index.ts`, then `index.js` when `main` is absent.

Node builtins: match a fixed list plus any `node:` prefix.

- [ ] **Step 4: Run the test**

Run: `cargo test -p analyzer --test resolve_workspace`
Expected: 8 passed.

- [ ] **Step 5: Commit**

```bash
git add packages/analyzer/src/resolve packages/analyzer/tests/resolve_workspace.rs
git commit -m "feat(analyzer): resolve workspace packages and assemble the ladder"
```

---

### Task 8: Graph construction

**Files:**
- Create: `packages/analyzer/src/graph.rs`
- Test: `packages/analyzer/tests/graph.rs`

**Interfaces:**
- Produces:
  - `struct FileId(pub u32)`
  - `struct DynamicGap { file: String, line: u32 }`
  - `struct Graph { files: Vec<String>, forward: Vec<BTreeSet<FileId>>, reverse: Vec<BTreeSet<FileId>>, unresolved: Vec<UnresolvedRef>, dynamic_gaps: Vec<DynamicGap>, parse_failures: Vec<String> }`
  - `struct UnresolvedRef { file: String, specifier: String, line: u32 }`
  - `build_graph(fs: &dyn FileSystem, root: &str) -> Graph`
  - `Graph::id_of(&self, path: &str) -> Option<FileId>`

- [ ] **Step 1: Write the failing test**

Create `packages/analyzer/tests/graph.rs`:

```rust
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
    assert_eq!(forward, reverse, "every edge must appear in both directions");
}

#[test]
fn the_plain_fixture_resolves_every_import() {
    let graph = fixture("plain");
    assert!(graph.unresolved.is_empty(), "unresolved: {:?}", graph.unresolved);
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
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p analyzer --test graph`
Expected: compile error.

- [ ] **Step 3: Implement**

Two passes. First `discover` and assign a `FileId` to every path, so the id space is stable before any edge exists. Then, for each file, parse it, resolve every `ImportRef` through the `Resolver`, and record edges — `Resolved::File` into both directions, `Resolved::Unresolved` into `unresolved`, `Resolved::External` dropped, and every `DynamicExpression` into `dynamic_gaps`.

Use `BTreeSet` for the edge sets: it deduplicates a doubly-imported module and keeps iteration order stable.

- [ ] **Step 4: Run the test**

Run: `cargo test -p analyzer --test graph`
Expected: 9 passed.

- [ ] **Step 5: Commit**

```bash
git add packages/analyzer/src/graph.rs packages/analyzer/src/lib.rs packages/analyzer/tests/graph.rs
git commit -m "feat(analyzer): build forward and reverse import graph with gap accounting"
```

---

### Task 9: Reverse reachability

**Files:**
- Create: `packages/analyzer/src/reach.rs`
- Test: `packages/analyzer/tests/reach.rs`

**Interfaces:**
- Produces:
  - `struct Reached { depths: BTreeMap<String, u32> }`
  - `reverse_reach(graph: &Graph, changed: &[String]) -> Reached`

Depth 0 is a changed file itself; depth 1 is a direct importer. Depth is what makes the eventual PR comment readable, so it is recorded from the start.

**This traversal must be iterative.** The cycle fixture exists to prove it.

- [ ] **Step 1: Write the failing test**

Create `packages/analyzer/tests/reach.rs`:

```rust
use analyzer::fs::RealFs;
use analyzer::graph::build_graph;
use analyzer::reach::reverse_reach;

fn reach(fixture: &str, changed: &[&str]) -> analyzer::reach::Reached {
    let root = format!("{}/tests/fixtures/{fixture}", env!("CARGO_MANIFEST_DIR"));
    let graph = build_graph(&RealFs::new(root), "");
    reverse_reach(&graph, &changed.iter().map(|s| s.to_string()).collect::<Vec<_>>())
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
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p analyzer --test reach`
Expected: compile error.

- [ ] **Step 3: Implement**

```rust
use std::collections::{BTreeMap, VecDeque};
use crate::graph::Graph;

#[derive(Debug, Default)]
pub struct Reached {
    pub depths: BTreeMap<String, u32>,
}

/// Breadth-first over reverse edges. Iterative with a visited set, so an
/// import cycle terminates rather than overflowing the stack — see
/// docs/SPEC.md section 4 and the `cyclic` fixture.
pub fn reverse_reach(graph: &Graph, changed: &[String]) -> Reached {
    let mut depths: BTreeMap<u32, u32> = BTreeMap::new();
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();

    for path in changed {
        // A changed path with no node — a deleted file, a markdown edit — is
        // simply not a seed. It is not an error.
        let Some(id) = graph.id_of(path) else { continue };
        if depths.insert(id.0, 0).is_none() {
            queue.push_back((id.0, 0));
        }
    }

    while let Some((id, depth)) = queue.pop_front() {
        let Some(importers) = graph.reverse.get(id as usize) else { continue };
        for importer in importers {
            // BFS visits in non-decreasing depth order, so the first time a
            // node is seen is its shortest depth.
            if depths.contains_key(&importer.0) { continue }
            depths.insert(importer.0, depth + 1);
            queue.push_back((importer.0, depth + 1));
        }
    }

    Reached {
        depths: depths
            .into_iter()
            .filter_map(|(id, depth)| graph.files.get(id as usize).map(|p| (p.clone(), depth)))
            .collect(),
    }
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p analyzer --test reach`
Expected: 9 passed.

- [ ] **Step 5: Commit**

```bash
git add packages/analyzer/src/reach.rs packages/analyzer/src/lib.rs packages/analyzer/tests/reach.rs
git commit -m "feat(analyzer): reverse reachability with depth and cycle tolerance"
```

---

### Task 10: The public entry point and wasm surface

**Files:**
- Modify: `packages/analyzer/src/lib.rs`, `packages/analyzer/src/wasm.rs`, `scripts/wasm-smoke.mjs`
- Test: `packages/analyzer/tests/analyze.rs`

**Interfaces:**
- Produces:
  - `struct AnalyzeRequest { root: String, changed: Vec<String> }`
  - `struct AnalyzeResult { reached: Vec<ReachedFile>, stats: GraphStats, unresolved: Vec<UnresolvedRef>, dynamic_gaps: Vec<DynamicGap> }`
  - `struct ReachedFile { path: String, depth: u32 }`
  - `struct GraphStats { files: u32, edges: u32, unresolved: u32, dynamic_gaps: u32, parse_failures: u32 }`
  - `analyze(fs: &dyn FileSystem, request: &AnalyzeRequest) -> AnalyzeResult`
  - wasm export `analyze_json(input: &str) -> String`

`GraphStats` is what feeds the comment's "graph health" line in a later phase. It is built now because a result without it can be read as more confident than it is.

- [ ] **Step 1: Write the failing test**

Create `packages/analyzer/tests/analyze.rs`:

```rust
use analyzer::{analyze, AnalyzeRequest};
use analyzer::fs::RealFs;

fn run(fixture: &str, changed: &[&str]) -> analyzer::AnalyzeResult {
    let root = format!("{}/tests/fixtures/{fixture}", env!("CARGO_MANIFEST_DIR"));
    analyze(
        &RealFs::new(root),
        &AnalyzeRequest { root: String::new(), changed: changed.iter().map(|s| s.to_string()).collect() },
    )
}

#[test]
fn returns_reached_files_sorted_by_depth_then_path() {
    let result = run("plain", &["src/util/round.ts"]);
    let depths: Vec<u32> = result.reached.iter().map(|r| r.depth).collect();
    let mut sorted = depths.clone();
    sorted.sort();
    assert_eq!(depths, sorted, "shallowest first — depth is what makes the output readable");
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
    let result = analyze(&fs, &AnalyzeRequest { root: String::new(), changed: vec!["README.md".into()] });
    assert!(result.reached.is_empty());
    assert_eq!(result.stats.files, 0);
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p analyzer --test analyze`
Expected: compile error.

- [ ] **Step 3: Implement**

Derive `serde::Serialize` on every result struct with `#[serde(rename_all = "camelCase")]`, since the consumer is JavaScript. Sort `reached` by `(depth, path)`.

- [ ] **Step 4: Extend the wasm surface**

In `src/wasm.rs`, expose `analyze_json(input: &str) -> String`: deserialise `AnalyzeRequest`, construct a `RealFs` rooted at `request.root`, run `analyze`, serialise the result. On a deserialisation failure return a JSON error object rather than panicking — a panic across the wasm boundary is opaque and would surface as an unhelpful CI failure.

- [ ] **Step 5: Extend the wasm smoke test**

In `scripts/wasm-smoke.mjs`, load the built module, call `analyze_json` against the `plain` fixture with `src/util/round.ts` changed, and assert the reached set contains `src/index.ts` at depth 3. This is the only test that proves the wasm build and the native build agree.

- [ ] **Step 6: Measure against the performance budget**

Spec §4 sets 30 seconds for 5,000 files. Generate a synthetic tree — 5,000 files in a chain-plus-fan-out shape — into a temp directory with a small script, run the analyzer over it, and record the wall-clock time in the README's "Notable engineering decisions" section.

If it exceeds the budget, do **not** start optimising here. Record the number, note where the time went, and let a later phase decide — parallelising with `rayon` is the obvious lever and is a self-contained change.

- [ ] **Step 7: Run everything**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test -p analyzer && wasm-pack build packages/analyzer --target nodejs && node scripts/wasm-smoke.mjs`
Expected: all green.

- [ ] **Step 8: Commit**

```bash
git add packages/analyzer/src packages/analyzer/tests/analyze.rs scripts/wasm-smoke.mjs README.md
git commit -m "feat(analyzer): add analyze entry point and json wasm surface"
```

---

## Phase 1 exit criteria

- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test -p analyzer` all pass.
- [ ] `wasm-pack build` succeeds and `scripts/wasm-smoke.mjs` agrees with the native result.
- [ ] All four fixtures resolve with zero unresolved specifiers.
- [ ] The cyclic fixture terminates — no hang, no stack overflow.
- [ ] Dynamic-import gaps are counted and present in the JSON result.
- [ ] The 5,000-file timing is recorded in the README, whether or not it met the budget.
- [ ] No `unsafe`, no `unwrap` or `expect` in library code, no GitHub concepts anywhere in the analyzer.

## Open questions

- **`exports` field resolution.** Task 7 reads `main` only. Modern packages use `exports` with conditions, which changes which file a bare import lands on. Deferred; note it in the README as a known limitation rather than pretending it works.
- **pnpm-workspace.yaml parsing** is hand-rolled to avoid a YAML dependency. Fine for the common shapes; revisit if a real repository breaks it.
- **Depth cap.** The reach walk is unbounded. On a large monorepo, a change to a shared utility may reach thousands of files. Display already caps at depth 3, but the *walk* may need a cap too — decide once the 5,000-file timing in Task 10 Step 6 is known.
