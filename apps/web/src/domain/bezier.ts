/**
 * Cubic-Bézier geometry: evaluation, subdivision, editing operations, validation,
 * point fitting, templates, and the Bézier → solver mapping (PRD2 §8–§28, §110).
 *
 * Everything is pure and immutable: each edit returns a new geometry, which is
 * what makes undo/redo a snapshot swap and the pipeline deterministic.
 *
 * Coordinates are body-local (the body's position/rotation/scale are applied
 * afterwards, §55), +x right, +y up.
 */
import type { BezierGeometry, BezierNode, BodyGeometry, NodeType, Scene, SceneBody, Vec2 } from './types';
import { SOLVER_FORMAT_VERSION } from './types';
import { newId, vadd, vlen, vrotate, vscale, vsub } from './scene';

export type Cubic = [Vec2, Vec2, Vec2, Vec2];
export type HandleSide = 'in' | 'out';

const ZERO: Vec2 = { x: 0, y: 0 };
const EPS = 1e-12;
/** Circle-arc handle length for a quarter turn: 4/3·tan(π/8). */
export const KAPPA = 0.5522847498307936;

const dot = (a: Vec2, b: Vec2) => a.x * b.x + a.y * b.y;
const cross = (a: Vec2, b: Vec2) => a.x * b.y - a.y * b.x;
const neg = (a: Vec2): Vec2 => ({ x: -a.x, y: -a.y });
const lerp = (a: Vec2, b: Vec2, t: number): Vec2 => ({ x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t });
const unit = (a: Vec2): Vec2 => {
  const l = vlen(a);
  return l > EPS ? vscale(a, 1 / l) : ZERO;
};
const finite = (a: Vec2) => Number.isFinite(a.x) && Number.isFinite(a.y);

// ───────────────────────────── cubic maths (BEZ-002..004) ─────────────────────────────

export function cubicAt(c: Cubic, t: number): Vec2 {
  const u = 1 - t;
  const b0 = u * u * u;
  const b1 = 3 * u * u * t;
  const b2 = 3 * u * t * t;
  const b3 = t * t * t;
  return {
    x: b0 * c[0].x + b1 * c[1].x + b2 * c[2].x + b3 * c[3].x,
    y: b0 * c[0].y + b1 * c[1].y + b2 * c[2].y + b3 * c[3].y,
  };
}

/** First derivative dB/dt. */
export function cubicDerivative(c: Cubic, t: number): Vec2 {
  const u = 1 - t;
  const a = vscale(vsub(c[1], c[0]), 3 * u * u);
  const b = vscale(vsub(c[2], c[1]), 6 * u * t);
  const d = vscale(vsub(c[3], c[2]), 3 * t * t);
  return vadd(vadd(a, b), d);
}

/** Exact de Casteljau subdivision: two cubics that together trace the original curve. */
export function splitCubic(c: Cubic, t: number): [Cubic, Cubic] {
  const p01 = lerp(c[0], c[1], t);
  const p12 = lerp(c[1], c[2], t);
  const p23 = lerp(c[2], c[3], t);
  const p012 = lerp(p01, p12, t);
  const p123 = lerp(p12, p23, t);
  const mid = lerp(p012, p123, t);
  return [
    [c[0], p01, p012, mid],
    [mid, p123, p23, c[3]],
  ];
}

// ───────────────────────────── geometry access ─────────────────────────────

export function makeNode(position: Vec2, patch: Partial<Omit<BezierNode, 'position'>> = {}): BezierNode {
  return { id: newId('node'), position, inHandle: ZERO, outHandle: ZERO, nodeType: 'corner', ...patch };
}

export const emptyBezier = (): BezierGeometry => ({ kind: 'bezier', closed: false, nodes: [] });

export function segmentCount(g: BezierGeometry): number {
  const n = g.nodes.length;
  if (n < 2) return 0;
  return g.closed ? n : n - 1;
}

export function segmentCubic(g: BezierGeometry, i: number): Cubic {
  const a = g.nodes[i];
  const b = g.nodes[(i + 1) % g.nodes.length];
  return [a.position, vadd(a.position, a.outHandle), vadd(b.position, b.inHandle), b.position];
}

