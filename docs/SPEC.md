# Blast Radius — design specification

> A pull request touches four files. Blast Radius tells you the forty modules,
> eleven routes, and three untested paths that those four files actually reach.

Status: design approved, not yet implemented.
Visual direction: terminal-adjacent — near-black, monospace throughout, one alarm colour used only when something is wrong. See the root README.

---

## 0. Deviation from the original sketch — read this first

The idea was pitched as a **GitHub App** backed by Cloudflare Workers. That does
not survive the $0 constraint, and the reason is worth writing down because it is
the most interesting decision in the project.

Cloudflare's free Workers plan enforces a hard CPU-time ceiling per invocation
measured in milliseconds. Building an import graph for a real repository means
fetching a tarball, decompressing it, and parsing hundreds of source files. That
is seconds of CPU, not milliseconds. No amount of cleverness closes that gap, and
routing the work to a queue or a container reintroduces a monthly bill.

**Resolution:** ship it as a **GitHub Action** instead. The analysis runs on
GitHub's runner, which is free and generous for public repositories, and the
comment is posted with the workflow's own `GITHUB_TOKEN`. There is no server in
the critical path at all.

A small Cloudflare Worker is still part of the system, but only for an optional
**public gallery** — repos that opt in POST their graph snapshot to it, and the
landing page renders real graphs from real projects. That workload is a signed
JSON insert, which fits inside the free CPU budget comfortably.

The install story weakens slightly (add a workflow file, rather than click
Install) and strengthens in one way: an Action is auditable and needs no
third-party access to anyone's code. Say so on the landing page.

*Alternative considered and rejected:* a GitHub App that fires `repository_dispatch`
into a central repo of mine, spending my Actions minutes on strangers' analyses.
Free until it isn't, and an obvious abuse target.

---

## 1. Why this exists

Diff size is a terrible proxy for risk. A one-line change to a shared utility can
reach half a codebase; a three-hundred-line change to a leaf component reaches
nothing. Reviewers have no cheap way to tell these apart, so they use file count,
which is wrong.

Blast Radius computes the actual reverse-reachable set of a diff and reports it
in the PR, along with the part reviewers care about most: **which of the affected
code has no test covering it.**

### What it proves to a reader

- A genuine graph algorithm — reverse reachability with cycle handling — over a
  structure derived from parsing real source code.
- Rust compiled to WebAssembly, loaded from a JavaScript Action. No Docker, no
  runner-specific binaries, cold start in milliseconds.
- Module resolution done properly: `tsconfig` path aliases, barrel files, index
  resolution, workspace packages. This is where naive implementations fall over.
- Webhook signature verification and an edge API with a real datastore.
- Distribution as a product: Marketplace listing, versioned releases, semver tags.

### Non-goals

- No runtime call-graph analysis. Static imports only.
- No languages beyond TypeScript and JavaScript in v1. The parser is behind an
  interface so Python or Go can follow.
- No attempt to resolve fully dynamic imports. They are detected, counted, and
  reported as graph gaps — honesty beats a wrong answer.
- No code review suggestions. It reports impact; it does not have opinions.

---

## 2. User journeys

### J1 — The ten-second visitor

Lands on the site. Sees a live, pannable dependency graph of a well-known open
source repo with a real PR's blast radius highlighted in red. Reads one sentence.
Understands the product. This is precomputed and static — instant, always up.

### J2 — The engineer who wants it

Copies a six-line workflow snippet from the landing page into
`.github/workflows/blast-radius.yml`. Opens a PR. Gets a comment.

### J3 — The PR author

Sees a comment that says: 4 files changed, 38 modules affected, 6 routes affected,
and **3 affected modules have no test in their reverse-reachable set**. Expands the
collapsed tree to see the paths. Adds a test.

### J4 — The gallery contributor

Sets `gallery: true` in the workflow. Their repo's graph appears on the public
gallery page, which doubles as a growing showcase of the tool working on real code.

---

## 3. Architecture

