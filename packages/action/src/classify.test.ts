import { describe, expect, test } from 'vitest';
import { classify, isPublicApi, isRoute, isTest } from './classify.js';

describe('isTest', () => {
  test('matches test conventions only', () => {
    expect(isTest('src/a.test.ts')).toBe(true);
    expect(isTest('src/a.spec.tsx')).toBe(true);
    expect(isTest('__tests__/a.ts')).toBe(true);
    expect(isTest('src/latest.ts')).toBe(false);
    expect(isTest('src/contest.ts')).toBe(false);
  });
});

describe('isRoute', () => {
  test('matches path conventions without reading contents', () => {
    expect(isRoute('app/checkout/page.tsx')).toBe(true);
    expect(isRoute('pages/index.js')).toBe(true);
    expect(isRoute('src/routes/users.ts')).toBe(true);
    expect(isRoute('src/lib/format.ts')).toBe(false);
  });

  test('a handler call counts only with a framework import', () => {
    const handler = "import express from 'express'\napp.get('/x', h)\n";
    expect(isRoute('src/server.ts', handler)).toBe(true);
    // A Map.get is not a route.
    expect(isRoute('src/cache.ts', 'cache.get(key)\n')).toBe(false);
    // A framework import with no handler call is not a route either.
    expect(isRoute('src/app.ts', "import { Hono } from 'hono'\n")).toBe(false);
  });
});

describe('isPublicApi', () => {
  test('a root barrel file is public API', () => {
    expect(isPublicApi('index.ts', {})).toBe(true);
    expect(isPublicApi('src/index.ts', {})).toBe(false);
  });

  test('package.json exports targets match with js-to-ts mapping', () => {
    const files = {
      'package.json': JSON.stringify({ exports: { '.': './index.js', './x': ['./a.js'] } }),
    };
    expect(isPublicApi('index.ts', files)).toBe(true);
    expect(isPublicApi('a.ts', files)).toBe(true);
    expect(isPublicApi('internal.ts', files)).toBe(false);
  });

  test('a broken package.json means no exports match, not a throw', () => {
    expect(isPublicApi('index.ts', { 'package.json': 'not json' })).toBe(true);
    expect(isPublicApi('other.ts', { 'package.json': 'not json' })).toBe(false);
  });
});

describe('classify', () => {
  test('sorts each bucket by path', () => {
    const reached = [
      { path: 'src/z.test.ts', depth: 1 },
      { path: 'src/a.test.ts', depth: 1 },
    ];
    expect(classify(reached, {}).tests).toEqual(['src/a.test.ts', 'src/z.test.ts']);
  });
});