/**
 * Sample the curve into a polyline. A closed path does not repeat its first
 * point. The per-segment count follows a fixed total budget, so the result is a
 * deterministic function of the geometry (PAN-002).
 */
export function sampleBezier(g: BezierGeometry, budget = 720): Vec2[] {
  const segs = segmentCount(g);
  if (segs === 0) return g.nodes.map((n) => n.position);
  const per = Math.min(96, Math.max(16, Math.round(budget / segs)));
  const out: Vec2[] = [];
  for (let s = 0; s < segs; s++) {
    const c = segmentCubic(g, s);
    for (let k = 0; k < per; k++) out.push(cubicAt(c, k / per));
  }
  if (!g.closed) out.push(g.nodes[g.nodes.length - 1].position);
  return dedupe(out, g.closed);
}

function dedupe(pts: Vec2[], closed: boolean): Vec2[] {
  const out: Vec2[] = [];
  for (const p of pts) {
    const last = out[out.length - 1];
    if (!last || Math.hypot(p.x - last.x, p.y - last.y) > 1e-10) out.push(p);
  }
  while (closed && out.length > 1 && Math.hypot(out[0].x - out[out.length - 1].x, out[0].y - out[out.length - 1].y) <= 1e-10) out.pop();
  return out;
}

export interface Hit {
  segment: number;
  t: number;
  point: Vec2;
  distance: number;
}

/** Closest point on the curve to `p` (coarse scan, then ternary refinement). */
export function nearestOnCurve(g: BezierGeometry, p: Vec2): Hit | null {
  let best: Hit | null = null;
  const steps = 40;
  for (let s = 0; s < segmentCount(g); s++) {
    const c = segmentCubic(g, s);
    const d = (t: number) => Math.hypot(cubicAt(c, t).x - p.x, cubicAt(c, t).y - p.y);
    let bt = 0;
    let bd = Infinity;
    for (let k = 0; k <= steps; k++) {
      const dk = d(k / steps);
      if (dk < bd) {
        bd = dk;
        bt = k / steps;
      }
    }
    let lo = Math.max(0, bt - 1 / steps);
    let hi = Math.min(1, bt + 1 / steps);
    for (let i = 0; i < 50; i++) {
      const m1 = lo + (hi - lo) / 3;
      const m2 = hi - (hi - lo) / 3;
      if (d(m1) < d(m2)) hi = m2;
      else lo = m1;
    }
    const t = (lo + hi) / 2;
    const dist = d(t);
    if (!best || dist < best.distance) best = { segment: s, t, point: cubicAt(c, t), distance: dist };
  }
  return best;
}

// ───────────────────────────── editing (GEO-002..008) ─────────────────────────────

const mapNode = (g: BezierGeometry, id: string, fn: (n: BezierNode) => BezierNode): BezierGeometry => ({
  ...g,
  nodes: g.nodes.map((n) => (n.id === id ? fn(n) : n)),
});

export function moveNodes(g: BezierGeometry, ids: string[], delta: Vec2): BezierGeometry {
  const set = new Set(ids);
  return { ...g, nodes: g.nodes.map((n) => (set.has(n.id) ? { ...n, position: vadd(n.position, delta) } : n)) };
}

export function setNodePosition(g: BezierGeometry, id: string, position: Vec2): BezierGeometry {
  return mapNode(g, id, (n) => ({ ...n, position }));
}

/**
 * Set one handle (relative offset). Smooth nodes keep the other handle
 * collinear and unchanged in length; symmetric nodes mirror it; corner nodes
 * leave it alone (BEZ-005/006).
 */
export function setHandle(n: BezierNode, side: HandleSide, offset: Vec2): BezierNode {
  const next: BezierNode = side === 'in' ? { ...n, inHandle: offset } : { ...n, outHandle: offset };
  const other = side === 'in' ? 'outHandle' : 'inHandle';
  if (n.nodeType === 'symmetric') next[other] = neg(offset);
  else if (n.nodeType === 'smooth' && vlen(offset) > EPS) next[other] = vscale(unit(neg(offset)), vlen(n[other]));
  return next;
}

