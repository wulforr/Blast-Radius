import { describe, expect, test } from 'vitest';
import { getChangedFiles, type FileChange } from './diff.js';

function fake(changes: FileChange[]) {
  return async () => changes;
}

describe('getChangedFiles', () => {
  test('returns new names, excluding removed files', async () => {
    const changed = await getChangedFiles(
      fake([
        { filename: 'src/a.ts', status: 'modified' },
        { filename: 'src/old.ts', status: 'removed' },
        { filename: 'src/new.ts', status: 'renamed' },
        { filename: 'src/added.ts', status: 'added' },
      ]),
      'owner',
      'repo',
      7,
    );
    expect(changed).toEqual(['src/a.ts', 'src/new.ts', 'src/added.ts']);
  });

  test('an empty diff is an empty list, not an error', async () => {
    await expect(getChangedFiles(fake([]), 'o', 'r', 1)).resolves.toEqual([]);
  });

  test('API failures propagate to the caller (main.ts turns them into warnings)', async () => {
    await expect(
      getChangedFiles(async () => {
        throw new Error('401 Unauthorized');
      }, 'o', 'r', 1),
    ).rejects.toThrow('401 Unauthorized');
  });
});
