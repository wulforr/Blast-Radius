import * as core from '@actions/core';
import * as github from '@actions/github';
import { getInputs } from './inputs.js';
import { getChangedFiles } from './diff.js';
import { run, type RunOptions } from './run.js';

type RunFn = (options: RunOptions) => Promise<unknown>;

/**
 * Failure-proof entry point. *Every* error — no token, API outage, unreadable
 * checkout, wasm failure — becomes a warning and exit zero. Hard constraint:
 * this Action must never fail someone's CI. (The sole future exception is
 * `fail-on-untested`, enforced from M7.)
 */
export async function main(exec: RunFn = run): Promise<void> {
  try {
    await execute(exec);
  } catch (error) {
    core.warning(`Blast Radius analysis skipped: ${error instanceof Error ? error.message : String(error)}`);
  }
}

async function execute(exec: RunFn): Promise<void> {
  const inputs = getInputs();
  const workspace = process.env['GITHUB_WORKSPACE'] ?? process.cwd();
  const context = github.context;
  const pr = context.payload.pull_request as { number?: unknown } | undefined;

  let changed: string[];
  if (typeof pr?.number !== 'number') {
    core.warning('No pull_request in the event payload; analysing with an empty changed set.');
    changed = [];
  } else {
    const token = process.env['GITHUB_TOKEN'] ?? '';
    if (token === '') {
      throw new Error('GITHUB_TOKEN is required to list PR files.');
    }
    const octokit = github.getOctokit(token);
    const { owner, repo } = context.repo;
    changed = await getChangedFiles(
      (params) =>
        octokit.paginate(octokit.rest.pulls.listFiles, {
          owner: params.owner,
          repo: params.repo,
          pull_number: params.pull_number,
          per_page: 100,
        }),
      owner,
      repo,
      pr.number,
    );
  }

  await exec({
    workspace,
    roots: inputs.paths,
    changed,
    // In dist/, the glue sits beside the bundle. The guard is for vitest,
    // which runs modules as ESM (no __dirname); the value is unused on the
    // stubbed-exec paths the tests exercise.
    analyzerDir: typeof __dirname === 'string' ? __dirname : process.cwd(),
  });
}

if (typeof require !== 'undefined' && require.main === module) {
  void main();
}