export function setNodeHandle(g: BezierGeometry, id: string, side: HandleSide, offset: Vec2): BezierGeometry {
  return mapNode(g, id, (n) => setHandle(n, side, offset));
}

/** Catmull-Rom-style handles for node `i` from its neighbours (used when a node becomes smooth). */
function autoHandles(g: BezierGeometry, i: number): { inHandle: Vec2; outHandle: Vec2 } {
  const n = g.nodes.length;
  const cur = g.nodes[i].position;
  const prev = i > 0 || g.closed ? g.nodes[(i - 1 + n) % n].position : null;
  const next = i < n - 1 || g.closed ? g.nodes[(i + 1) % n].position : null;
  if (prev && next) {
    const t = unit(vsub(next, prev));
    return { inHandle: vscale(t, -vlen(vsub(cur, prev)) / 3), outHandle: vscale(t, vlen(vsub(next, cur)) / 3) };
  }
  if (next) return { inHandle: ZERO, outHandle: vscale(vsub(next, cur), 1 / 3) };
  if (prev) return { inHandle: vscale(vsub(prev, cur), 1 / 3), outHandle: ZERO };
  return { inHandle: ZERO, outHandle: ZERO };
}

export function setNodeType(g: BezierGeometry, id: string, type: NodeType): BezierGeometry {
  const i = g.nodes.findIndex((n) => n.id === id);
  if (i < 0) return g;
  const n = g.nodes[i];
  if (type === 'corner') return mapNode(g, id, (x) => ({ ...x, nodeType: type }));
  let { inHandle, outHandle } = n;
  if (vlen(inHandle) < EPS || vlen(outHandle) < EPS) ({ inHandle, outHandle } = autoHandles(g, i));
  else {
    // Make the tangent continuous: keep each handle's length, share one direction.
    const dir = unit(vsub(outHandle, inHandle));
    inHandle = vscale(dir, -vlen(inHandle));
    outHandle = vscale(dir, vlen(outHandle));
  }
  if (type === 'symmetric') {
    const l = (vlen(inHandle) + vlen(outHandle)) / 2;
    const dir = unit(vsub(outHandle, inHandle));
    inHandle = vscale(dir, -l);
    outHandle = vscale(dir, l);
  }
  return mapNode(g, id, (x) => ({ ...x, nodeType: type, inHandle, outHandle }));
}

/**
 * Split segment `segment` at `t` with exact subdivision, so the curve is
 * unchanged (BEZ-004, GEOM-TEST-004). Returns the new node's id.
 */
export function insertNodeAt(g: BezierGeometry, segment: number, t: number): { geometry: BezierGeometry; nodeId: string } {
  const n = g.nodes.length;
  const [a, b] = splitCubic(segmentCubic(g, segment), t);
  const j = (segment + 1) % n;
  const inserted = makeNode(a[3], { inHandle: vsub(a[2], a[3]), outHandle: vsub(b[1], b[0]), nodeType: 'smooth' });
  // A symmetric neighbour whose handle was shortened is only smooth now.
  const retype = (node: BezierNode): NodeType => (node.nodeType === 'symmetric' ? 'smooth' : node.nodeType);
  const nodes = g.nodes.map((node, k) => {
    if (k === segment) return { ...node, outHandle: vsub(a[1], a[0]), nodeType: retype(node) };
    if (k === j) return { ...node, inHandle: vsub(b[2], b[3]), nodeType: retype(node) };
    return node;
  });
  nodes.splice(segment + 1, 0, inserted);
  return { geometry: { ...g, nodes }, nodeId: inserted.id };
}

export const minNodes = (g: BezierGeometry): number => (g.closed ? 3 : 2);

/**
 * Remove nodes. Neighbours keep their own handles, so the two segments around
 * the hole merge into one. Refused (null) when it would leave too few nodes to
 * be a valid shape (§15).
 */
export function deleteNodes(g: BezierGeometry, ids: string[]): BezierGeometry | null {
  const set = new Set(ids);
  const nodes = g.nodes.filter((n) => !set.has(n.id));
  if (nodes.length === g.nodes.length) return g;
  if (nodes.length < minNodes(g) && nodes.length > 0) return null;
  return { ...g, nodes };
}

