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
