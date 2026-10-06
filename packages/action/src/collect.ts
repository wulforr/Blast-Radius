import * as core from '@actions/core';
import * as fs from 'node:fs';
import * as path from 'node:path';

// Mirrors packages/analyzer/src/discover.rs. The wasm side re-applies the
// same filter over the shipped keys, so this is a transport optimisation
// (binaries and build output never cross the boundary), not a second source
// of truth — the parity test below pins the behaviour, not the literals.
const SOURCE_EXTENSIONS = new Set(['ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs']);
const IGNORED_DIRS = new Set([
  'node_modules',
  'dist',
  'build',
  'out',
  'coverage',
  '.next',
  '.nuxt',
  '.output',
  '.git',
  'target',
]);

function isAnalysable(name: string): boolean {
  if (name.endsWith('.d.ts') || name.endsWith('.d.mts') || name.endsWith('.d.cts')) {
    return false;
  }
  const dot = name.lastIndexOf('.');
  if (dot < 0) return false;
  return SOURCE_EXTENSIONS.has(name.slice(dot + 1));
}

/**
 * Read every analysable source file under `roots` into memory, keyed by
 * workspace-relative forward-slash path. Roots escaping the workspace are
 * skipped with a warning — the Action must never read outside the checkout.
 */
export function collectFiles(workspace: string, roots: string[]): Record<string, string> {
  const files: Record<string, string> = {};
  // Canonicalise once: on macOS /var and /tmp are symlinks, so string
  // comparison against an unresolved base would reject everything.
  let base: string;
  try {
    base = fs.realpathSync(path.resolve(workspace));
  } catch {
    return {};
  }
  const seen = new Set<string>();

  for (const root of roots) {
    const abs = path.resolve(base, root);
    if (abs !== base && !abs.startsWith(base + path.sep)) {
      core.warning(`Ignoring paths entry '${root}': outside the workspace.`);
      continue;
    }
    let canonical: string;
    try {
      canonical = fs.realpathSync(abs);
    } catch {
      core.warning(`Ignoring paths entry '${root}': not a readable directory.`);
      continue;
    }
    // realpathSync succeeds on files too, so directory-ness is checked
    // explicitly before the containment check below.
    let stat: fs.Stats;
    try {
      stat = fs.statSync(canonical);
    } catch {
      core.warning(`Ignoring paths entry '${root}': not a readable directory.`);
      continue;
    }
    if (!stat.isDirectory()) {
      core.warning(`Ignoring paths entry '${root}': not a readable directory.`);
      continue;
    }
    // Resolve symlinks before the containment check: a committed
    // `link -> /etc` otherwise passes the string check above and gets walked.
    // (M5 has no exfiltration path — contents only land in the local artifact —
    // but a scope escape is still a wrong analysis.)
    if (canonical !== base && !canonical.startsWith(base + path.sep)) {
      core.warning(`Ignoring paths entry '${root}': resolves outside the workspace.`);
      continue;
    }
    if (seen.has(canonical)) continue;
    seen.add(canonical);
    walk(canonical, base, files);
  }
  return files;
}

function walk(dir: string, base: string, files: Record<string, string>): void {
  let entries: fs.Dirent[];
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const entry of entries) {
    const abs = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (!IGNORED_DIRS.has(entry.name)) walk(abs, base, files);
    } else if (entry.isFile() && isAnalysable(entry.name)) {
      const key = path.relative(base, abs).split(path.sep).join('/');
      try {
        files[key] = fs.readFileSync(abs, 'utf8');
      } catch {
        core.warning(`Skipping unreadable file '${key}'.`);
      }
    }
  }
}
