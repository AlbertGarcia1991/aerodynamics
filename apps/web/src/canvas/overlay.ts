/**
 * Canvas 2D overlay: grid, axes, streamlines, particles, vectors, bodies,
 * element glyphs, selection handles. Everything crisp and theme-aware; the
 * continuous field lives on the WebGL layer underneath.
 */
import type { Scene, SceneBody, SceneElement, StreamlinesResult, Vec2, VectorFieldResult } from '@/domain/types';
import { elementPosition } from '@/domain/scene';
import type { VisualizationStore } from '@/state/visualizationStore';
import { bodyRadius, currentPolygon } from './bodyCache';
import { formatCoordinate, niceStep, visibleBounds, worldToScreen, type Viewport } from './viewport';
import { sampleColormap } from '@/render/colormaps';

export interface ThemeColors {
  gridMinor: string;
  gridMajor: string;
  axis: string;
  text: string;
  textMuted: string;
  bodyFill: string;
  bodyStroke: string;
  streamline: string;
  particle: string;
  vector: string;
  accent: string;
  select: string;
  handle: string;
  surface: string;
}

export function readThemeColors(): ThemeColors {
  const cs = getComputedStyle(document.documentElement);
  const v = (name: string) => cs.getPropertyValue(name).trim();
  return {
    gridMinor: v('--grid-minor'),
    gridMajor: v('--grid-major'),
    axis: v('--axis'),
    text: v('--text'),
    textMuted: v('--text-muted'),
    bodyFill: v('--body-fill'),
    bodyStroke: v('--body-stroke'),
    streamline: v('--streamline'),
    particle: v('--particle'),
    vector: v('--vector'),
    accent: v('--accent'),
    select: v('--select'),
    handle: v('--handle'),
    surface: v('--surface'),
  };
}

export interface DragVisual {
  box?: { a: Vec2; b: Vec2 } | null;
  /** Screen position of the pointer, for the placement ghost. */
  pointer?: Vec2 | null;
}

export interface OverlayState {
  vp: Viewport;
  dpr: number;
  scene: Scene;
  selectedIds: Set<string>;
  hoverId: string | null;
  viz: VisualizationStore;
  streamlines: StreamlinesResult | null;
  vectors: VectorFieldResult | null;
  vectorBounds: { min: Vec2; max: Vec2 } | null;
  time: number;
  colors: ThemeColors;
  drag: DragVisual;
  pendingAdd: string | null;
  reducedMotion: boolean;
}

export const GLYPH_RADIUS = 11;
export const HANDLE_OFFSET_PX = 28;

export function drawOverlay(ctx: CanvasRenderingContext2D, s: OverlayState): void {
  const { vp, dpr } = s;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, vp.width, vp.height);
  if (s.viz.showGrid) drawGrid(ctx, s);
  if (s.viz.showAxes) drawAxes(ctx, s);
  if (s.streamlines && s.viz.streamlines.show) drawStreamlines(ctx, s);
  if (s.vectors && s.viz.vectors.show) drawVectors(ctx, s);
  drawBodies(ctx, s);
  drawElements(ctx, s);
  drawSeeds(ctx, s);
  if (s.drag.box) drawBox(ctx, s);
  if (s.pendingAdd && s.drag.pointer) drawGhost(ctx, s);
  drawScaleBar(ctx, s);
}

/** A scale bar in the bottom-left corner (PRD §34 "scale"). */
function drawScaleBar(ctx: CanvasRenderingContext2D, s: OverlayState) {
  const { vp, colors } = s;
  const step = niceStep(vp.scale, 120);
  const px = step * vp.scale;
  const x = 14;
  const y = vp.height - 22;
  ctx.strokeStyle = colors.text;
  ctx.fillStyle = colors.text;
  ctx.lineWidth = 1.5;
  ctx.setLineDash([]);
  ctx.beginPath();
  ctx.moveTo(x, y - 5);
  ctx.lineTo(x, y);
  ctx.lineTo(x + px, y);
  ctx.lineTo(x + px, y - 5);
  ctx.stroke();
  ctx.font = '10px ui-monospace, SFMono-Regular, Menlo, monospace';
  ctx.textAlign = 'left';
  ctx.textBaseline = 'bottom';
  const label = step >= 1000 ? `${step / 1000} km` : step >= 1 ? `${step} m` : step >= 0.01 ? `${Math.round(step * 100)} cm` : `${Math.round(step * 1000)} mm`;
  ctx.fillText(label, x + 4, y - 3);
}

