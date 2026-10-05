import * as core from '@actions/core';
import * as path from 'node:path';
import { writeFileSync } from 'node:fs';
import { analyzeTree, type AnalyzeResult } from './analyzer.js';
import { collectFiles } from './collect.js';

export interface RunOptions {
  workspace: string;
  roots: string[];
  changed: string[];
  analyzerDir: string;
}

export interface RunResult {
  result: AnalyzeResult;
  resultPath: string;
}

export async function run(options: RunOptions): Promise<RunResult> {
  const files = collectFiles(options.workspace, options.roots);
  const result = analyzeTree(options.analyzerDir, files, options.changed);
  const resultPath = path.join(options.workspace, 'blast-radius.json');
  writeFileSync(resultPath, JSON.stringify(result, null, 2) + '\n');
  core.setOutput('result-path', resultPath);
  core.info(
    `Blast radius: ${result.reached.length} modules reached, ` +
      `${result.stats.unresolved} unresolved, ${result.stats.dynamicGaps} dynamic gaps.`,
  );
  return { result, resultPath };
}
