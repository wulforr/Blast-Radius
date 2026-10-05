import { describe, expect, test, beforeEach, afterEach } from 'vitest';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { collectFiles } from './collect.js';

let workspace = '';

beforeEach(() => {
  workspace = fs.mkdtempSync(path.join(os.tmpdir(), 'blast-collect-'));
  const write = (rel: string, body = 'export {}\n') => {
    fs.mkdirSync(path.dirname(path.join(workspace, rel)), { recursive: true });
    fs.writeFileSync(path.join(workspace, rel), body);
  };
  write('src/index.ts');
  write('src/deep/nested/thing.tsx');
  write('src/legacy.js', 'module.exports = {}\n');
  write('src/types.d.ts', 'declare const x: number\n');
  write('src/styles.css', 'body{}\n');
  write('node_modules/left-pad/index.js');
  write('dist/bundle.js');
  // A directory named *.ts: readdir finds it, readFileSync fails, warn + skip.
  fs.mkdirSync(path.join(workspace, 'src/broken.ts'));
});

afterEach(() => {
  fs.rmSync(workspace, { recursive: true, force: true });
});

describe('collectFiles', () => {
  test('collects analysable sources only, workspace-relative keys', () => {
    expect(Object.keys(collectFiles(workspace, ['.'])).sort()).toEqual([
      'src/deep/nested/thing.tsx',
      'src/index.ts',
      'src/legacy.js',
    ]);
  });

  test('skips roots escaping the workspace', () => {
    expect(collectFiles(workspace, ['..', '/etc', '.'])).toEqual(collectFiles(workspace, ['.']));
  });

  test('deduplicates overlapping roots', () => {
    expect(collectFiles(workspace, ['src', 'src/deep'])).toEqual(
      collectFiles(workspace, ['src']),
    );
  });
});
