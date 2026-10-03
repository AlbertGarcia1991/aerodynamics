/**
 * Architectural boundaries (PRD §66–§68, §86), enforced as a test so they fail
 * CI rather than relying on review. Each rule names the PRD requirement it
 * protects.
 */
import { describe, expect, it } from 'vitest';

const sources = import.meta.glob<string>('/src/**/*.{ts,tsx}', { query: '?raw', import: 'default', eager: true });

/** Value imports (type-only imports are erased and do not create a runtime dependency). */
function imports(src: string): string[] {
  const out: string[] = [];
  const re = /^\s*import\s+(?!type\b)(?:[^'"]*?from\s+)?['"]([^'"]+)['"]/gm;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src))) out.push(m[1]);
  // Dynamic imports count too.
  const dyn = /import\(\s*['"]([^'"]+)['"]\s*\)/g;
  while ((m = dyn.exec(src))) out.push(m[1]);
  return out;
}

const files = Object.entries(sources).filter(([path]) => !path.includes('/wasm/pkg/') && !path.endsWith('.test.ts') && !path.endsWith('.test.tsx'));

function violations(inDir: string, forbidden: RegExp, allowFile?: (p: string) => boolean): string[] {
  const bad: string[] = [];
  for (const [path, src] of files) {
    if (!path.startsWith(`/src/${inDir}`)) continue;
    if (allowFile?.(path)) continue;
    for (const spec of imports(src)) if (forbidden.test(spec)) bad.push(`${path} → ${spec}`);
  }
  return bad;
}

describe('module boundaries', () => {
  it('finds the source tree', () => {
    expect(files.length).toBeGreaterThan(30);
  });

  it('positive control: the import parser sees the dependencies that do exist', () => {
    // If the regex silently matched nothing, every rule below would pass vacuously.
    const worker = sources['/src/solver/solver.worker.ts'];
    expect(imports(worker).some((s) => /wasm\/pkg/.test(s))).toBe(true);
    const app = sources['/src/App.tsx'];
    expect(imports(app)).toContain('@/solver/client');
    // Type-only imports are ignored, multi-line named imports are not.
    expect(imports("import type { X } from '@/state/a';")).toEqual([]);
    expect(imports("import {\n  a,\n  b,\n} from '@/state/b';")).toEqual(['@/state/b']);
  });

  it('only the solver worker loads the WASM module (WASM-002: UI ≠ WASM)', () => {
    const bad: string[] = [];
    for (const [path, src] of files) {
      if (path === '/src/solver/solver.worker.ts') continue;
      for (const spec of imports(src)) if (/wasm\/pkg/.test(spec)) bad.push(`${path} → ${spec}`);
    }
    expect(bad).toEqual([]);
  });

  it('the domain model depends on nothing but itself (shared contracts, PRD §68)', () => {
    expect(violations('domain/', /^@\/(?!domain\/)|^\.\.\//)).toEqual([]);
  });

  it('exporters depend only on the domain model', () => {
    expect(violations('export/', /^@\/(?!domain\/|export\/)/)).toEqual([]);
  });

  it('renderers do not reach into React components or the solver (solver ≠ renderer, PRD §86.1)', () => {
    expect(violations('render/', /^@\/(components|solver)\/|^react/)).toEqual([]);
  });

  it('the solver transport does not depend on UI components', () => {
    expect(violations('solver/', /^@\/components\/|^react/)).toEqual([]);
  });

  it('state stores do not import components', () => {
    expect(violations('state/', /^@\/components\//)).toEqual([]);
  });
});
