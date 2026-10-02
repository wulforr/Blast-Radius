#!/usr/bin/env node
// Copies the wasm-pack output next to the ncc bundle and refuses to ship a
// stale glue file. Exits non-zero (failing the build, never a user's CI)
// when analyzer.js has no analyzeJson export or the .wasm is missing.
import { copyFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = resolve(here, '../../analyzer/pkg');
const dist = resolve(here, '../dist');

const glue = resolve(pkg, 'analyzer.js');
const wasm = resolve(pkg, 'analyzer_bg.wasm');

if (!existsSync(glue) || !existsSync(wasm)) {
  console.error(`copy-wasm: missing wasm-pack output in ${pkg}. Run wasm-pack build first.`);
  process.exit(1);
}
if (!readFileSync(glue, 'utf8').includes('analyzeJson')) {
  console.error('copy-wasm: analyzer.js has no analyzeJson export — stale pkg, rebuild with wasm-pack.');
  process.exit(1);
}

mkdirSync(dist, { recursive: true });
copyFileSync(glue, resolve(dist, 'analyzer.js'));
copyFileSync(wasm, resolve(dist, 'analyzer_bg.wasm'));
console.log('copy-wasm: ok');
