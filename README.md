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

Early. Milestone M0 (toolchain and CI) is done; the analyzer does not analyse
anything yet. See §10 of the spec for the milestone list.

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

Build the WebAssembly artifact:

```bash
cd packages/analyzer
wasm-pack build --target nodejs --out-dir pkg --out-name analyzer --release
node ../../scripts/wasm-smoke.mjs
```

`pkg/` is generated and git-ignored. The wasm that Actions actually execute is
committed under `packages/action/dist/`, which arrives with milestone M5.

## Licence

MIT — see [`LICENSE`](LICENSE).
