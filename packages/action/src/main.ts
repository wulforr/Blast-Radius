import * as core from '@actions/core';
import * as github from '@actions/github';
import { getInputs } from './inputs.js';
import { getChangedFiles, type ListFiles } from './diff.js';
import { postComment } from './comment.js';
import { run, type RunOptions } from './run.js';

// Minimal structural type: the real `run` returns the full `RunResult`
// (which carries `body` and `untested`), and test stubs return just those.
type RunFn = (options: RunOptions) => Promise<{ body: string; untested: string[] }>;

type Poster = (args: {
  owner: string;
  repo: string;
  pr: number;
  body: string;
  token: string;
}) => Promise<unknown>;

// Builds the paginated file lister for a token. Injected (like exec and
// poster) so the PR path is testable without network.
type Lister = (token: string) => ListFiles;

function defaultLister(token: string): ListFiles {
  const octokit = github.getOctokit(token);
  return (params) =>
    octokit.paginate(octokit.rest.pulls.listFiles, {
      owner: params.owner,
      repo: params.repo,
      pull_number: params.pull_number,
      per_page: 100,
    });
}

/**
 * Failure-proof entry point. *Every* error — no token, API outage, unreadable
 * checkout, wasm failure — becomes a warning and exit zero. Hard constraint:
 * this Action must never fail someone's CI. (The sole exception is
 * `fail-on-untested`, enforced below after posting.)
 */
export async function main(
  exec: RunFn = run,
  poster: Poster = postComment,
  lister: Lister = defaultLister,
): Promise<void> {
  try {
    await execute(exec, poster, lister);
  } catch (error) {
    core.warning(`Blast Radius analysis skipped: ${error instanceof Error ? error.message : String(error)}`);
  }
}

async function execute(exec: RunFn, poster: Poster, lister: Lister): Promise<void> {
  const inputs = getInputs();
  const workspace = process.env['GITHUB_WORKSPACE'] ?? process.cwd();
  const context = github.context;
  const pr = context.payload.pull_request as { number?: unknown } | undefined;
  const prNumber = typeof pr?.number === 'number' ? pr.number : null;

  // `context.repo` throws when GITHUB_REPOSITORY is unset, so it is read
  // only inside the PR branch — the non-PR path must never touch it (the
  // existing push-event test pins this).
  let target: { owner: string; repo: string; pr: number } | null = null;
  let changed: string[] = [];
  if (prNumber !== null) {
    const token = process.env['GITHUB_TOKEN'] ?? '';
    if (token === '') {
      throw new Error('GITHUB_TOKEN is required to list PR files.');
    }
    const octokitOwner = context.repo;
    changed = await getChangedFiles(
      lister(token),
      octokitOwner.owner,
      octokitOwner.repo,
      prNumber,
    );
    target = { owner: octokitOwner.owner, repo: octokitOwner.repo, pr: prNumber };
  } else {
    core.warning('No pull_request in the event payload; analysing with an empty changed set.');
  }

  const out = await exec({
    workspace,
    roots: inputs.paths,
    changed,
    // In dist/, the glue sits beside the bundle. The guard is for vitest,
    // which runs modules as ESM (no __dirname); the value is unused on the
    // stubbed-exec paths the tests exercise.
    analyzerDir: typeof __dirname === 'string' ? __dirname : process.cwd(),
    maxDepth: inputs.maxDepth,
  });

  if (!inputs.comment) {
    core.info('Comment posting is disabled (comment: false); skipping.');
  } else if (target === null) {
    core.info('No pull_request in the event payload; skipping comment.');
  } else {
    await poster({
      owner: target.owner,
      repo: target.repo,
      pr: target.pr,
      body: out.body,
      token: process.env['GITHUB_TOKEN'] ?? '',
    });
  }

  // Evaluated regardless of the comment gate: with `comment: false` the
  // exit code is the only failure signal.
  if (inputs.failOnUntested && out.untested.length > 0) {
    const n = out.untested.length;
    core.setFailed(
      `Blast Radius: ${n} reached ${n === 1 ? 'module has' : 'modules have'} no test coverage (fail-on-untested).`,
    );
  }
}

if (typeof require !== 'undefined' && require.main === module) {
  void main();
}