/** Pen tool: append a node; a drag supplies a symmetric out-handle. */
export function appendNode(g: BezierGeometry, position: Vec2, out: Vec2 = ZERO): { geometry: BezierGeometry; nodeId: string } {
  const has = vlen(out) > EPS;
  const node = makeNode(position, has ? { outHandle: out, inHandle: neg(out), nodeType: 'symmetric' } : {});
  return { geometry: { ...g, nodes: [...g.nodes, node] }, nodeId: node.id };
}

/** Open ↔ closed. Closing onto a coincident last node merges it into the first (§21). */
export function setClosed(g: BezierGeometry, closed: boolean): BezierGeometry {
  if (g.closed === closed) return g;
  if (!closed) return { ...g, closed };
  const n = g.nodes;
  if (n.length > 2 && vlen(vsub(n[0].position, n[n.length - 1].position)) < 1e-9) {
    const last = n[n.length - 1];
    return { ...g, closed, nodes: [{ ...n[0], inHandle: last.inHandle }, ...n.slice(1, -1)] };
  }
  return { ...g, closed };
}

/** Same shape, opposite winding. */
export function reverseGeometry(g: BezierGeometry): BezierGeometry {
  return { ...g, nodes: [...g.nodes].reverse().map((n) => ({ ...n, inHandle: n.outHandle, outHandle: n.inHandle })) };
}

export function boundsOf(pts: Vec2[]): { min: Vec2; max: Vec2 } {
  const min = { x: Infinity, y: Infinity };
  const max = { x: -Infinity, y: -Infinity };
  for (const p of pts) {
    min.x = Math.min(min.x, p.x);
    min.y = Math.min(min.y, p.y);
    max.x = Math.max(max.x, p.x);
    max.y = Math.max(max.y, p.y);
  }
  return { min, max };
}

/** Mirror about the vertical (x → −x) or horizontal axis through the shape's centre. */
export function mirrorGeometry(g: BezierGeometry, axis: 'vertical' | 'horizontal'): BezierGeometry {
  if (g.nodes.length === 0) return g;
  const b = boundsOf(sampleBezier(g, 4000));
  const flipX = axis === 'vertical';
  const f = (p: Vec2, centre: boolean): Vec2 =>
    flipX ? { x: centre ? b.min.x + b.max.x - p.x : -p.x, y: p.y } : { x: p.x, y: centre ? b.min.y + b.max.y - p.y : -p.y };
  // A reflection flips the winding, so reverse to keep the original orientation.
  return reverseGeometry({
    ...g,
    nodes: g.nodes.map((n) => ({ ...n, position: f(n.position, true), inHandle: f(n.inHandle, false), outHandle: f(n.outHandle, false) })),
  });
}

// ───────────────────────────── validation (§26, §49, §50) ─────────────────────────────

export type BezierStatus = 'ok' | 'open' | 'invalid';

export interface BezierValidation {
  status: BezierStatus;
  messages: string[];
  /** Self-intersection points, body-local, for the overlay. */
  intersections: Vec2[];
}

const properCross = (a: Vec2, b: Vec2, c: Vec2, d: Vec2): Vec2 | null => {
  const r = vsub(b, a);
  const s = vsub(d, c);
  const den = cross(r, s);
  if (Math.abs(den) < EPS) return null;
  const t = cross(vsub(c, a), s) / den;
  const u = cross(vsub(c, a), r) / den;
  return t > 1e-9 && t < 1 - 1e-9 && u > 1e-9 && u < 1 - 1e-9 ? vadd(a, vscale(r, t)) : null;
};

export function polygonSelfIntersections(poly: Vec2[], closed: boolean, limit = 8): Vec2[] {
  const out: Vec2[] = [];
  const n = poly.length;
  const edges = closed ? n : n - 1;
  for (let i = 0; i < edges && out.length < limit; i++) {
    for (let j = i + 2; j < edges; j++) {
      if (closed && i === 0 && j === edges - 1) continue; // adjacent through the wrap
      const hit = properCross(poly[i], poly[(i + 1) % n], poly[j], poly[(j + 1) % n]);
      if (hit) {
        out.push(hit);
        if (out.length >= limit) break;
      }
    }
  }
  return out;
}

