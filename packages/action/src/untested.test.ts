import { describe, expect, test } from 'vitest';
import { findUntested } from './untested.js';
import type { ReachedFile } from './analyzer.js';

function at(...paths: string[]): ReachedFile[] {
  return paths.map((path) => ({ path, depth: 1 }));
}

describe('findUntested', () => {
  test('a module reached by a test is covered', () => {
    const reached = at('src/util.ts', 'src/app.ts', 'src/app.test.ts');
    const edges: Array<[string, string]> = [
      ['src/app.ts', 'src/util.ts'],
      ['src/app.test.ts', 'src/app.ts'],
    ];
    expect(findUntested(reached, edges)).toEqual([]);
  });

  test('a module no test reaches is flagged, with the uncovered importer too', () => {
    const reached = at('src/lonely.ts', 'src/holder.ts');
    const edges: Array<[string, string]> = [['src/holder.ts', 'src/lonely.ts']];
    expect(findUntested(reached, edges)).toEqual(['src/holder.ts', 'src/lonely.ts']);
  });

  test('a test file covers itself and is never flagged', () => {
    expect(findUntested(at('src/a.test.ts'), [])).toEqual([]);
  });

  test('an import cycle terminates with every node flagged', () => {
    const reached = at('src/a.ts', 'src/b.ts');
    const edges: Array<[string, string]> = [
      ['src/a.ts', 'src/b.ts'],
      ['src/b.ts', 'src/a.ts'],
    ];
    expect(findUntested(reached, edges)).toEqual(['src/a.ts', 'src/b.ts']);
  });

  test('output is sorted regardless of walk order', () => {
    const reached = at('src/z.ts', 'src/a.ts', 'src/m.ts');
    expect(findUntested(reached, [])).toEqual(['src/a.ts', 'src/m.ts', 'src/z.ts']);
  });

  test('a 2,000-module chain completes with the right answer', () => {
    const reached: ReachedFile[] = [];
    const edges: Array<[string, string]> = [];
    for (let i = 0; i < 2000; i += 1) {
      reached.push({ path: `m${i}.ts`, depth: i });
      if (i > 0) edges.push([`m${i}.ts`, `m${i - 1}.ts`]);
    }
    // No tests anywhere: everything is flagged, and it terminates.
    const untested = findUntested(reached, edges);
    expect(untested.length).toBe(2000);
    expect(untested[0]).toBe('m0.ts');
  });
});
