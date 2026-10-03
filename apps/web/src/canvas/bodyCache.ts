/**
 * World-space body contours for drawing and hit-testing.
 *
 * Contours come from the solver (which owns panelisation and the trailing-edge
 * rotation). While a drag is in flight the solution lags the scene by a frame
 * or two, so the cached contour is re-posed with the *delta* between the
 * transform it was solved at and the body's current transform. The drawn
 * outline therefore follows the cursor exactly even before the solve lands.
 */
import type { SceneBody, Solution, Vec2 } from '@/domain/types';

interface Cached {
  polygon: Vec2[];
  position: Vec2;
  rotation: number;
  scale: number;
}

const cache = new Map<string, Cached>();

export function syncBodyCache(solution: Solution | null, bodies: SceneBody[]): void {
  if (!solution) return;
  for (const r of solution.bodies) {
    const b = bodies.find((x) => x.id === r.id);
    if (!b) continue;
    cache.set(r.id, { polygon: r.polygon, position: b.position, rotation: b.rotation, scale: b.scale });
  }
  for (const id of Array.from(cache.keys())) {
    if (!bodies.some((b) => b.id === id)) cache.delete(id);
  }
}

/** The body's contour at its *current* transform, or `null` before the first solve. */
export function currentPolygon(body: SceneBody): Vec2[] | null {
  const c = cache.get(body.id);
  if (!c) return null;
  const dr = body.rotation - c.rotation;
  const ds = body.scale / (c.scale || 1);
  if (dr === 0 && ds === 1 && body.position.x === c.position.x && body.position.y === c.position.y) return c.polygon;
  const cos = Math.cos(dr);
  const sin = Math.sin(dr);
  return c.polygon.map((p) => {
    const x = (p.x - c.position.x) * ds;
    const y = (p.y - c.position.y) * ds;
    return { x: x * cos - y * sin + body.position.x, y: x * sin + y * cos + body.position.y };
  });
}

export function pointInPolygon(p: Vec2, poly: Vec2[]): boolean {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const a = poly[i];
    const b = poly[j];
    if (a.y > p.y !== b.y > p.y && p.x < a.x + ((p.y - a.y) / (b.y - a.y)) * (b.x - a.x)) inside = !inside;
  }
  return inside;
}

/** Largest distance from the body origin to its contour — a handle radius. */
export function bodyRadius(body: SceneBody): number {
  const poly = currentPolygon(body);
  if (!poly || poly.length === 0) return 0.5 * body.scale;
  let r = 0;
  for (const p of poly) r = Math.max(r, Math.hypot(p.x - body.position.x, p.y - body.position.y));
  return r;
}