export const signedArea = (poly: Vec2[]): number => {
  let a = 0;
  for (let i = 0; i < poly.length; i++) a += cross(poly[i], poly[(i + 1) % poly.length]);
  return a / 2;
};

export function validateBezier(g: BezierGeometry): BezierValidation {
  const invalid = (message: string, intersections: Vec2[] = []): BezierValidation => ({ status: 'invalid', messages: [message], intersections });
  if (g.nodes.some((n) => !finite(n.position) || !finite(n.inHandle) || !finite(n.outHandle))) {
    return invalid('Geometry contains non-finite coordinates.');
  }
  if (g.nodes.length < 2) return { status: 'open', messages: ['Add at least 3 nodes, then close the path.'], intersections: [] };
  for (let s = 0; s < segmentCount(g); s++) {
    const c = segmentCubic(g, s);
    if (c.every((p) => vlen(vsub(p, c[0])) < 1e-9)) return invalid(`Segment ${s + 1} has zero length.`);
  }
  if (!g.closed) return { status: 'open', messages: ['Open path: close it to include it in the simulation.'], intersections: [] };
  if (g.nodes.length < 3) return invalid('A closed body needs at least 3 nodes.');
  const poly = sampleBezier(g, 400);
  if (poly.length < 3 || Math.abs(signedArea(poly)) < 1e-12) return invalid('The shape encloses no area.');
  const hits = polygonSelfIntersections(poly, true);
  if (hits.length) return invalid('The shape intersects itself; the panel solver cannot use it.', hits);
  const messages: string[] = [];
  const b = boundsOf(poly);
  const extent = Math.max(b.max.x - b.min.x, b.max.y - b.min.y);
  if (extent < 1e-3 || extent > 1e4) messages.push('Extreme size: the solver is best conditioned for bodies of roughly 0.01–1000 m.');
  return { status: 'ok', messages, intersections: [] };
}

const validationMemo = new WeakMap<BezierGeometry, BezierValidation>();
/** Geometry objects are immutable, so a validation result can be memoised on identity (the canvas asks every frame). */
export function validateCached(g: BezierGeometry): BezierValidation {
  let v = validationMemo.get(g);
  if (!v) {
    v = validateBezier(g);
    validationMemo.set(g, v);
  }
  return v;
}

// ───────────────────────────── transforms ─────────────────────────────

/** Body-local → world (the same convention as the Rust solver: scale, rotate, translate). */
export const bodyToWorld = (b: Pick<SceneBody, 'position' | 'rotation' | 'scale'>, p: Vec2): Vec2 =>
  vadd(vrotate(vscale(p, b.scale), b.rotation), b.position);

export const worldToBody = (b: Pick<SceneBody, 'position' | 'rotation' | 'scale'>, p: Vec2): Vec2 =>
  vscale(vrotate(vsub(p, b.position), -b.rotation), 1 / (b.scale || 1));

export function bezierWorldPolygon(body: SceneBody): Vec2[] | null {
  if (body.geometry.kind !== 'bezier') return null;
  const poly = sampleBezier(body.geometry);
  return poly.length >= 2 ? poly.map((p) => bodyToWorld(body, p)) : null;
}

// ───────────────────────────── Bézier → solver (§23, §28) ─────────────────────────────

/** Panel count used while a drag is in flight (RT-004). */
export const PREVIEW_PANELS = 64;

/**
 * The scene the solver receives: Bézier bodies become sampled `points` bodies
 * (the derived representation); open or invalid ones are left out rather than
 * solved as something the user did not draw. `previewPanels` caps the panel
 * count for the fast interactive pass.
 */
export function toSolverScene(scene: Scene, previewPanels?: number): Scene {
  const bodies: SceneBody[] = [];
  for (const b of scene.bodies) {
    const capped = previewPanels && b.panels.distribution !== 'asImported' ? { ...b.panels, count: Math.min(b.panels.count, previewPanels) } : b.panels;
    if (b.geometry.kind !== 'bezier') {
      bodies.push(capped === b.panels ? b : { ...b, panels: capped });
      continue;
    }
    if (validateCached(b.geometry).status !== 'ok') continue;
    bodies.push({ ...b, panels: capped, geometry: { kind: 'points', points: sampleBezier(b.geometry) } });
  }
  return { ...scene, version: SOLVER_FORMAT_VERSION, bodies };
}