function drawGrid(ctx: CanvasRenderingContext2D, s: OverlayState) {
  const { vp, colors } = s;
  const b = visibleBounds(vp);
  const major = niceStep(vp.scale, 110);
  const minor = major / 5;
  const drawLines = (step: number, color: string, width: number) => {
    ctx.strokeStyle = color;
    ctx.lineWidth = width;
    ctx.beginPath();
    for (let x = Math.ceil(b.min.x / step) * step; x <= b.max.x; x += step) {
      const sx = Math.round(worldToScreen({ x, y: 0 }, vp).x) + 0.5;
      ctx.moveTo(sx, 0);
      ctx.lineTo(sx, vp.height);
    }
    for (let y = Math.ceil(b.min.y / step) * step; y <= b.max.y; y += step) {
      const sy = Math.round(worldToScreen({ x: 0, y }, vp).y) + 0.5;
      ctx.moveTo(0, sy);
      ctx.lineTo(vp.width, sy);
    }
    ctx.stroke();
  };
  if (minor * vp.scale > 9) drawLines(minor, colors.gridMinor, 1);
  drawLines(major, colors.gridMajor, 1);
  // Coordinate labels along the bottom and left edges.
  ctx.fillStyle = colors.textMuted;
  ctx.font = '10px ui-monospace, SFMono-Regular, Menlo, monospace';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'bottom';
  for (let x = Math.ceil(b.min.x / major) * major; x <= b.max.x; x += major) {
    const sx = worldToScreen({ x, y: 0 }, vp).x;
    if (sx > 30 && sx < vp.width - 20) ctx.fillText(formatCoordinate(x, major), sx, vp.height - 4);
  }
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  for (let y = Math.ceil(b.min.y / major) * major; y <= b.max.y; y += major) {
    const sy = worldToScreen({ x: 0, y }, vp).y;
    if (sy > 14 && sy < vp.height - 24) ctx.fillText(formatCoordinate(y, major), 6, sy);
  }
}

function drawAxes(ctx: CanvasRenderingContext2D, s: OverlayState) {
  const { vp, colors } = s;
  const o = worldToScreen({ x: 0, y: 0 }, vp);
  ctx.strokeStyle = colors.axis;
  ctx.lineWidth = 1;
  ctx.beginPath();
  if (o.y >= 0 && o.y <= vp.height) {
    ctx.moveTo(0, Math.round(o.y) + 0.5);
    ctx.lineTo(vp.width, Math.round(o.y) + 0.5);
  }
  if (o.x >= 0 && o.x <= vp.width) {
    ctx.moveTo(Math.round(o.x) + 0.5, 0);
    ctx.lineTo(Math.round(o.x) + 0.5, vp.height);
  }
  ctx.stroke();
  if (o.x >= 0 && o.x <= vp.width && o.y >= 0 && o.y <= vp.height) {
    ctx.fillStyle = colors.axis;
    ctx.beginPath();
    ctx.arc(o.x, o.y, 2.5, 0, Math.PI * 2);
    ctx.fill();
  }
}

