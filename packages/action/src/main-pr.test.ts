import { describe, expect, test, beforeEach, afterEach, vi } from 'vitest';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

const EVENT = path.join(os.tmpdir(), `blast-event-${process.pid}.json`);

function setEvent(payload: unknown): void {
  fs.writeFileSync(EVENT, JSON.stringify(payload));
  process.env['GITHUB_EVENT_PATH'] = EVENT;
  process.env['GITHUB_REPOSITORY'] = 'o/r';
}

beforeEach(() => {
  delete process.env['INPUT_COMMENT'];
  delete process.env['INPUT_PATHS'];
  delete process.env['GITHUB_TOKEN'];
});

afterEach(() => {
  delete process.env['GITHUB_EVENT_PATH'];
  delete process.env['GITHUB_REPOSITORY'];
  delete process.env['GITHUB_TOKEN'];
  try {
    fs.rmSync(EVENT, { force: true });
  } catch {
    // Already gone; nothing to clean.
  }
});

async function freshMain() {
  vi.resetModules();
  return import('./main.js');
}

describe('main on PR events', () => {
  test('posts the rendered body through the poster', async () => {
    setEvent({ pull_request: { number: 7 } });
    process.env['GITHUB_TOKEN'] = 'token';
    const { main } = await freshMain();
    const seen: Array<{ body: string; pr: number }> = [];
    const lister = (_token: string) => async () => [{ filename: 'src/a.ts', status: 'modified' }];
    await main(
      async () => ({ body: '<!-- blast-radius -->\ntest' }),
      async (args) => {
        seen.push({ body: args.body, pr: args.pr });
        return 'created';
      },
      lister,
    );
    expect(seen.length).toBe(1);
    expect(seen[0]?.pr).toBe(7);
    expect(seen[0]?.body).toContain('<!-- blast-radius -->');
  });

  test('a missing token on a PR warns and exits zero, never 401', async () => {
    setEvent({ pull_request: { number: 7 } });
    const { main } = await freshMain();
    await main(async () => ({ body: '' }));
    expect(process.exitCode ?? 0).toBe(0);
  });

  test('comment: false skips the poster entirely', async () => {
    setEvent({ pull_request: { number: 7 } });
    process.env['GITHUB_TOKEN'] = 'token';
    process.env['INPUT_COMMENT'] = 'false';
    const { main } = await freshMain();
    let called = 0;
    let execSeen: unknown;
    const lister = (_token: string) => async () => [{ filename: 'src/a.ts', status: 'modified' }];
    await main(
      async (options) => {
        execSeen = options;
        return { body: 'x' };
      },
      async () => {
        called += 1;
        return 'created';
      },
      lister,
    );
    expect(called).toBe(0);
    expect(execSeen).toMatchObject({ changed: ['src/a.ts'] });
  });
});