/** Geometry as the worker's generators understand it. */
export function toSolverGeometry(g: BodyGeometry): Exclude<BodyGeometry, BezierGeometry> {
  return g.kind === 'bezier' ? { kind: 'points', points: sampleBezier(g) } : g;
}

// ───────────────────────────── fitting points to Béziers (§48) ─────────────────────────────

export interface FitResult {
  geometry: BezierGeometry;
  /** Largest distance from any source point to the fitted curve. */
  maxError: number;
}

const CORNER_COS = Math.cos((50 * Math.PI) / 180);

function distToPolyline(p: Vec2, poly: Vec2[]): number {
  let best = Infinity;
  for (let i = 0; i + 1 < poly.length; i++) {
    const a = poly[i];
    const ab = vsub(poly[i + 1], a);
    const l2 = dot(ab, ab);
    const t = l2 > 0 ? Math.min(1, Math.max(0, dot(vsub(p, a), ab) / l2)) : 0;
    best = Math.min(best, vlen(vsub(p, vadd(a, vscale(ab, t)))));
  }
  return best;
}

/** Least-squares cubic through `pts` with fixed end tangents (Schneider). `t2` points back into the curve. */
function fitCubic(pts: Vec2[], t1: Vec2, t2: Vec2): Cubic {
  const first = pts[0];
  const last = pts[pts.length - 1];
  const seg = vlen(vsub(last, first));
  const fallback = (): Cubic => [first, vadd(first, vscale(t1, seg / 3)), vadd(last, vscale(t2, seg / 3)), last];
  if (pts.length < 3) return fallback();
  const u: number[] = [0];
  for (let i = 1; i < pts.length; i++) u.push(u[i - 1] + vlen(vsub(pts[i], pts[i - 1])));
  const total = u[u.length - 1] || 1;
  let c00 = 0, c01 = 0, c11 = 0, x0 = 0, x1 = 0;
  pts.forEach((d, i) => {
    const t = u[i] / total;
    const w = 1 - t;
    const b0 = w * w * w, b1 = 3 * t * w * w, b2 = 3 * t * t * w, b3 = t * t * t;
    const a1 = vscale(t1, b1);
    const a2 = vscale(t2, b2);
    c00 += dot(a1, a1);
    c01 += dot(a1, a2);
    c11 += dot(a2, a2);
    const tmp = vsub(d, vadd(vscale(first, b0 + b1), vscale(last, b2 + b3)));
    x0 += dot(a1, tmp);
    x1 += dot(a2, tmp);
  });
  const det = c00 * c11 - c01 * c01;
  if (Math.abs(det) < EPS) return fallback();
  const al1 = (x0 * c11 - x1 * c01) / det;
  const al2 = (c00 * x1 - c01 * x0) / det;
  const eps = 1e-6 * seg;
  if (!(al1 > eps && al2 > eps) || al1 > 2 * total || al2 > 2 * total) return fallback();
  return [first, vadd(first, vscale(t1, al1)), vadd(last, vscale(t2, al2)), last];
}

/**
 * Approximate a closed contour by cubic Béziers within `tolerance` (model
 * units). Sharp corners (turn > 50°) become corner nodes — a trailing edge
 * stays sharp — and extra nodes are added only where the fit exceeds the
 * tolerance, so the node count follows the shape's complexity.
 */
