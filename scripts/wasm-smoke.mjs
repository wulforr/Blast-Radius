#!/usr/bin/env node
// Loads the wasm-pack output and calls across the boundary.
//
// `wasm-pack build` succeeding only proves the module compiled and validated.
// This proves Node can instantiate it and that a value survives the round trip,
// which is the failure mode that would otherwise be discovered by a stranger's
// CI rather than by ours.

import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import { readFileSync } from 'node:fs'

const here = dirname(fileURLToPath(import.meta.url))
const pkg = resolve(here, '../packages/analyzer/pkg/analyzer.js')

const require = createRequire(import.meta.url)
const analyzer = require(pkg)

const info = JSON.parse(analyzer.analyzerInfo())

if (info.name !== 'blast-radius-analyzer') {
  console.error(`wasm smoke: unexpected name ${JSON.stringify(info.name)}`)
  process.exit(1)
}

if (!/^\d+\.\d+\.\d+$/.test(info.version)) {
  console.error(`wasm smoke: unexpected version ${JSON.stringify(info.version)}`)
  process.exit(1)
}

console.log(`wasm smoke: ok — ${info.name} ${info.version}`)

// The only test that proves the wasm build and the native build agree:
// analyse the `plain` fixture through the JSON boundary and check the facts
// the Rust suite pins down — `src/index.ts` is reached at depth 3 from
// `src/util/round.ts`, over a fully-resolved graph.
//
// File contents cross the boundary explicitly: wasm32-unknown-unknown has no
// filesystem, so the caller walks the tree (here: the fixture directory) and
// ships `{ path: contents }`, exactly as the Action will.
const fixtureDir = resolve(here, '../packages/analyzer/tests/fixtures/plain')
const fixtureSources = [
  'src/index.ts',
  'src/pricing/index.ts',
  'src/pricing/total.ts',
  'src/util/round.ts',
  'src/orphan.ts',
]
const files = Object.fromEntries(
  fixtureSources.map((rel) => [rel, readFileSync(resolve(fixtureDir, rel), 'utf8')]),
)
const result = JSON.parse(
  analyzer.analyzeJson(JSON.stringify({ changed: ['src/util/round.ts'], files })),
)

if (result.error) {
  console.error(`wasm smoke: analysis returned an error: ${result.error}`)
  process.exit(1)
}

const reachedIndex = result.reached.find((r) => r.path === 'src/index.ts')

if (!reachedIndex || reachedIndex.depth !== 3) {
  console.error(`wasm smoke: expected src/index.ts at depth 3, got ${JSON.stringify(reachedIndex)}`)
  process.exit(1)
}

if (result.stats.files !== 5 || result.unresolved.length !== 0) {
  console.error(`wasm smoke: expected a clean 5-file graph, got ${JSON.stringify(result.stats)}`)
  process.exit(1)
}

console.log('wasm smoke: ok — analyzeJson reaches src/index.ts at depth 3')
