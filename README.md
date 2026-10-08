# Blast Radius

Your PR changed 4 files. Here's what it actually touched.

A GitHub Action that comments on a pull request with the modules, routes, and
untested paths its diff actually reaches. Diff size is a terrible proxy for
risk; the reverse-reachable set of the diff is a much better one.

- Design spec: [`docs/SPEC.md`](docs/SPEC.md)
- Working rules: [`CLAUDE.md`](CLAUDE.md)

Analysis runs on your own runner. There is no server in the critical path, and
no third party is granted access to your code. [`docs/SPEC.md` §0](docs/SPEC.md)
explains why this is an Action rather than a GitHub App.

## Status

Milestones M0–M7 are done: the Action flags reached modules with no test
coverage in the comment and can fail CI on them. The public gallery arrives
with M9. See §10 of the spec for the milestone list.

## Layout

```
packages/analyzer/   Rust. Parsing, resolution, graph, reverse reachability.
packages/action/     TypeScript. Loads the WASM, talks to GitHub, renders markdown.
apps/web/            Astro landing page + gallery.
apps/worker/         Hono on Cloudflare Workers, D1.
fixtures/            Sample repos: plain, tsconfig-paths, pnpm monorepo, cyclic.
```

The analyzer knows nothing about GitHub. It takes a directory and a list of
changed file paths and returns a graph plus a reached set.

## Development

Requires a stable Rust toolchain with the `wasm32-unknown-unknown` target, and
[`wasm-pack`](https://github.com/wasm-bindgen/wasm-pack).

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack        # or install a prebuilt release binary
```

```bash
cargo test                                   # analyzer unit and fixture tests
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

```bash
pnpm --filter @blast-radius/action test    # action unit tests (needs pkg/ built, see below)
pnpm --filter @blast-radius/action run build  # bundles TS + WASM into dist/, which IS committed
```

Build the WebAssembly artifact:

```bash
cd packages/analyzer
wasm-pack build --target nodejs --out-dir pkg --out-name analyzer --release
node ../../scripts/wasm-smoke.mjs
```

`pkg/` is generated and git-ignored. The wasm that Actions actually execute is
committed under `packages/action/dist/`, rebuilt by `pnpm --filter
@blast-radius/action run build` (which runs `wasm-pack` output through `ncc`
and refuses a stale glue file).

## Usage

```yaml
# .github/workflows/blast-radius.yml
permissions:
  contents: read
  pull-requests: write # posts the sticky comment and nothing else
jobs:
  blast-radius:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: shauryasingh/blast-radius@v0
        with:
          paths: '.'
      - uses: actions/upload-artifact@v7
        with:
          name: blast-radius-json
          path: blast-radius.json
```

Analysis runs on your own runner; nothing leaves it in M5 (gallery opt-in
arrives with M9). A failure anywhere in the Action warns and exits zero —
it never fails your CI. Set `fail-on-untested: true` to fail the check when
reached modules lack coverage; the comment is still posted first.

## Notable engineering decisions

- **File contents cross the WASM boundary explicitly.** `wasm32-unknown-unknown`
  has no filesystem, so `analyzeJson` takes `{"changed": [...],
  "files": {"path": "contents"}}` and analyses an in-memory tree. The Action
  walks the checkout in JavaScript and ships the contents over.
- **Performance, measured.** A synthetic 5,001-file tree (import chain plus
  one module imported by every file) analyses in ~190ms wall clock
  (`build_graph` ~189ms, `reverse_reach` <1ms) in a native release build on
  Apple Silicon — two orders of magnitude inside the 30-second budget from
  spec §4, single-threaded. `rayon` parallelisation is deferred until a real
  repository says otherwise.
- **Known limitations of the resolver.** `package.json#exports` conditions are
  not read — only `main` (plus `src/index.ts`, `index.ts`, `index.js`
  fallbacks) locates a workspace entry. `pnpm-workspace.yaml` is parsed by
  hand (a `packages:` key plus `- 'glob'` lines); anything fancier is
  ignored rather than misread.

## Licence

MIT — see [`LICENSE`](LICENSE).