function drawStreamlines(ctx: CanvasRenderingContext2D, s: OverlayState) {
  const { vp, colors, streamlines: sl, viz } = s;
  if (!sl) return;
  const { data, offsets, count } = sl;
  const settings = viz.streamlines;

  // Per-line mean speed, for colouring and particle pacing.
  let maxMean = 1e-9;
  const means = new Float32Array(count);
  for (let l = 0; l < count; l++) {
    const a = offsets[l];
    const b = offsets[l + 1];
    let acc = 0;
    for (let i = a; i < b; i++) acc += data[i * 3 + 2];
    means[l] = b > a ? acc / (b - a) : 0;
    if (means[l] > maxMean) maxMean = means[l];
  }
  let minMean = Infinity;
  for (let l = 0; l < count; l++) minMean = Math.min(minMean, means[l]);
  if (!Number.isFinite(minMean)) minMean = 0;

  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  const buildPath = (l: number) => {
    const a = offsets[l];
    const b = offsets[l + 1];
    ctx.beginPath();
    for (let i = a; i < b; i++) {
      const p = worldToScreen({ x: data[i * 3], y: data[i * 3 + 1] }, vp);
      if (i === a) ctx.moveTo(p.x, p.y);
      else ctx.lineTo(p.x, p.y);
    }
  };

  ctx.setLineDash([]);
  ctx.lineWidth = settings.lineWidth;
  for (let l = 0; l < count; l++) {
    if (offsets[l + 1] - offsets[l] < 2) continue;
    buildPath(l);
    if (settings.colorBySpeed) {
      const t = (means[l] - minMean) / Math.max(maxMean - minMean, 1e-9);
      const [r, g, b] = sampleColormap(viz.colormap === 'coolwarm' ? 'viridis' : viz.colormap, t);
      ctx.strokeStyle = `rgba(${Math.round(r * 255)},${Math.round(g * 255)},${Math.round(b * 255)},0.9)`;
    } else {
      ctx.strokeStyle = colors.streamline;
    }
    ctx.stroke();
  }

  if (settings.particles && !s.reducedMotion) {
    ctx.strokeStyle = colors.particle;
    ctx.lineWidth = settings.lineWidth + 0.8;
    const base = 55 * settings.particleSpeed; // px/s for the fastest line
    for (let l = 0; l < count; l++) {
      if (offsets[l + 1] - offsets[l] < 2) continue;
      const speed = base * Math.max(0.15, means[l] / maxMean);
      ctx.setLineDash([2.5, 16]);
      ctx.lineDashOffset = -((s.time / 1000) * speed) % 18.5;
      buildPath(l);
      ctx.stroke();
    }
    ctx.setLineDash([]);
  }
}

function drawVectors(ctx: CanvasRenderingContext2D, s: OverlayState) {
  const { vp, colors, vectors: vf, viz } = s;
  if (!vf) return;
  const b = vf.bounds;
  const dx = (b.max.x - b.min.x) / (vf.nx - 1);
  const dy = (b.max.y - b.min.y) / (vf.ny - 1);
  const spacing = viz.vectors.spacingPx;
  const maxLen = spacing * 0.85 * viz.vectors.scale;
  ctx.strokeStyle = colors.vector;
  ctx.fillStyle = colors.vector;
  ctx.lineWidth = 1.2;
  ctx.setLineDash([]);
  for (let j = 0; j < vf.ny; j++) {
    for (let i = 0; i < vf.nx; i++) {
      const k = j * vf.nx + i;
      if (vf.mask[k]) continue;
      const u = vf.u[k];
      const v = vf.v[k];
      const mag = Math.hypot(u, v);
      if (!Number.isFinite(mag) || mag === 0) continue;
      const len = Math.min(maxLen, (mag / Math.max(vf.maxMagnitude, 1e-9)) * maxLen);
      if (len < 2) continue;
      const p = worldToScreen({ x: b.min.x + i * dx, y: b.min.y + j * dy }, vp);
      const ux = u / mag;
      const uy = -v / mag; // screen y down
      const tx = p.x + ux * len;
      const ty = p.y + uy * len;
      ctx.beginPath();
      ctx.moveTo(p.x - ux * len * 0.5, p.y - uy * len * 0.5);
      ctx.lineTo(tx - ux * len * 0.5, ty - uy * len * 0.5);
      ctx.stroke();
      const hx = tx - ux * len * 0.5;
      const hy = ty - uy * len * 0.5;
      const hs = Math.min(6, len * 0.4);
      ctx.beginPath();
      ctx.moveTo(hx, hy);
      ctx.lineTo(hx - ux * hs - uy * hs * 0.5, hy - uy * hs + ux * hs * 0.5);
      ctx.lineTo(hx - ux * hs + uy * hs * 0.5, hy - uy * hs - ux * hs * 0.5);
      ctx.closePath();
      ctx.fill();
    }
  }
}

