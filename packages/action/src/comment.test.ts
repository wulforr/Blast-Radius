import { describe, expect, test } from 'vitest';
import { MARKER, renderComment, type CommentData } from './comment.js';

function base(overrides: Partial<CommentData> = {}): CommentData {
  return {
    changedCount: 4,
    reached: [
      { path: 'src/index.ts', depth: 3 },
      { path: 'src/pricing/total.ts', depth: 1 },
    ],
    classification: { routes: ['app/pay/page.tsx'], tests: ['src/x.test.ts'], publicApi: [] },
    untested: [],
    stats: { files: 5, edges: 3, unresolved: 0, dynamicGaps: 0, parseFailures: 0 },
    unresolved: [],
    maxDepth: 3,
    ...overrides,
  };
}

describe('renderComment', () => {
  test('matches the spec mock line for line on the summary', () => {
    const body = renderComment(
      base({
        changedCount: 4,
        reached: new Array(38).fill(0).map((_, i) => ({ path: `m${i}.ts`, depth: 1 })),
        classification: {
          routes: new Array(6).fill(0).map((_, i) => `r${i}.ts`),
          tests: new Array(12).fill(0).map((_, i) => `t${i}.test.ts`),
          publicApi: [],
        },
      }),
    );
    expect(body.startsWith(`${MARKER}\n### 🔴 Blast radius\n`)).toBe(true);
    expect(body).toContain('**4 files changed** → **38 modules**, **6 routes**, **12 tests** affected.');
    expect(body).not.toContain('⚠️');
  });

  test('the untested warning appears only with untested modules, singular grammar intact', () => {
    const one = renderComment(base({ untested: ['src/a.ts'] }));
    expect(one).toContain('⚠️ **1 affected module has no test coverage in their reach.**');
    const many = renderComment(base({ untested: ['src/a.ts', 'src/b.ts'] }));
    expect(many).toContain('⚠️ **2 affected modules have no test coverage in their reach.**');
  });

  test('section lists cap at 20 lines with an overflow line', () => {
    const routes = new Array(45).fill(0).map((_, i) => `src/r${i}.ts`);
    const body = renderComment(base({ classification: { routes, tests: [], publicApi: [] } }));
    expect(body).toContain('…and 25 more');
    expect(body.match(/src\/r\d+\.ts/g)?.length).toBe(20);
  });

  test('hostile paths render as text, never as HTML', () => {
    const body = renderComment(
      base({
        reached: [{ path: '<script>alert(1)</script>.ts', depth: 1 }],
        classification: { routes: [], tests: [], publicApi: [] },
      }),
    );
    expect(body).not.toContain('<script>');
    expect(body).toContain('&lt;script&gt;');
  });

  test('the tree honours max-depth with a deeper notice', () => {
    const body = renderComment(
      base({
        reached: [
          { path: 'a.ts', depth: 1 },
          { path: 'b.ts', depth: 9 },
        ],
        maxDepth: 3,
      }),
    );
    expect(body).toContain('a.ts (depth 1)');
    expect(body).not.toContain('b.ts (depth 9)');
    expect(body).toContain('…and 1 more below depth 3');
  });
});