```
      ┌──────────────── GitHub Actions runner (free) ────────────────┐
      │                                                               │
      │  actions/checkout                                             │
      │        │                                                      │
      │        ▼                                                      │
      │  blast-radius action  (Node 20 JS action)                     │
      │        │                                                      │
      │        ├─► load analyzer.wasm  (Rust, oxc parser)             │
      │        │       ├─ walk source tree                            │
      │        │       ├─ parse imports per file                      │
      │        │       ├─ resolve specifiers → file paths             │
      │        │       ├─ build forward + reverse edge lists          │
      │        │       └─ reverse-reach from changed files            │
      │        │                                                      │
      │        ├─► classify: routes / tests / public API / orphans    │
      │        ├─► render markdown, upsert sticky PR comment          │
      │        ├─► write blast-radius.json as workflow artifact       │
      │        └─► (opt-in) POST signed snapshot ────────┐            │
      └──────────────────────────────────────────────────┼────────────┘
                                                         │
                        ┌────────────────────────────────▼───────────┐
                        │  Cloudflare Worker (Hono)                  │
                        │   POST /snapshots   HMAC-verified, stores  │
                        │   GET  /gallery     list                   │
                        │   GET  /graph/:id   one snapshot           │
                        └────────────────┬───────────────────────────┘
                                         │
                                    ┌────▼────┐
                                    │   D1    │
                                    └─────────┘
                                         ▲
                        ┌────────────────┴───────────────────────────┐
                        │  Landing page + gallery (Cloudflare Pages) │
                        └────────────────────────────────────────────┘
```

Three deployable units, three repos' worth of concern in one repo:
`packages/analyzer` (Rust), `packages/action` (TS), `apps/web` (Astro + the Worker).

---

## 4. The hard part: getting the graph right

A dependency graph that is 90% correct is worse than useless — reviewers will
find one wrong edge and stop trusting the whole comment. The work is in resolution.

### Parsing

`oxc_parser` in Rust. For each file collect: static `import`, `export ... from`,
`require()` with a literal argument, and dynamic `import()` with a literal
argument. Record everything else as a **dynamic gap** with its source location.

### Resolution

Given a specifier and an importing file, resolve to a concrete path by walking, in
order:

1. Relative specifiers, trying extensions `.ts .tsx .js .jsx .mjs .cjs`, then
   `/index.*`.
2. `tsconfig.json` / `jsconfig.json` `compilerOptions.paths`, including `extends`
   chains and multiple wildcard candidates.
3. Workspace packages — `pnpm-workspace.yaml`, `package.json#workspaces` — mapped
   to their `main` / `exports` entry, so a monorepo graph crosses package borders.
4. Anything else is external; recorded as a leaf, not traversed.

Every resolution failure is counted and surfaced. A repo where 30% of imports
failed to resolve should say so loudly rather than report a confidently wrong
blast radius.

### Reverse reachability

Build both edge directions during the walk. Seed the frontier with changed files
from the PR diff. BFS over reverse edges with a visited set — this handles import
cycles for free, which is necessary because real codebases have them.

Track the depth at which each module was reached. Depth is what makes the comment
readable: direct importers are interesting, things nine hops away usually are not.

### Classification

Applied to the reached set:

- **Routes** — detected by convention: `app/**/page.tsx`, `pages/**`,
  `src/routes/**`, and an `express`/`hono` handler heuristic. Convention-based
  detection is imperfect, so the comment says "detected", not "are".
- **Tests** — `*.test.*`, `*.spec.*`, `__tests__/**`.
- **Untested paths** — a reached module with no test file in *its own* reverse-
  reachable set. This is the highest-value output of the whole tool.
- **Public API** — matches `package.json#exports` or a root barrel file. Changes
  reaching these get a distinct warning.

### Performance budget

Target: under 30 seconds wall clock for a 5,000-file repository on a standard
runner. Parsing is parallelised with `rayon`. The graph is cached as a workflow
artifact keyed by the base commit SHA, so a push to an existing PR only reparses
changed files and patches the graph.

---

## 5. The PR comment

One comment per PR, updated in place — found by an HTML marker comment, never
duplicated on force-push. Terse by default, everything else behind `<details>`.

```
### 🔴 Blast radius

**4 files changed** → **38 modules**, **6 routes**, **12 tests** affected.

⚠️ **3 affected modules have no test coverage in their reach.**
   src/lib/pricing/round.ts
   src/lib/pricing/tax.ts
   src/components/Invoice/Total.tsx

<details><summary>Affected routes (6)</summary> … </details>
<details><summary>Reach tree, depth ≤ 3</summary> … </details>
<details><summary>Graph health: 812 imports, 4 unresolved, 2 dynamic</summary> … </details>
```

Design rules: no emoji spam, no ASCII art, no scores out of 10. The comment must
be skimmable in three seconds and must never be the longest thing on the PR page.

### Action inputs

| Input | Default | Purpose |
|-------|---------|---------|
| `paths` | `.` | Roots to analyse |
| `max-depth` | `3` | Reach tree display depth |
| `fail-on-untested` | `false` | Exit non-zero if untested modules are reached |
| `comment` | `true` | Set false for artifact-only mode |
| `gallery` | `false` | Opt in to the public gallery |
| `gallery-token` | — | HMAC secret for gallery submission |

---

## 6. Data model (D1)

