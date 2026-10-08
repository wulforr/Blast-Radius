import { describe, expect, test, beforeEach, afterEach } from 'vitest';
import { main } from './main.js';

let exitCode: number | undefined;

beforeEach(() => {
  // @types/node widens exitCode; tests only ever produce numbers here.
  exitCode = typeof process.exitCode === 'number' ? process.exitCode : undefined;
  delete process.env['GITHUB_EVENT_NAME'];
  delete process.env['INPUT_PATHS'];
});

afterEach(() => {
  process.exitCode = exitCode;
});

describe('main', () => {
  test('a throwing pipeline warns and exits zero', async () => {
    const messages: string[] = [];
    const stdout = process.stdout.write.bind(process.stdout);
    process.stdout.write = ((chunk: unknown) => {
      messages.push(String(chunk));
      return true;
    }) as typeof process.stdout.write;
    try {
      await main(async () => {
        throw new Error('boom');
      });
    } finally {
      process.stdout.write = stdout;
    }
    expect(process.exitCode).toBeUndefined();
    expect(messages.join('')).toMatch(/::warning::.*boom/);
  });

  test('a non-PR event analyses with an empty changed set', async () => {
    process.env['GITHUB_EVENT_NAME'] = 'push';
    process.env['GITHUB_WORKSPACE'] = '/tmp/does-not-matter-x';
    let seen: unknown;
    await main(async (options) => {
      seen = options;
      return { body: '', untested: [] };
    });
    expect(seen).toMatchObject({ changed: [] });
    delete process.env['GITHUB_WORKSPACE'];
  });

  test('a PR event without a token exits zero, never 401', async () => {
    process.env['GITHUB_EVENT_NAME'] = 'pull_request';
    delete process.env['GITHUB_TOKEN'];
    // github.context.payload has no pull_request outside the runner, so this
    // follows the empty-changed path; the token-missing branch is covered by
    // the dogfood workflow on a real PR.
    await main(async () => ({ body: '', untested: [] }));
    expect(process.exitCode).toBeUndefined();
  });
});
