/**
 * The Bézier editing layer (PRD2 §38, §56 layers 6–9): node/handle placement on
 * screen, hit testing, and drawing. Interaction and the overlay share these
 * helpers so what you see is exactly what you can grab.
 */
import type { BezierNode, Scene, SceneBody, Vec2 } from '@/domain/types';
import { bodyToWorld, nearestOnCurve, validateCached, worldToBody, type HandleSide } from '@/domain/bezier';
import { vadd, vlen } from '@/domain/scene';
import { screenToWorld, worldToScreen, type Viewport } from './viewport';
import type { ThemeColors } from './overlay';

export const NODE_HIT_PX = 9;
export const HANDLE_HIT_PX = 8;
export const CURVE_HIT_PX = 7;
const NODE_HALF = 4.5;
/** Inside the node's own glyph the node always wins, so short handles cannot make it ungrabbable. */
const NODE_GLYPH_PX = NODE_HALF + 2;

export type BezierHit =
  | { kind: 'handle'; nodeId: string; side: HandleSide }
  | { kind: 'node'; nodeId: string }
  | { kind: 'curve'; segment: number; t: number; point: Vec2 };

/** The body being edited: the single selected Bézier body, if it is not locked. */
export function editTarget(scene: Scene, selectedIds: string[]): SceneBody | null {
  if (selectedIds.length !== 1) return null;
  const b = scene.bodies.find((x) => x.id === selectedIds[0]);
  return b && b.geometry.kind === 'bezier' && !b.locked ? b : null;
}

export const nodeScreen = (body: SceneBody, n: BezierNode, vp: Viewport): Vec2 => worldToScreen(bodyToWorld(body, n.position), vp);

export const handleScreen = (body: SceneBody, n: BezierNode, side: HandleSide, vp: Viewport): Vec2 =>
  worldToScreen(bodyToWorld(body, vadd(n.position, side === 'in' ? n.inHandle : n.outHandle)), vp);

const hasHandle = (n: BezierNode, side: HandleSide) => vlen(side === 'in' ? n.inHandle : n.outHandle) > 1e-12;

/** The node glyph, then handle grips (they sit on top of the curve), then the wider node area, then the curve. */
export function hitBezier(body: SceneBody, screen: Vec2, vp: Viewport, selectedNodeIds: string[]): BezierHit | null {
  if (body.geometry.kind !== 'bezier') return null;
  const g = body.geometry;
  const near = (a: Vec2, r: number) => Math.hypot(a.x - screen.x, a.y - screen.y) <= r;
  for (const n of g.nodes) {
    if (near(nodeScreen(body, n, vp), NODE_GLYPH_PX)) return { kind: 'node', nodeId: n.id };
  }
  for (const n of g.nodes) {
    if (!selectedNodeIds.includes(n.id)) continue;
    for (const side of ['out', 'in'] as const) {
      if (hasHandle(n, side) && near(handleScreen(body, n, side, vp), HANDLE_HIT_PX)) return { kind: 'handle', nodeId: n.id, side };
    }
  }
  for (let i = g.nodes.length - 1; i >= 0; i--) {
    if (near(nodeScreen(body, g.nodes[i], vp), NODE_HIT_PX)) return { kind: 'node', nodeId: g.nodes[i].id };
  }
  const hit = nearestOnCurve(g, worldToBody(body, screenToWorld(screen, vp)));
  if (hit && near(worldToScreen(bodyToWorld(body, hit.point), vp), CURVE_HIT_PX)) {
    return { kind: 'curve', segment: hit.segment, t: hit.t, point: hit.point };
  }
  return null;
}

export interface EditVisual {
  tool: string;
  selectedNodeIds: Set<string>;
  /** Curve point under the cursor where a click would insert a node (body-local). */
  insertHint: Vec2 | null;
}

/** Nodes, handles, insertion hint and self-intersection marks for one selected Bézier body. */
export function drawBezierEdit(ctx: CanvasRenderingContext2D, body: SceneBody, vp: Viewport, colors: ThemeColors, edit: EditVisual): void {
  if (body.geometry.kind !== 'bezier') return;
  const g = body.geometry;
  const editing = (edit.tool === 'node' || edit.tool === 'pen') && !body.locked;
  ctx.save();
  ctx.setLineDash([]);
  ctx.lineWidth = 1.25;

  if (editing) {
    // Handles of selected nodes: a stem and a round grip.
    for (const n of g.nodes) {
      if (!edit.selectedNodeIds.has(n.id)) continue;
      const p = nodeScreen(body, n, vp);
      for (const side of ['in', 'out'] as const) {
        if (!hasHandle(n, side)) continue;
        const h = handleScreen(body, n, side, vp);
        ctx.strokeStyle = colors.accent;
        ctx.beginPath();
        ctx.moveTo(p.x, p.y);
        ctx.lineTo(h.x, h.y);
        ctx.stroke();
        ctx.fillStyle = colors.handle;
        ctx.beginPath();
        ctx.arc(h.x, h.y, 4, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
      }
    }
    // Nodes: squares for corners, circles for smooth/symmetric.
    for (const n of g.nodes) {
      const p = nodeScreen(body, n, vp);
      const sel = edit.selectedNodeIds.has(n.id);
      ctx.fillStyle = sel ? colors.select : colors.handle;
      ctx.strokeStyle = sel ? colors.select : colors.accent;
      ctx.lineWidth = sel ? 1.8 : 1.25;
      ctx.beginPath();
      if (n.nodeType === 'corner') ctx.rect(p.x - NODE_HALF, p.y - NODE_HALF, NODE_HALF * 2, NODE_HALF * 2);
      else ctx.arc(p.x, p.y, NODE_HALF, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    }
    if (edit.insertHint) {
      const p = worldToScreen(bodyToWorld(body, edit.insertHint), vp);
      ctx.strokeStyle = colors.accent;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(p.x, p.y, 6, 0, Math.PI * 2);
      ctx.moveTo(p.x - 3, p.y);
      ctx.lineTo(p.x + 3, p.y);
      ctx.moveTo(p.x, p.y - 3);
      ctx.lineTo(p.x, p.y + 3);
      ctx.stroke();
    }
  }

  // Self-intersection marks (PRD2 §50).
  ctx.strokeStyle = colors.danger;
  ctx.lineWidth = 2;
  for (const x of validateCached(g).intersections) {
    const p = worldToScreen(bodyToWorld(body, x), vp);
    ctx.beginPath();
    ctx.moveTo(p.x - 5, p.y - 5);
    ctx.lineTo(p.x + 5, p.y + 5);
    ctx.moveTo(p.x + 5, p.y - 5);
    ctx.lineTo(p.x - 5, p.y + 5);
    ctx.stroke();
  }
  ctx.restore();
}