function tracePolygon(ctx: CanvasRenderingContext2D, poly: Vec2[], vp: Viewport) {
  ctx.beginPath();
  poly.forEach((p, i) => {
    const q = worldToScreen(p, vp);
    if (i === 0) ctx.moveTo(q.x, q.y);
    else ctx.lineTo(q.x, q.y);
  });
  ctx.closePath();
}

export function rotationHandleWorld(body: SceneBody): Vec2 | null {
  return { x: body.position.x, y: body.position.y + bodyRadius(body) };
}

function drawBodies(ctx: CanvasRenderingContext2D, s: OverlayState) {
  const { vp, colors } = s;
  for (const body of s.scene.bodies) {
    if (!body.visible) continue;
    const poly = currentPolygon(body);
    const selected = s.selectedIds.has(body.id);
    const hovered = s.hoverId === body.id;
    if (!poly) {
      // Before the first solve: a marker at the body origin.
      const p = worldToScreen(body.position, vp);
      ctx.strokeStyle = colors.bodyStroke;
      ctx.setLineDash([3, 3]);
      ctx.beginPath();
      ctx.arc(p.x, p.y, 10, 0, Math.PI * 2);
      ctx.stroke();
      ctx.setLineDash([]);
      continue;
    }
    tracePolygon(ctx, poly, vp);
    ctx.fillStyle = colors.bodyFill;
    ctx.fill();
    ctx.lineWidth = selected ? 2.2 : hovered ? 1.8 : 1.2;
    ctx.strokeStyle = selected ? colors.select : hovered ? colors.accent : colors.bodyStroke;
    ctx.setLineDash([]);
    ctx.stroke();

    if (selected) {
      // Origin cross and rotation handle.
      const o = worldToScreen(body.position, vp);
      ctx.strokeStyle = colors.select;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(o.x - 6, o.y);
      ctx.lineTo(o.x + 6, o.y);
      ctx.moveTo(o.x, o.y - 6);
      ctx.lineTo(o.x, o.y + 6);
      ctx.stroke();
      const hw = rotationHandleWorld(body);
      if (hw) {
        const h = worldToScreen(hw, vp);
        const hy = h.y - HANDLE_OFFSET_PX;
        ctx.setLineDash([3, 3]);
        ctx.beginPath();
        ctx.moveTo(o.x, o.y);
        ctx.lineTo(h.x, hy);
        ctx.stroke();
        ctx.setLineDash([]);
        ctx.fillStyle = colors.handle;
        ctx.beginPath();
        ctx.arc(h.x, hy, 6, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
        // Tiny rotate glyph.
        ctx.beginPath();
        ctx.arc(h.x, hy, 3, -Math.PI * 0.9, Math.PI * 0.6);
        ctx.stroke();
      }
    }
  }
}

export function rotationHandleScreen(body: SceneBody, vp: Viewport): Vec2 | null {
  const hw = rotationHandleWorld(body);
  if (!hw) return null;
  const h = worldToScreen(hw, vp);
  return { x: h.x, y: h.y - HANDLE_OFFSET_PX };
}

function arrow(ctx: CanvasRenderingContext2D, x: number, y: number, dx: number, dy: number, len: number, head = 4) {
  const n = Math.hypot(dx, dy) || 1;
  const ux = dx / n;
  const uy = dy / n;
  const tx = x + ux * len;
  const ty = y + uy * len;
  ctx.beginPath();
  ctx.moveTo(x, y);
  ctx.lineTo(tx, ty);
  ctx.stroke();
  ctx.beginPath();
  ctx.moveTo(tx, ty);
  ctx.lineTo(tx - ux * head - uy * head * 0.6, ty - uy * head + ux * head * 0.6);
  ctx.lineTo(tx - ux * head + uy * head * 0.6, ty - uy * head - ux * head * 0.6);
  ctx.closePath();
  ctx.fill();
}

export function drawElementGlyph(ctx: CanvasRenderingContext2D, el: SceneElement['element'], p: Vec2, color: string, bg: string, r = GLYPH_RADIUS) {
  ctx.lineWidth = 1.6;
  ctx.strokeStyle = color;
  ctx.fillStyle = color;
  ctx.setLineDash([]);
  switch (el.type) {
    case 'source': {
      ctx.beginPath();
      ctx.arc(p.x, p.y, r * 0.42, 0, Math.PI * 2);
      ctx.fill();
      for (let k = 0; k < 4; k++) {
        const a = (k * Math.PI) / 2 + Math.PI / 4;
        arrow(ctx, p.x + Math.cos(a) * r * 0.55, p.y + Math.sin(a) * r * 0.55, Math.cos(a), Math.sin(a), r * 0.75, 3.5);
      }
      break;
    }
    case 'sink': {
      ctx.beginPath();
      ctx.arc(p.x, p.y, r * 0.42, 0, Math.PI * 2);
      ctx.stroke();
      for (let k = 0; k < 4; k++) {
        const a = (k * Math.PI) / 2 + Math.PI / 4;
        arrow(ctx, p.x + Math.cos(a) * r * 1.35, p.y + Math.sin(a) * r * 1.35, -Math.cos(a), -Math.sin(a), r * 0.7, 3.5);
      }
      break;
    }
    case 'vortex': {
      const ccw = el.circulation >= 0;
      ctx.beginPath();
      ctx.arc(p.x, p.y, r * 0.25, 0, Math.PI * 2);
      ctx.fill();
      ctx.beginPath();
      // Screen y is down, so a counter-clockwise (world) arc is clockwise on screen.
      ctx.arc(p.x, p.y, r * 0.85, -Math.PI * 0.75, Math.PI * 0.55, ccw);
      ctx.stroke();
      const endA = ccw ? -Math.PI * 0.75 : Math.PI * 0.55;
      const ex = p.x + Math.cos(endA) * r * 0.85;
      const ey = p.y + Math.sin(endA) * r * 0.85;
      // Tangent direction at the arc end.
      const tdir = ccw ? -1 : 1;
      arrow(ctx, ex, ey, -Math.sin(endA) * tdir, Math.cos(endA) * tdir, 0.1, 5);
      break;
    }
    case 'doublet': {
      ctx.save();
      ctx.translate(p.x, p.y);
      ctx.rotate(-el.orientation);
      ctx.beginPath();
      ctx.ellipse(0, 0, r, r * 0.55, 0, 0, Math.PI * 2);
      ctx.fillStyle = bg;
      ctx.fill();
      ctx.stroke();
      ctx.fillStyle = color;
      arrow(ctx, -r * 0.55, 0, 1, 0, r * 1.1, 4);
      ctx.restore();
      break;
    }
    case 'uniformFlow': {
      for (let k = -1; k <= 1; k++) arrow(ctx, p.x - r, p.y + k * 6, Math.cos(-el.direction), Math.sin(-el.direction), 2 * r, 4);
      break;
    }
  }
}

function drawElements(ctx: CanvasRenderingContext2D, s: OverlayState) {
  const { vp, colors } = s;
  let uniformSlot = 0;
  for (const e of s.scene.elements) {
    if (!e.visible) continue;
    const selected = s.selectedIds.has(e.id);
    const hovered = s.hoverId === e.id;
    const color = selected ? colors.select : hovered ? colors.accent : colors.text;
    const pos = elementPosition(e.element);
    let p: Vec2;
    if (pos) p = worldToScreen(pos, vp);
    else {
      // Uniform-flow elements have no location: stack them top-left.
      p = { x: 36, y: 64 + uniformSlot * 34 };
      uniformSlot++;
    }
    // Halo for legibility over any field colour.
    ctx.fillStyle = colors.surface;
    ctx.globalAlpha = 0.85;
    ctx.beginPath();
    ctx.arc(p.x, p.y, GLYPH_RADIUS + 5, 0, Math.PI * 2);
    ctx.fill();
    ctx.globalAlpha = 1;
    if (!e.visible) ctx.globalAlpha = 0.4;
    drawElementGlyph(ctx, e.element, p, color, colors.surface);
    ctx.globalAlpha = 1;
    if (selected || hovered) {
      ctx.strokeStyle = color;
      ctx.lineWidth = selected ? 2 : 1.2;
      ctx.setLineDash(selected ? [] : [3, 3]);
      ctx.beginPath();
      ctx.arc(p.x, p.y, GLYPH_RADIUS + 7, 0, Math.PI * 2);
      ctx.stroke();
      ctx.setLineDash([]);
    }
    if (selected || hovered || e.locked) {
      ctx.fillStyle = colors.textMuted;
      ctx.font = '11px system-ui, sans-serif';
      ctx.textAlign = 'left';
      ctx.textBaseline = 'middle';
      ctx.fillText(e.name + (e.locked ? ' 🔒' : ''), p.x + GLYPH_RADIUS + 12, p.y);
    }
  }
}

function drawSeeds(ctx: CanvasRenderingContext2D, s: OverlayState) {
  const { vp, colors } = s;
  if (s.viz.seeds.length === 0) return;
  ctx.fillStyle = colors.accent;
  ctx.strokeStyle = colors.surface;
  ctx.lineWidth = 1;
  for (const seed of s.viz.seeds) {
    const p = worldToScreen(seed, vp);
    ctx.beginPath();
    ctx.moveTo(p.x, p.y - 5);
    ctx.lineTo(p.x + 5, p.y);
    ctx.lineTo(p.x, p.y + 5);
    ctx.lineTo(p.x - 5, p.y);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
  }
}

function drawBox(ctx: CanvasRenderingContext2D, s: OverlayState) {
  const box = s.drag.box!;
  ctx.strokeStyle = s.colors.accent;
  ctx.fillStyle = s.colors.accent;
  ctx.globalAlpha = 0.12;
  ctx.fillRect(box.a.x, box.a.y, box.b.x - box.a.x, box.b.y - box.a.y);
  ctx.globalAlpha = 1;
  ctx.lineWidth = 1;
  ctx.setLineDash([4, 3]);
  ctx.strokeRect(box.a.x + 0.5, box.a.y + 0.5, box.b.x - box.a.x, box.b.y - box.a.y);
  ctx.setLineDash([]);
}

function drawGhost(ctx: CanvasRenderingContext2D, s: OverlayState) {
  const p = s.drag.pointer!;
  const kind = s.pendingAdd as SceneElement['element']['type'];
  const ghost: SceneElement['element'] =
    kind === 'source' ? { type: 'source', position: { x: 0, y: 0 }, strength: 1 }
    : kind === 'sink' ? { type: 'sink', position: { x: 0, y: 0 }, strength: 1 }
    : kind === 'vortex' ? { type: 'vortex', position: { x: 0, y: 0 }, circulation: -1 }
    : kind === 'doublet' ? { type: 'doublet', position: { x: 0, y: 0 }, strength: 1, orientation: 0 }
    : { type: 'uniformFlow', velocity: 1, direction: 0 };
  ctx.globalAlpha = 0.6;
  drawElementGlyph(ctx, ghost, p, s.colors.accent, s.colors.surface);
  ctx.globalAlpha = 1;
  ctx.fillStyle = s.colors.textMuted;
  ctx.font = '11px system-ui, sans-serif';
  ctx.textAlign = 'left';
  ctx.fillText('click to place · Esc to cancel', p.x + 18, p.y + 18);
}
