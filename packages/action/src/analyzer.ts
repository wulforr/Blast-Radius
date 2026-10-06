import { createRequire } from 'node:module';
import * as path from 'node:path';

export interface ReachedFile {
  path: string;
  depth: number;
}
export interface GraphStats {
  files: number;
  edges: number;
  unresolved: number;
  dynamicGaps: number;
  parseFailures: number;
}
export interface UnresolvedRef {
  file: string;
  specifier: string;
  line: number;
}
export interface DynamicGap {
  file: string;
  line: number;
}
export interface AnalyzeResult {
  reached: ReachedFile[];
  stats: GraphStats;
  unresolved: UnresolvedRef[];
  dynamicGaps: DynamicGap[];
  edges: Array<[string, string]>;
}

export interface AnalyzerModule {
  analyzeJson(input: string): string;
}

function isAnalyzeResult(value: unknown): value is AnalyzeResult {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    Array.isArray(v['reached']) &&
    typeof v['stats'] === 'object' &&
    v['stats'] !== null &&
    Array.isArray(v['unresolved']) &&
    Array.isArray(v['dynamicGaps'])
  );
}

/**
 * Load the wasm-pack glue by path, not by bundle. `analyzer.js` reads
 * `${__dirname}/analyzer_bg.wasm` synchronously at require time, so the two
 * files must stay siblings — in `pkg/` during tests, in `dist/` at runtime.
 * The specifier is computed so `ncc` cannot statically resolve (and inline)
 * it; inlining would detach the glue from its `.wasm` file.
 *
 * `__filename` is guarded because vitest runs modules as ESM (where it does
 * not exist); under `ncc` output and plain Node CJS it always does.
 */
export function loadAnalyzer(pkgDir: string): AnalyzerModule {
  const base: string =
    typeof __filename === 'string' ? __filename : `${process.cwd()}/`;
  const require = createRequire(base);
  const gluePath = path.join(pkgDir, 'analyzer.js');
  let glue: unknown;
  try {
    glue = require(gluePath);
  } catch (error) {
    throw new Error(
      `Could not load the analyzer WASM glue at ${gluePath}. Run \`wasm-pack build packages/analyzer --target nodejs --out-dir pkg --out-name analyzer --release\` first. Cause: ${String(error)}`,
    );
  }
  const module = glue as Partial<AnalyzerModule>;
  if (typeof module.analyzeJson !== 'function') {
    throw new Error(
      `Analyzer glue at ${gluePath} has no analyzeJson export — it is stale. Rebuild it with wasm-pack (see above).`,
    );
  }
  return { analyzeJson: module.analyzeJson.bind(glue) };
}

export function analyzeTree(
  pkgDir: string,
  files: Record<string, string>,
  changed: string[],
): AnalyzeResult {
  const analyzer = loadAnalyzer(pkgDir);
  const raw = analyzer.analyzeJson(JSON.stringify({ changed, files }));
  const parsed: unknown = JSON.parse(raw);
  if (
    typeof parsed === 'object' &&
    parsed !== null &&
    'error' in parsed &&
    typeof (parsed as Record<string, unknown>)['error'] === 'string'
  ) {
    throw new Error(`Analyzer failed: ${(parsed as Record<string, unknown>)['error']}`);
  }
  if (!isAnalyzeResult(parsed)) {
    throw new Error('Analyzer returned an unrecognised JSON shape.');
  }
  return parsed;
}
