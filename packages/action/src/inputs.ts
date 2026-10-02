import * as core from '@actions/core';

export interface ActionInputs {
  paths: string[];
  maxDepth: number;
  failOnUntested: boolean;
  comment: boolean;
  gallery: boolean;
  galleryToken: string;
}

function getBoolean(name: string, fallback: boolean): boolean {
  const raw = core.getInput(name).trim().toLowerCase();
  if (raw === '') return fallback;
  // Accept only strict YAML-1.2-core-schema spellings; anything else warns
  // and falls back rather than guessing.
  if (raw === 'true') return true;
  if (raw === 'false') return false;
  core.warning(`Input '${name}' has non-boolean value '${raw}'; using '${fallback}'.`);
  return fallback;
}

export function getInputs(): ActionInputs {
  const paths = core
    .getInput('paths')
    .split(/[\r\n,]+/)
    .map((p) => p.trim())
    .filter((p) => p.length > 0);
  const maxDepth = Number.parseInt(core.getInput('max-depth').trim() || '3', 10);
  return {
    paths: paths.length > 0 ? paths : ['.'],
    maxDepth: Number.isInteger(maxDepth) && maxDepth > 0 ? maxDepth : 3,
    failOnUntested: getBoolean('fail-on-untested', false),
    comment: getBoolean('comment', true),
    gallery: getBoolean('gallery', false),
    galleryToken: core.getInput('gallery-token'),
  };
}
