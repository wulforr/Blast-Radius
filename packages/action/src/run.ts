import * as core from '@actions/core';
import * as path from 'node:path';
import { writeFileSync } from 'node:fs';
import { analyzeTree, type AnalyzeResult } from './analyzer.js';
import { classify, type Classification } from './classify.js';
import { collectFiles } from './collect.js';
import { renderComment } from './comment.js';
import { findUntested } from './untested.js';

export interface RunOptions {
  workspace: string;
  roots: string[];
  changed: string[];
  analyzerDir: string;
  maxDepth: number;
}

export interface RunResult {
  result: AnalyzeResult;
  resultPath: string;
  classification: Classification;
  body: string;
  untested: string[];
}

export async function run(options: RunOptions): Promise<RunResult> {
  const files = collectFiles(options.workspace, options.roots);
  const result = analyzeTree(options.analyzerDir, files, options.changed);
  const classification = classify(result.reached, files);
  const untested = findUntested(result.reached, result.edges);
  const body = renderComment({
    changedCount: options.changed.length,
    reached: result.reached,
    classification,
    untested,
    stats: result.stats,
    unresolved: result.unresolved,
    maxDepth: options.maxDepth,
  });
  const resultPath = path.join(options.workspace, 'blast-radius.json');
  writeFileSync(resultPath, JSON.stringify(result, null, 2) + '\n');
  core.setOutput('result-path', resultPath);
  core.info(
    `Blast radius: ${result.reached.length} modules reached, ` +
      `${result.stats.unresolved} unresolved, ${result.stats.dynamicGaps} dynamic gaps.`,
  );
  return { result, resultPath, classification, body, untested };
}
