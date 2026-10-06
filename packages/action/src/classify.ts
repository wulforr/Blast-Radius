import type { ReachedFile } from './analyzer.js';

export interface Classification {
  routes: string[];
  tests: string[];
  publicApi: string[];
}

const TEST_PATTERNS = [/\.test\.[^/]+$/, /\.spec\.[^/]+$/, /(^|\/)__tests__\//];

// App-router pages, pages directories, and explicit route folders.
const ROUTE_PATHS = [/^app\/.+\/page\.tsx$/, /^pages\//, /^src\/routes\//];

// A handler call (`app.get(`, `router.post(` …) means a route only when the
// file also wires a framework — otherwise every `map.get(` in the world
// would match.
const HANDLER_CALL = /\.(get|post|put|delete|patch|options|head|all|use)\s*\(/;
const FRAMEWORK_IMPORT =
  /(from\s+['"])(express|hono)(['"])|require\(\s*['"](express|hono)(\/[^'"]*)?['"]\s*\)/;

export function isTest(path: string): boolean {
  return TEST_PATTERNS.some((re) => re.test(path));
}

export function isRoute(path: string, contents?: string): boolean {
  if (ROUTE_PATHS.some((re) => re.test(path))) return true;
  if (contents === undefined) return false;
  return FRAMEWORK_IMPORT.test(contents) && HANDLER_CALL.test(contents);
}

const SOURCE_EXTENSIONS = ['ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs'];

function stripJsExtension(target: string): string {
  // `./index.js` in `exports` usually means `./index.ts` on disk.
  const dot = target.lastIndexOf('.');
  if (dot < 0) return target;
  const ext = target.slice(dot + 1);
  if (ext === 'ts' || ext === 'tsx') return target;
  if (SOURCE_EXTENSIONS.includes(ext)) return target.slice(0, dot);
  return target;
}

function exportsTargets(manifest: string): string[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(manifest) as unknown;
  } catch {
    return [];
  }
  if (typeof parsed !== 'object' || parsed === null) return [];
  const exportsField = (parsed as Record<string, unknown>)['exports'];
  const out: string[] = [];
  const visit = (node: unknown): void => {
    if (typeof node === 'string') {
      out.push(node);
    } else if (Array.isArray(node)) {
      for (const item of node) visit(item);
    } else if (typeof node === 'object' && node !== null) {
      for (const value of Object.values(node)) visit(value);
    }
  };
  visit(exportsField);
  return out;
}

export function isPublicApi(path: string, files: Record<string, string>): boolean {
  // A root barrel file: `index.ts` with no directory.
  if (/^index\.(ts|tsx|js|jsx|mjs|cjs)$/.test(path)) return true;
  const manifest = files['package.json'];
  if (manifest === undefined) return false;
  return exportsTargets(manifest).some((target) => {
    const bare = target.startsWith('./') ? target.slice(2) : target;
    return path === bare || path === `${stripJsExtension(bare)}.ts` || path === `${stripJsExtension(bare)}.tsx`;
  });
}

export function classify(
  reached: ReachedFile[],
  files: Record<string, string>,
): Classification {
  const routes: string[] = [];
  const tests: string[] = [];
  const publicApi: string[] = [];
  for (const { path } of reached) {
    if (isRoute(path, files[path])) routes.push(path);
    if (isTest(path)) tests.push(path);
    if (isPublicApi(path, files)) publicApi.push(path);
  }
  routes.sort();
  tests.sort();
  publicApi.sort();
  return { routes, tests, publicApi };
}
