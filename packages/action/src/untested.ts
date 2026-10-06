import type { ReachedFile } from './analyzer.js';
import { isTest } from './classify.js';

/**
 * Reached modules with no test coverage: a module counts as covered when a
 * test file appears anywhere in its own reverse-reachable set (modules that
 * transitively import it — i.e. anything that would exercise it). Spec §4.
 *
 * Soundness note: any test covering a reached module is itself reached (a
 * test importing M, where M transitively imports changed code, transitively
 * imports that changed code too), so walking the full edge set is exact, not
 * an approximation. The reverse map is built once; each walk is iterative
 * with a visited set, so import cycles terminate.
 */
export function findUntested(
  reached: ReachedFile[],
  edges: Array<[string, string]>,
): string[] {
  const importers = new Map<string, Set<string>>();
  for (const [from, to] of edges) {
    let set = importers.get(to);
    if (set === undefined) {
      set = new Set<string>();
      importers.set(to, set);
    }
    set.add(from);
  }

  const untested: string[] = [];
  for (const { path } of reached) {
    if (!hasCoveringTest(path, importers)) {
      untested.push(path);
    }
  }
  return untested.sort();
}

function hasCoveringTest(start: string, importers: Map<string, Set<string>>): boolean {
  const seen = new Set<string>([start]);
  const queue: string[] = [start];
  while (queue.length > 0) {
    const current = queue.pop();
    if (current === undefined) continue;
    if (isTest(current)) return true;
    const next = importers.get(current) ?? [];
    for (const importer of next) {
      if (!seen.has(importer)) {
        seen.add(importer);
        queue.push(importer);
      }
    }
  }
  return false;
}