```
snapshots
  id           text pk
  repo         text          -- owner/name, public repos only
  commit_sha   text
  created_at   integer
  nodes        integer
  edges        integer
  graph        text          -- gzipped JSON, capped at 512 KB
  unresolved   integer
  dynamic_gaps integer

gallery
  repo         text pk
  latest_id    text fk
  stars        integer       -- fetched once at submit, for ordering
  approved     integer       -- manual gate, default 0
```

`approved` defaults to zero. A public gallery that anyone can push into is a
defacement waiting to happen, so submissions queue until reviewed.

---

## 7. Landing page

Cloudflare Pages, Astro, mostly static with one interactive island.

1. **Hero.** `Blast Radius` / "Your PR changed 4 files. Here's what it actually
   touched." Immediately below: the live graph island showing a real PR from a
   well-known repo, with the reached set glowing red. Pan, zoom, hover a node to
   see its path.
2. **The comment.** A pixel-accurate mock of the PR comment. Most visitors will
   never install anything; this is what they take away.
3. **Install.** The six-line workflow snippet with a copy button. One step.
4. **How it works.** The architecture diagram plus the resolution-order list from
   §4 — that list is the credibility of the whole project.
5. **The hard part.** ~300 words on why the free-tier CPU limit killed the server
   design, and why an Action is better anyway. Engineers respect a documented
   constraint more than a feature list.
6. **Gallery.** Grid of approved public repos with their graphs. Grows over time
   at no cost, and gives the page a reason to be revisited.
7. **Footer.**

Graph rendering: `d3-force` for layout, canvas rather than SVG above ~500 nodes.
Precompute layout server-side at snapshot time so the page never runs a cold
simulation in front of a visitor.

---

## 8. Hosting, cost, and degradation

| Concern | Plan |
|---------|------|
| Analysis compute | GitHub Actions runners. Free for public repos; consumers spend their own minutes on private ones. |
| Gallery API | Cloudflare Workers free tier. Request volume is negligible. |
| Storage | D1 free tier, with a 512 KB cap per snapshot and a retention job that keeps the newest snapshot per repo. |
| Landing page | Cloudflare Pages, static. |

**Degradation path.** The gallery is entirely optional. If the Worker or D1 is
unavailable, the Action logs a warning and succeeds — a broken side-feature must
never fail someone's CI. The landing page ships the hero graph as a committed
JSON file, so it renders with the API down.

### Abuse surface

- Gallery submissions are HMAC-signed and rejected if the repo is private or the
  payload exceeds the cap.
- Snapshots are held unapproved until reviewed. Repo names are rendered as text,
  never as HTML.
- The Action requests `pull-requests: write` and `contents: read` only, and the
  README states exactly why each is needed.

---

## 9. Testing

- **Rust unit tests** on the resolver, one per resolution rule, plus fixture
  repos: a plain project, a `paths`-aliased project, a pnpm monorepo, and a
  project with an import cycle.
- **Snapshot tests** on graph output for each fixture. A resolver regression
  should show up as a diff, not as a mystery.
- **Action integration test** using `act` or a dedicated sandbox repo, asserting
  the comment body against a golden file.
- **Worker tests** with Miniflare against a local D1, covering signature rejection.
- CI runs the Action on its own repository. Dogfooding is both a test and a
  demonstration.

---

## 10. Milestones

| # | Deliverable | Done when |
|---|-------------|-----------|
| M0 | Repo, Rust+wasm-pack toolchain, CI | `analyzer.wasm` builds in CI |
| M1 | Parse imports from a file | Fixture file yields expected specifiers |
| M2 | Resolver: relative + extensions + index | Plain fixture repo resolves 100% |
| M3 | Resolver: tsconfig paths + workspaces | Monorepo fixture resolves 100% |
| M4 | Graph build + reverse reachability | Cycle fixture terminates, correct set |
| M5 | JS Action wrapper, WASM loading, diff input | Runs green on a test PR |
| M6 | Classification + markdown comment | Sticky comment updates on force-push |
| M7 | Untested-path detection | Correctly flags a deliberately untested module |
| M8 | Landing page with static hero graph | Live URL, no API dependency |
| M9 | Worker + D1 + gallery | A second repo appears in the gallery |
| M10 | Marketplace listing, README, GIF | Installable by a stranger |

M8 before M9, again: presentable before clever.

---

## 11. Open questions

- **Layout precomputation** for large graphs may exceed the snapshot size cap.
  If so, store positions at reduced precision, or cluster by directory and render
  the cluster graph with drill-down.
- **Coverage data.** Reading an `lcov` report would make "untested" precise rather
  than heuristic. Worth an optional `coverage-file` input, probably at v1.1.
- **Second language.** Python is the obvious next parser and would prove the
  interface was real. Explicitly deferred, explicitly designed for.
