import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { EXAMPLES } from './examples';
import { stableExampleScene } from './exampleFixtures';

// Vitest runs from apps/web; fixtures live at the repository root.
const examplesDir = resolve(process.cwd(), '../../examples');

describe('example fixtures', () => {
  it.each(EXAMPLES.map((e) => [e.id, e] as const))('examples/%s.aeroflow.json is up to date', (id, ex) => {
    const file = resolve(examplesDir, `${id}.aeroflow.json`);
    expect(existsSync(file), `missing ${file} — run: npx vite-node scripts/export-examples.ts`).toBe(true);
    expect(JSON.parse(readFileSync(file, 'utf8'))).toEqual(JSON.parse(JSON.stringify(stableExampleScene(ex))));
  });
});
