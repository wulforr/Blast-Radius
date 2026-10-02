import { describe, expect, test, beforeEach } from 'vitest';
import { getInputs } from './inputs.js';

function setEnv(vars: Record<string, string>): void {
  for (const key of ['INPUT_PATHS', 'INPUT_MAX-DEPTH', 'INPUT_FAIL-ON-UNTESTED', 'INPUT_COMMENT', 'INPUT_GALLERY', 'INPUT_GALLERY-TOKEN']) {
    delete process.env[key];
  }
  for (const [key, value] of Object.entries(vars)) {
    process.env[key] = value;
  }
}

describe('getInputs', () => {
  beforeEach(() => setEnv({}));

  test('applies spec defaults when nothing is set', () => {
    expect(getInputs()).toEqual({
      paths: ['.'],
      maxDepth: 3,
      failOnUntested: false,
      comment: true,
      gallery: false,
      galleryToken: '',
    });
  });

  test('splits multiline and comma-separated paths', () => {
    setEnv({ INPUT_PATHS: 'packages/a, packages/b\napps/web' });
    expect(getInputs().paths).toEqual(['packages/a', 'packages/b', 'apps/web']);
  });

  test('falls back on garbage booleans and depths', () => {
    setEnv({ 'INPUT_FAIL-ON-UNTESTED': 'yes', 'INPUT_MAX-DEPTH': '-2' });
    const inputs = getInputs();
    expect(inputs.failOnUntested).toBe(false);
    expect(inputs.maxDepth).toBe(3);
  });
});