export function fitPoints(points: Vec2[], tolerance: number): FitResult {
  const raw = points.filter(finite);
  const ext = boundsOf(raw);
  const scale = Math.max(ext.max.x - ext.min.x, ext.max.y - ext.min.y, EPS);
  const pts = dedupe(raw, true);
  if (pts.length < 3) throw new Error('At least three distinct points are needed to build a closed shape.');
  const N = pts.length;
  const P = (i: number) => pts[((i % N) + N) % N];
  const tol = Math.max(tolerance, 1e-9 * scale);

  const corner = new Set<number>();
  for (let i = 0; i < N; i++) {
    const d1 = unit(vsub(P(i), P(i - 1)));
    const d2 = unit(vsub(P(i + 1), P(i)));
    if (dot(d1, d2) < CORNER_COS) corner.add(i);
  }
  const fixed = new Set(corner);
  if (fixed.size < 2) {
    const a = fixed.size ? [...fixed][0] : 0;
    let far = a === 0 ? 1 : 0;
    for (let i = 0; i < N; i++) if (vlen(vsub(P(i), P(a))) > vlen(vsub(P(far), P(a)))) far = i;
    fixed.add(a);
    fixed.add(far);
  }
  const starts = [...fixed].sort((p, q) => p - q);

  const smoothT = (i: number) => unit(vsub(P(i + 1), P(i - 1)));
  interface Span { a: number; cubic: Cubic; err: number }
  const spans: Span[] = [];
  const fitSpan = (a: number, b: number) => {
    const tA = corner.has(((a % N) + N) % N) ? unit(vsub(P(a + 1), P(a))) : smoothT(a);
    const tB = corner.has(((b % N) + N) % N) ? unit(vsub(P(b - 1), P(b))) : neg(smoothT(b));
    const span: Vec2[] = [];
    for (let i = a; i <= b; i++) span.push(P(i));
    const cubic = fitCubic(span, tA, tB);
    const poly: Vec2[] = [];
    for (let k = 0; k <= 24; k++) poly.push(cubicAt(cubic, k / 24));
    let err = 0;
    let worst = a;
    for (let i = a + 1; i < b; i++) {
      const d = distToPolyline(P(i), poly);
      if (d > err) {
        err = d;
        worst = i;
      }
    }
    if (err > tol && b - a > 1) {
      fitSpan(a, worst);
      fitSpan(worst, b);
    } else spans.push({ a, cubic, err });
  };
  starts.forEach((s, k) => fitSpan(s, k + 1 < starts.length ? starts[k + 1] : starts[0] + N));

  const nodes: BezierNode[] = spans.map((sp, k) => {
    const prev = spans[(k - 1 + spans.length) % spans.length];
    const isCorner = corner.has(((sp.a % N) + N) % N);
    return makeNode(sp.cubic[0], {
      outHandle: vsub(sp.cubic[1], sp.cubic[0]),
      inHandle: vsub(prev.cubic[2], prev.cubic[3]),
      nodeType: isCorner ? 'corner' : 'smooth',
    });
  });
  const maxError = spans.reduce((m, s) => Math.max(m, s.err), 0);
  return { geometry: { kind: 'bezier', closed: true, nodes, fitError: maxError }, maxError };
}

// ───────────────────────────── templates (§16.2, §84) ─────────────────────────────

export type BezierTemplate = 'circle' | 'ellipse' | 'naca0012' | 'naca2412' | 'flatPlate' | 'roundedPlate' | 'roundedRect' | 'blank';

export const TEMPLATE_LABELS: Record<BezierTemplate, string> = {
  naca2412: 'Airfoil (NACA 2412)',
  naca0012: 'Symmetric airfoil (NACA 0012)',
  circle: 'Circle',
  ellipse: 'Ellipse',
  flatPlate: 'Flat plate',
  roundedPlate: 'Rounded plate',
  roundedRect: 'Rounded rectangle',
  blank: 'Blank (draw with the pen)',
};

function ellipseNodes(a: number, b: number): BezierNode[] {
  const k = KAPPA;
  const at = (x: number, y: number, tx: number, ty: number) =>
    makeNode({ x, y }, { nodeType: 'smooth', outHandle: { x: tx, y: ty }, inHandle: { x: -tx, y: -ty } });
  return [at(a, 0, 0, k * b), at(0, b, -k * a, 0), at(-a, 0, 0, -k * b), at(0, -b, k * a, 0)];
}

