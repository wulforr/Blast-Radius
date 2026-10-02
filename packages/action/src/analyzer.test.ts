import { describe, expect, test } from 'vitest';
import * as path from 'node:path';
import { analyzeTree, loadAnalyzer } from './analyzer.js';

// Tests run with cwd = packages/action (guaranteed both by `pnpm --filter`
// and by CI's `working-directory:`), so the wasm-pack output is at a fixed
// relative path. `__dirname` is deliberately unused: vitest runs modules as
// ESM, where it does not exist.
const PKG = path.resolve(process.cwd(), '../analyzer/pkg');
const PLAIN: Record<string, string> = {
  'src/index.ts': "import { total } from './pricing'\nexport function checkout(): number { return total() }\n",
  'src/pricing/index.ts': "export { total } from './total'\n",
  'src/pricing/total.ts': "import { round } from '../util/round.js'\nexport function total(): number { return round(1.005) }\n",
  'src/util/round.ts':
    'export function round(value: number): number { return Math.round(value * 100) / 100 }\n',
  'src/orphan.ts': 'export const unusedByAnyone = true\n',
};

describe('loadAnalyzer', () => {
  test('loads the real wasm-pack output', () => {
    expect(typeof loadAnalyzer(PKG).analyzeJson).toBe('function');
  });

  test('refuses a directory with no glue', () => {
    expect(() => loadAnalyzer('/nonexistent-dir-xyz')).toThrow(/wasm-pack build/);
  });
});

describe('analyzeTree', () => {
  test('agrees with the Rust suite: index.ts at depth 3, clean graph', () => {
    const result = analyzeTree(PKG, PLAIN, ['src/util/round.ts']);
    expect(result.reached.find((r) => r.path === 'src/index.ts')).toEqual({
      path: 'src/index.ts',
      depth: 3,
    });
    expect(result.stats.files).toBe(5);
    expect(result.unresolved).toEqual([]);
  });

  test('surfaces wasm error objects as thrown Errors', () => {
    expect(() => analyzeTree(PKG, PLAIN, ['src/util/round.ts'].map(() => 42) as unknown as string[])).toThrow();
  });
});
