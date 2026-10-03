/**
 * Writes the bundled examples to `examples/*.aeroflow.json`.
 *
 *   cd apps/web && npx vite-node scripts/export-examples.ts
 *
 * Object ids are random at runtime; here they are replaced by stable ones so
 * the fixtures diff cleanly. The Rust test `crates/solver/tests/examples.rs`
 * loads and solves every file — the cross-language check of the shared scene
 * contract (PRD §68).
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { EXAMPLES } from '@/domain/examples';
import { stableExampleScene } from '@/domain/exampleFixtures';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const outDir = resolve(root, 'examples');
mkdirSync(outDir, { recursive: true });
for (const ex of EXAMPLES) {
  const file = resolve(outDir, `${ex.id}.aeroflow.json`);
  writeFileSync(file, JSON.stringify(stableExampleScene(ex), null, 2) + '\n');
  console.log(`wrote ${file}`);
}