function roundedRect(w: number, h: number, r: number): BezierNode[] {
  if (r <= 0) {
    return [{ x: w / 2, y: -h / 2 }, { x: w / 2, y: h / 2 }, { x: -w / 2, y: h / 2 }, { x: -w / 2, y: -h / 2 }].map((p) => makeNode(p));
  }
  const corners = [
    { c: { x: w / 2 - r, y: -h / 2 + r }, a0: -Math.PI / 2 },
    { c: { x: w / 2 - r, y: h / 2 - r }, a0: 0 },
    { c: { x: -w / 2 + r, y: h / 2 - r }, a0: Math.PI / 2 },
    { c: { x: -w / 2 + r, y: -h / 2 + r }, a0: Math.PI },
  ];
  const nodes: BezierNode[] = [];
  for (const { c, a0 } of corners) {
    const a1 = a0 + Math.PI / 2;
    const tangent = (a: number): Vec2 => ({ x: -Math.sin(a), y: Math.cos(a) });
    nodes.push(makeNode({ x: c.x + r * Math.cos(a0), y: c.y + r * Math.sin(a0) }, { outHandle: vscale(tangent(a0), KAPPA * r) }));
    nodes.push(makeNode({ x: c.x + r * Math.cos(a1), y: c.y + r * Math.sin(a1) }, { inHandle: vscale(tangent(a1), -KAPPA * r) }));
  }
  // A fully rounded end (r = h/2) leaves a zero-length straight edge: merge the coincident pair.
  const merged: BezierNode[] = [];
  for (const n of nodes) {
    const last = merged[merged.length - 1];
    if (last && vlen(vsub(n.position, last.position)) < 1e-12) merged[merged.length - 1] = { ...last, outHandle: n.outHandle, nodeType: 'smooth' };
    else merged.push(n);
  }
  const head = merged[0];
  const tail = merged[merged.length - 1];
  if (merged.length > 1 && vlen(vsub(head.position, tail.position)) < 1e-12) {
    merged[0] = { ...tail, outHandle: head.outHandle, nodeType: 'smooth' };
    merged.pop();
  }
  return merged;
}

/** NACA 4-digit contour, trailing edge first, closed trailing edge, centred on mid-chord. */
export function nacaPoints(code: string, perSide = 90): Vec2[] {
  const m = Number(code[0]) / 100;
  const p = Number(code[1]) / 10;
  const t = Number(code.slice(2)) / 100;
  const thick = (x: number) => 5 * t * (0.2969 * Math.sqrt(x) - 0.126 * x - 0.3516 * x * x + 0.2843 * x ** 3 - 0.1036 * x ** 4);
  const camber = (x: number) =>
    m === 0
      ? { y: 0, dy: 0 }
      : x < p
        ? { y: (m / (p * p)) * (2 * p * x - x * x), dy: ((2 * m) / (p * p)) * (p - x) }
        : { y: (m / (1 - p) ** 2) * (1 - 2 * p + 2 * p * x - x * x), dy: ((2 * m) / (1 - p) ** 2) * (p - x) };
  const side = (sign: 1 | -1, x: number): Vec2 => {
    const { y, dy } = camber(x);
    const th = Math.atan(dy);
    return { x: x - sign * thick(x) * Math.sin(th) - 0.5, y: y + sign * thick(x) * Math.cos(th) };
  };
  const xs = Array.from({ length: perSide + 1 }, (_, i) => (1 - Math.cos((Math.PI * i) / perSide)) / 2);
  // TE → upper → LE → lower (skipping the repeated LE and TE).
  return [...xs.slice().reverse().map((x) => side(1, x)), ...xs.slice(1, -1).map((x) => side(-1, x))];
}

export function templateGeometry(id: BezierTemplate): BezierGeometry {
  const closed = (nodes: BezierNode[]): BezierGeometry => ({ kind: 'bezier', closed: true, nodes });
  switch (id) {
    case 'circle':
      return closed(ellipseNodes(0.5, 0.5));
    case 'ellipse':
      return closed(ellipseNodes(0.5, 0.2));
    case 'naca0012':
      return { ...fitPoints(nacaPoints('0012'), 2e-4).geometry, fitError: null };
    case 'naca2412':
      return { ...fitPoints(nacaPoints('2412'), 2e-4).geometry, fitError: null };
    case 'flatPlate':
      return closed(roundedRect(1, 0.02, 0));
    case 'roundedPlate':
      return closed(roundedRect(1, 0.08, 0.04));
    case 'roundedRect':
      return closed(roundedRect(0.8, 0.5, 0.12));
    case 'blank':
      return emptyBezier();
  }
}
