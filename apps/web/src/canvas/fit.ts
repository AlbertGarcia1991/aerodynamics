/** Fit the view to everything in the scene (PRD §34 "fit-to-view"). */
import type { Bounds } from '@/domain/types';
import { elementPosition } from '@/domain/scene';
import { useSimulationStore } from '@/state/simulationStore';
import { useSolverStore } from '@/state/solverStore';
import { useViewportStore } from '@/state/viewportStore';

export function sceneBounds(): Bounds | null {
  const scene = useSimulationStore.getState().scene;
  const solution = useSolverStore.getState().solution;
  let min = { x: Infinity, y: Infinity };
  let max = { x: -Infinity, y: -Infinity };
  let any = false;
  const take = (x: number, y: number) => {
    any = true;
    min = { x: Math.min(min.x, x), y: Math.min(min.y, y) };
    max = { x: Math.max(max.x, x), y: Math.max(max.y, y) };
  };
  for (const e of scene.elements) {
    const p = elementPosition(e.element);
    if (p) {
      take(p.x - 0.5, p.y - 0.5);
      take(p.x + 0.5, p.y + 0.5);
    }
  }
  for (const b of scene.bodies) {
    const poly = solution?.bodies.find((r) => r.id === b.id)?.polygon;
    if (poly && poly.length) poly.forEach((p) => take(p.x, p.y));
    else take(b.position.x, b.position.y);
  }
  if (!any) return null;
  const w = Math.max(max.x - min.x, 0.5);
  const h = Math.max(max.y - min.y, 0.5);
  return {
    min: { x: min.x - 0.5 * w, y: min.y - 0.5 * h },
    max: { x: max.x + 0.5 * w, y: max.y + 0.5 * h },
  };
}

export function fitToScene(): void {
  const b = sceneBounds() ?? { min: { x: -3, y: -2 }, max: { x: 3, y: 2 } };
  useViewportStore.getState().fit(b, 0.06);
}
