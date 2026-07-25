# Blast Radius

A GitHub Action that comments on a PR with the modules, routes, and untested paths
its diff actually reaches.

**`docs/SPEC.md` is the source of truth.** Read section 0 first — it explains why
this is an Action and not a GitHub App, and that decision constrains everything
else.

## Hard constraints

1. **A failure in this Action must never fail someone's CI.** Analysis errors,
   gallery submission failures, network problems — log a warning and exit zero.
   The only non-zero exit is when the consumer explicitly sets `fail-on-untested`.
2. **No server in the critical path.** Analysis runs on the GitHub runner. The
   Cloudflare Worker exists only for the optional public gallery; if it is down,
   nothing user-facing breaks.
3. **Never report a confident blast radius over a broken graph.** Unresolved
   imports and dynamic-import gaps are counted and surfaced in the comment. A repo
   where resolution largely failed must say so loudly.
4. **One comment per PR, updated in place.** Found by an HTML marker. Never a
   second comment, never a new one on force-push.
5. **The comment must be skimmable in three seconds.** Summary line, the untested
   warning, everything else collapsed. It must never be the longest thing on the
   PR page.
6. **Gallery submissions are untrusted.** HMAC-verified, size-capped, public repos
   only, and held unapproved until reviewed. Repo names render as text, never HTML.
7. **Least privilege.** The Action asks for `contents: read` and
   `pull-requests: write`. Adding a scope requires a README justification.

## Layout

```
packages/analyzer/   Rust. Parsing, resolution, graph, reverse reachability.
packages/action/     TypeScript. Loads the WASM, talks to GitHub, renders markdown.
apps/web/            Astro landing page + gallery.
apps/worker/         Hono on Cloudflare Workers, D1.
fixtures/            Sample repos: plain, tsconfig-paths, pnpm monorepo, cyclic.
```

The analyzer knows nothing about GitHub. The action knows nothing about parsing.
Keep that boundary — it is what makes the analyzer testable in Rust alone.

## Commands

```
cargo test -p analyzer
cargo build --target wasm32-unknown-unknown --release
pnpm --filter action test
pnpm --filter action build      # bundles WASM + JS into dist/, which IS committed
pnpm --filter web dev
pnpm --filter worker test       # miniflare + local D1
```

`packages/action/dist/` is committed. GitHub Actions run the built artifact
directly; a PR that changes action source without rebuilding dist is incomplete,
and CI checks for that.

## Correctness rules for the analyzer

- Resolution order is specified in `docs/SPEC.md` §4 and is **normative**. Changing
  it changes results for every consumer — update the spec and the fixtures in the
  same commit.
- Every resolution branch gets a fixture test. Resolution bugs are the failure mode
  that destroys trust in the tool.
- Graph traversal must tolerate cycles. Real codebases have them; a stack overflow
  in someone's CI is the worst possible bug here.
- Snapshot tests cover graph output per fixture repo. A resolver change should show
  up as a reviewable diff, never as a silent behaviour change.

## Performance budget

Under 30 seconds wall clock for a 5,000-file repository on a standard runner.
Parsing is parallel. The graph is cached as a workflow artifact keyed by base SHA,
so a push to an existing PR patches rather than reparses.

## Visual direction

Terminal-adjacent: near-black, monospace throughout including headings, sharp
corners, near-monochrome with **one** alarm colour that appears only when something
is genuinely wrong. **Do not** reuse styling from the sibling projects in this
folder — they are deliberately unrelated.

## Out of scope — do not add

Runtime or call-graph analysis, languages beyond TS/JS in v1, resolution of fully
dynamic imports, code-review opinions, and scores out of ten. It reports impact; it
does not have taste.
