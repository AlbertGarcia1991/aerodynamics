/** Deterministic serialisation of bundled examples, for the checked-in fixtures. */
import type { Example } from './examples';
import type { Scene } from './types';

export function stableExampleScene(ex: Example): Scene {
  const scene = ex.build();
  scene.elements = scene.elements.map((e, i) => ({ ...e, id: `${ex.id}-element-${i + 1}` }));
  scene.bodies = scene.bodies.map((b, i) => ({ ...b, id: `${ex.id}-body-${i + 1}` }));
  return scene;
}
