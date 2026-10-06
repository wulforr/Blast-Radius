import type { GraphStats, ReachedFile, UnresolvedRef } from './analyzer.js';
import type { Classification } from './classify.js';

export const MARKER = '<!-- blast-radius -->';

// A section list never exceeds this many lines; the overflow becomes one
// "…and N more" line. The comment must never be the longest thing on the PR.
const MAX_LIST_LINES = 20;

export interface CommentData {
  changedCount: number;
  reached: ReachedFile[];
  classification: Classification;
  untested: string[];
  stats: GraphStats;
  unresolved: UnresolvedRef[];
  maxDepth: number;
}

function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function cappedList(items: string[]): string {
  const shown = items.slice(0, MAX_LIST_LINES).map((item) => `   ${escapeHtml(item)}`);
  if (items.length > shown.length) {
    shown.push(`   …and ${items.length - shown.length} more`);
  }
  return shown.join('\n');
}

export function renderComment(data: CommentData): string {
  const { classification, reached, stats } = data;
  const lines: string[] = [
    MARKER,
    '### 🔴 Blast radius',
    '',
    `**${plural(data.changedCount, 'file changed', 'files changed')}** → ` +
      `**${plural(reached.length, 'module', 'modules')}**, ` +
      `**${plural(classification.routes.length, 'route', 'routes')}**, ` +
      `**${plural(classification.tests.length, 'test', 'tests')}** affected.`,
  ];

  if (data.untested.length > 0) {
    const verb = data.untested.length === 1 ? 'has' : 'have';
    lines.push(
      '',
      `⚠️ **${plural(data.untested.length, 'affected module', 'affected modules')} ${verb} no test coverage in their reach.**`,
      cappedList([...data.untested].sort()),
    );
  }

  lines.push(
    '',
    `<details><summary>Affected routes (${classification.routes.length})</summary>`,
    '',
    classification.routes.length > 0 ? cappedList(classification.routes) : 'None detected.',
    '',
    '</details>',
  );

  const inScope = reached.filter((r) => r.depth <= data.maxDepth);
  const treeLines = inScope.slice(0, MAX_LIST_LINES).map((r) => `   ${escapeHtml(r.path)} (depth ${r.depth})`);
  const hidden = reached.length - Math.min(inScope.length, MAX_LIST_LINES);
  const deeper = reached.filter((r) => r.depth > data.maxDepth).length;
  if (deeper > 0) {
    treeLines.push(`   …and ${hidden} more (${deeper} below depth ${data.maxDepth})`);
  } else if (hidden > 0) {
    treeLines.push(`   …and ${hidden} more`);
  }
  lines.push(
    '',
    `<details><summary>Reach tree, depth ≤ ${data.maxDepth}</summary>`,
    '',
    treeLines.length > 0 ? treeLines.join('\n') : 'None.',
    '',
    '</details>',
  );

  const health = `Graph health: ${stats.edges} imports, ${stats.unresolved} unresolved, ${stats.dynamicGaps} dynamic`;
  const gaps: string[] = [];
  if (data.unresolved.length > 0) {
    gaps.push(
      'Unresolved imports:',
      // Escaped inside cappedList, so hostile specifiers render as text.
      cappedList(data.unresolved.map((u) => `${u.file} → ${u.specifier}`)),
    );
  }
  if (stats.parseFailures > 0) {
    gaps.push(`   ${stats.parseFailures} files could not be read or parsed.`);
  }
  lines.push(
    '',
    `<details><summary>${health}</summary>`,
    '',
    gaps.length > 0 ? gaps.join('\n') : health + '.',
    '',
    '</details>',
  );

  return lines.join('\n');
}

import * as github from '@actions/github';

export interface IssueComment {
  id: number;
  body?: string;
}

export interface CommentsClient {
  list(params: { owner: string; repo: string; issue_number: number }): Promise<IssueComment[]>;
  create(params: { owner: string; repo: string; issue_number: number; body: string }): Promise<unknown>;
  update(params: {
    owner: string;
    repo: string;
    comment_id: number;
    body: string;
  }): Promise<unknown>;
}

export async function upsertComment(
  client: CommentsClient,
  owner: string,
  repo: string,
  pr: number,
  body: string,
): Promise<'created' | 'updated'> {
  const comments = await client.list({ owner, repo, issue_number: pr });
  const existing = comments.find((c) => (c.body ?? '').includes(MARKER));
  if (existing === undefined) {
    await client.create({ owner, repo, issue_number: pr, body });
    return 'created';
  }
  await client.update({ owner, repo, comment_id: existing.id, body });
  return 'updated';
}

export async function postComment(args: {
  owner: string;
  repo: string;
  pr: number;
  body: string;
  token: string;
}): Promise<'created' | 'updated'> {
  if (args.token === '') {
    throw new Error('GITHUB_TOKEN is required to post the PR comment.');
  }
  const octokit = github.getOctokit(args.token);
  return upsertComment(
    {
      list: (params) =>
        octokit.paginate(octokit.rest.issues.listComments, {
          owner: params.owner,
          repo: params.repo,
          issue_number: params.issue_number,
          per_page: 100,
        }),
      create: (params) => octokit.rest.issues.createComment(params),
      update: (params) => octokit.rest.issues.updateComment(params),
    },
    args.owner,
    args.repo,
    args.pr,
    args.body,
  );
}
