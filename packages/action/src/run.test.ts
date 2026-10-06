import { describe, expect, test, beforeEach, afterEach } from 'vitest';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { run } from './run.js';

// See analyzer.test.ts: cwd is packages/action when the suite runs.
const PKG = path.resolve(process.cwd(), '../analyzer/pkg');
let workspace = '';

beforeEach(() => {
  workspace = fs.mkdtempSync(path.join(os.tmpdir(), 'blast-run-'));
  const write = (rel: string, body: string) => {
    fs.mkdirSync(path.dirname(path.join(workspace, rel)), { recursive: true });
    fs.writeFileSync(path.join(workspace, rel), body);
  };
  write('src/index.ts', "import { total } from './pricing'\n");
  write('src/pricing/index.ts', "export { total } from './total'\n");
  write('src/pricing/total.ts', "import { round } from '../util/round.js'\nexport function total(): number { return 1 }\n");
  write('src/util/round.ts', 'export function round(v: number): number { return v }\n');
});

afterEach(() => {
  fs.rmSync(workspace, { recursive: true, force: true });
});

describe('run', () => {
  test('writes blast-radius.json with the reached set', async () => {
    const { result, resultPath } = await run({
      workspace,
      roots: ['.'],
      changed: ['src/util/round.ts'],
      analyzerDir: PKG,
      maxDepth: 3,
    });
    expect(resultPath).toBe(path.join(workspace, 'blast-radius.json'));
    expect(result.reached.find((r) => r.path === 'src/index.ts')?.depth).toBe(3);
    const onDisk = JSON.parse(fs.readFileSync(resultPath, 'utf8')) as typeof result;
    expect(onDisk).toEqual(result);
  });

  test('an empty repo writes a valid empty result, not an error', async () => {
    fs.rmSync(path.join(workspace, 'src'), { recursive: true, force: true });
    const { result } = await run({ workspace, roots: ['.'], changed: [], analyzerDir: PKG, maxDepth: 3 });
    expect(result.reached).toEqual([]);
    expect(result.stats.files).toBe(0);
  });
});

describe('run untested detection', () => {
  const TREE: Record<string, string> = {
    'src/util.ts': 'export const u = 1\n',
    'src/app.ts': "import { u } from './util'\nexport const a = u\n",
    'src/app.test.ts': "import { a } from './app'\ntest('a', () => {})\n",
    'src/lonely.ts': 'export const l = 1\n',
    'src/holder.ts': "import { l } from './lonely'\nexport const h = l\n",
  };

  function tree(): string {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'blast-untested-'));
    for (const [rel, body] of Object.entries(TREE)) {
      fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true });
      fs.writeFileSync(path.join(dir, rel), body);
    }
    return dir;
  }

  test('flags a deliberately untested module and spares the covered ones', async () => {
    const workspace = tree();
    try {
      const { result, untested, body } = await run({
        workspace,
        roots: ['.'],
        changed: ['src/util.ts', 'src/lonely.ts'],
        analyzerDir: PKG,
        maxDepth: 3,
      });
      expect(untested).toEqual(['src/holder.ts', 'src/lonely.ts']);
      expect(result.reached.map((r) => r.path).sort()).toEqual(
        ['src/app.test.ts', 'src/app.ts', 'src/holder.ts', 'src/lonely.ts', 'src/util.ts'].sort(),
      );
      expect(body).toContain('⚠️ **2 affected modules have no test coverage in their reach.**');
      expect(body).toContain('src/lonely.ts');
    } finally {
      fs.rmSync(workspace, { recursive: true, force: true });
    }
  });

  test('a PR that only adds tests passes with nothing untested', async () => {
    const workspace = tree();
    try {
      const { untested, body } = await run({
        workspace,
        roots: ['.'],
        changed: ['src/app.test.ts'],
        analyzerDir: PKG,
        maxDepth: 3,
      });
      expect(untested).toEqual([]);
      expect(body).not.toContain('⚠️');
    } finally {
      fs.rmSync(workspace, { recursive: true, force: true });
    }
  });
});
