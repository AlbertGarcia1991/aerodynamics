/**
 * Pointer interaction for the canvas (PRD §9, §34): select, drag, rotate,
 * box-select, pan, zoom, pinch, place-on-click, seed placement, hover probe.
 *
 * A drag gesture is wrapped in a simulation-store *transaction* so that one
 * drag produces one undo entry and every intermediate step is a transient
 * update (PRD §9.2, §50).
 */
import type { BezierGeometry, Vec2 } from '@/domain/types';
import { appendNode, emptyBezier, insertNodeAt, moveNodes, setClosed, setNodeHandle, setNodeType, worldToBody, type HandleSide } from '@/domain/bezier';
import { createBody, elementPosition, vsub } from '@/domain/scene';
import { useSimulationStore } from '@/state/simulationStore';
import { useUIStore } from '@/state/uiStore';
import { useViewportStore } from '@/state/viewportStore';
import { useVisualizationStore } from '@/state/visualizationStore';
import { useSolverStore } from '@/state/solverStore';
import { solverRef } from '@/App';
import { currentPolygon, pointInPolygon } from './bodyCache';
import { editTarget, hitBezier, nodeScreen as nodeScreenOf } from './bezierEdit';
import { GLYPH_RADIUS, rotationHandleScreen } from './overlay';
import { screenToWorld, worldToScreen, type Viewport } from './viewport';

type Mode =
  | { kind: 'idle' }
  | { kind: 'pan'; last: Vec2 }
  | { kind: 'drag'; ids: string[]; startWorld: Vec2; origins: Map<string, Vec2>; moved: boolean }
  | { kind: 'rotate'; id: string; startAngle: number; startRotation: number }
  | { kind: 'box'; start: Vec2; current: Vec2; additive: boolean }
  | { kind: 'pinch'; pointers: Map<number, Vec2>; lastDist: number; lastMid: Vec2 }
  | { kind: 'pressed'; id: string | null; start: Vec2; additive: boolean; button: number }
  // Bézier editing: every move is recomputed from the geometry at gesture start, so nothing drifts.
  | { kind: 'node'; bodyId: string; start: BezierGeometry; ids: string[]; startLocal: Vec2; moved: boolean; clicked: string; additive: boolean; wasSelected: boolean }
  | { kind: 'handle'; bodyId: string; start: BezierGeometry; nodeId: string; side: HandleSide }
  | { kind: 'pen'; bodyId: string; nodeId: string; anchor: Vec2; base: BezierGeometry; dragged: boolean };

export type Cursor = 'default' | 'grab' | 'grabbing' | 'move' | 'crosshair' | 'pointer';

export interface InteractionView {
  pointer: Vec2 | null;
  /** Body-local curve point where a click would insert a node. */
  insertHint: Vec2 | null;
  box: { a: Vec2; b: Vec2 } | null;
  cursor: Cursor;
}

const DRAG_THRESHOLD = 3;

export class CanvasInteraction {
  private mode: Mode = { kind: 'idle' };
  private pointers = new Map<number, Vec2>();
  pointer: Vec2 | null = null;
  private insertHint: Vec2 | null = null;
  private onChange: () => void;

  constructor(private el: HTMLElement, onChange: () => void) {
    this.onChange = onChange;
    el.addEventListener('pointerdown', this.onDown);
    el.addEventListener('pointermove', this.onMove);
    el.addEventListener('pointerup', this.onUp);
    el.addEventListener('pointercancel', this.onUp);
    el.addEventListener('pointerleave', this.onLeave);
    el.addEventListener('wheel', this.onWheel, { passive: false });
    el.addEventListener('dblclick', this.onDblClick);
    el.addEventListener('contextmenu', (e) => e.preventDefault());
  }

  dispose(): void {
    this.el.removeEventListener('pointerdown', this.onDown);
    this.el.removeEventListener('pointermove', this.onMove);
    this.el.removeEventListener('pointerup', this.onUp);
    this.el.removeEventListener('pointercancel', this.onUp);
    this.el.removeEventListener('pointerleave', this.onLeave);
    this.el.removeEventListener('wheel', this.onWheel);
    this.el.removeEventListener('dblclick', this.onDblClick);
  }

  view(): InteractionView {
    const m = this.mode;
    const ui = useUIStore.getState();
    let cursor: Cursor = 'default';
    if (m.kind === 'pan') cursor = 'grabbing';
    else if (m.kind === 'drag' || m.kind === 'rotate' || m.kind === 'node' || m.kind === 'handle') cursor = 'move';
    else if (ui.tool === 'pan') cursor = 'grab';
    else if (ui.pendingAdd || ui.tool === 'seed' || ui.tool === 'pen') cursor = 'crosshair';
    else if (ui.hoverId) cursor = 'pointer';
    return {
      pointer: this.pointer,
      insertHint: this.insertHint,
      box: m.kind === 'box' ? { a: m.start, b: m.current } : null,
      cursor,
    };
  }

  /**
   * The HUD (toolbar, legend, read-out) is layered inside the canvas container.
   * Only events that start on the drawing surface itself belong to the scene;
   * capturing the pointer for a toolbar press would swallow the button's click.
   */
  private onSurface(e: Event): boolean {
    const t = e.target as HTMLElement | null;
    return t === this.el || t?.tagName === 'CANVAS';
  }

  private local(e: PointerEvent | WheelEvent | MouseEvent): Vec2 {
    const r = this.el.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  private vp(): Viewport {
    return useViewportStore.getState();
  }

  /** Topmost object under a screen point: elements first (small, on top), then bodies. */
  hitTest(screen: Vec2): { id: string; kind: 'element' | 'body' } | null {
    const scene = useSimulationStore.getState().scene;
    const vp = this.vp();
    let uniformSlot = 0;
    for (let i = scene.elements.length - 1; i >= 0; i--) {
      const e = scene.elements[i];
      if (!e.visible) continue;
      const pos = elementPosition(e.element);
      let p: Vec2;
      if (pos) p = worldToScreen(pos, vp);
      else {
        p = { x: 36, y: 64 + uniformSlot * 34 };
        uniformSlot++;
      }
      if (Math.hypot(p.x - screen.x, p.y - screen.y) <= GLYPH_RADIUS + 6) return { id: e.id, kind: 'element' };
    }
    const world = screenToWorld(screen, vp);
    for (let i = scene.bodies.length - 1; i >= 0; i--) {
      const b = scene.bodies[i];
      if (!b.visible) continue;
      const poly = currentPolygon(b);
      if (poly ? pointInPolygon(world, poly) : Math.hypot(worldToScreen(b.position, vp).x - screen.x, worldToScreen(b.position, vp).y - screen.y) < 12) {
        return { id: b.id, kind: 'body' };
      }
    }
    return null;
  }

  private hitRotationHandle(screen: Vec2): string | null {
    const ui = useUIStore.getState();
    if (ui.selectedIds.length !== 1) return null;
    const body = useSimulationStore.getState().scene.bodies.find((b) => b.id === ui.selectedIds[0]);
    if (!body || body.locked) return null;
    const h = rotationHandleScreen(body, this.vp());
    if (!h) return null;
    return Math.hypot(h.x - screen.x, h.y - screen.y) <= 9 ? body.id : null;
  }

  /**
   * Node and pen tools. Returns false when the press hit nothing editable, so
   * the caller falls back to ordinary select / box-select / pan behaviour.
   */
  private beginBezierGesture(p: Vec2, e: PointerEvent): boolean {
    const ui = useUIStore.getState();
    const sim = useSimulationStore.getState();
    const vp = this.vp();
    const world = screenToWorld(p, vp);
    let body = editTarget(sim.scene, ui.selectedIds);
    const additive = e.shiftKey || e.ctrlKey || e.metaKey;

    if (ui.tool === 'pen' && !(body?.geometry.kind === 'bezier' && body.geometry.closed)) {
      sim.beginTransaction();
      if (!body) {
        // Nothing editable selected: the pen starts a new body, with an identity transform so local = world.
        const nb = createBody(sim.scene, emptyBezier());
        sim.update((sc) => void sc.bodies.push(nb), { transient: true });
        ui.select([nb.id]);
        body = nb;
      }
      const g = body.geometry as BezierGeometry;
      const first = g.nodes[0];
      const onFirst = hitBezier(body, p, vp, []);
      if (!g.closed && g.nodes.length >= 3 && first && onFirst?.kind === 'node' && onFirst.nodeId === first.id) {
        sim.updateBody(body.id, (b) => ({ ...b, geometry: setClosed(g, true) }), { transient: true });
        sim.commitTransaction();
        this.onChange();
        return true;
      }
      const local = worldToBody(body, world);
      const { geometry, nodeId } = appendNode(g, local);
      sim.updateBody(body.id, (b) => ({ ...b, geometry }), { transient: true });
      ui.selectNodes([nodeId]);
      this.mode = { kind: 'pen', bodyId: body.id, nodeId, anchor: local, base: geometry, dragged: false };
      this.onChange();
      return true;
    }

    if (!body) return false;
    const hit = hitBezier(body, p, vp, ui.selectedNodeIds);
    if (!hit) return false;
    const g = body.geometry as BezierGeometry;
    sim.beginTransaction();
    if (hit.kind === 'handle') {
      this.mode = { kind: 'handle', bodyId: body.id, start: g, nodeId: hit.nodeId, side: hit.side };
    } else if (hit.kind === 'node') {
      const wasSelected = ui.selectedNodeIds.includes(hit.nodeId);
      if (!wasSelected) ui.selectNodes([hit.nodeId], additive);
      this.mode = { kind: 'node', bodyId: body.id, start: g, ids: useUIStore.getState().selectedNodeIds, startLocal: worldToBody(body, world), moved: false, clicked: hit.nodeId, additive, wasSelected };
    } else {
      // Click on the curve: split it exactly, then let the new node be dragged straight away.
      const { geometry, nodeId } = insertNodeAt(g, hit.segment, hit.t);
      sim.updateBody(body.id, (b) => ({ ...b, geometry }), { transient: true });
      ui.selectNodes([nodeId]);
      this.mode = { kind: 'node', bodyId: body.id, start: geometry, ids: [nodeId], startLocal: worldToBody(body, world), moved: false, clicked: nodeId, additive: false, wasSelected: true };
    }
    this.onChange();
    return true;
  }

  private onDown = (e: PointerEvent) => {
    if (!this.onSurface(e)) return;
    const p = this.local(e);
    this.pointers.set(e.pointerId, p);
    this.el.setPointerCapture(e.pointerId);
    this.pointer = p;
    const ui = useUIStore.getState();
    const sim = useSimulationStore.getState();

    if (this.pointers.size === 2) {
      const [a, b] = Array.from(this.pointers.values());
      this.mode = { kind: 'pinch', pointers: new Map(this.pointers), lastDist: Math.hypot(a.x - b.x, a.y - b.y), lastMid: { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 } };
      this.onChange();
      return;
    }

    // Middle button, pan tool, or space-held → pan.
    if (e.button === 1 || ui.tool === 'pan') {
      this.mode = { kind: 'pan', last: p };
      this.onChange();
      return;
    }
    if (e.button !== 0) return;

    const world = screenToWorld(p, this.vp());
    if (ui.pendingAdd) {
      const id = sim.addElement(ui.pendingAdd, world);
      ui.setPendingAdd(null);
      ui.select([id]);
      this.onChange();
      return;
    }
    if (ui.tool === 'seed') {
      useVisualizationStore.getState().addSeed(world);
      this.onChange();
      return;
    }

    if ((ui.tool === 'node' || ui.tool === 'pen') && this.beginBezierGesture(p, e)) return;

    const handleId = this.hitRotationHandle(p);
    if (handleId) {
      const body = sim.scene.bodies.find((b) => b.id === handleId)!;
      const c = worldToScreen(body.position, this.vp());
      sim.beginTransaction();
      this.mode = { kind: 'rotate', id: handleId, startAngle: Math.atan2(-(p.y - c.y), p.x - c.x), startRotation: body.rotation };
      this.onChange();
      return;
    }

    const hit = this.hitTest(p);
    this.mode = { kind: 'pressed', id: hit?.id ?? null, start: p, additive: e.shiftKey || e.ctrlKey || e.metaKey, button: e.button };
    this.onChange();
  };

  private onMove = (e: PointerEvent) => {
    const p = this.local(e);
    if (this.mode.kind === 'idle' && !this.onSurface(e)) {
      // Hovering the HUD: no hover highlight, no probe.
      if (this.pointer) {
        this.pointer = null;
        useUIStore.getState().setHover(null);
        this.onChange();
      }
      return;
    }
    this.pointer = p;
    if (this.pointers.has(e.pointerId)) this.pointers.set(e.pointerId, p);
    const vpStore = useViewportStore.getState();
    const ui = useUIStore.getState();
    const sim = useSimulationStore.getState();
    const m = this.mode;

    switch (m.kind) {
      case 'idle': {
        const target = ui.tool === 'node' ? editTarget(sim.scene, ui.selectedIds) : null;
        const bez = target ? hitBezier(target, p, this.vp(), ui.selectedNodeIds) : null;
        this.insertHint = bez?.kind === 'curve' ? bez.point : null;
        const hit = this.hitTest(p);
        ui.setHover(hit?.id ?? null);
        const w = screenToWorld(p, this.vp());
        solverRef.probe?.(w.x, w.y);
        break;
      }
      case 'pan':
        vpStore.pan(p.x - m.last.x, p.y - m.last.y);
        m.last = p;
        break;
      case 'pressed': {
        if (Math.hypot(p.x - m.start.x, p.y - m.start.y) < DRAG_THRESHOLD) break;
        if (m.id) {
          // Start dragging the pressed object (and the rest of the selection if it is part of it).
          const selected = ui.selectedIds.includes(m.id) ? ui.selectedIds : [m.id];
          if (!ui.selectedIds.includes(m.id)) ui.select([m.id], m.additive);
          const origins = new Map<string, Vec2>();
          const scene = sim.scene;
          for (const id of selected) {
            const el = scene.elements.find((x) => x.id === id);
            const bd = scene.bodies.find((x) => x.id === id);
            const locked = (el?.locked ?? bd?.locked) === true;
            const pos = el ? elementPosition(el.element) : bd?.position ?? null;
            if (pos && !locked) origins.set(id, pos);
          }
          if (origins.size === 0) {
            this.mode = { kind: 'idle' };
            break;
          }
          sim.beginTransaction();
          this.mode = { kind: 'drag', ids: Array.from(origins.keys()), startWorld: screenToWorld(m.start, this.vp()), origins, moved: false };
        } else if (e.shiftKey || m.additive || (ui.tool === 'node' && editTarget(sim.scene, ui.selectedIds))) {
          this.mode = { kind: 'box', start: m.start, current: p, additive: m.additive };
        } else {
          this.mode = { kind: 'pan', last: p };
        }
        break;
      }
      case 'drag': {
        const w = screenToWorld(p, this.vp());
        const dx = w.x - m.startWorld.x;
        const dy = w.y - m.startWorld.y;
        m.moved = true;
        sim.update((scene) => {
          for (const [id, o] of m.origins) {
            const pos = { x: o.x + dx, y: o.y + dy };
            scene.elements = scene.elements.map((el) => (el.id === id && el.element.type !== 'uniformFlow' ? { ...el, element: { ...el.element, position: pos } } : el));
            scene.bodies = scene.bodies.map((b) => (b.id === id ? { ...b, position: pos } : b));
          }
        }, { transient: true });
        break;
      }
      case 'rotate': {
        const body = sim.scene.bodies.find((b) => b.id === m.id);
        if (!body) break;
        const c = worldToScreen(body.position, this.vp());
        const a = Math.atan2(-(p.y - c.y), p.x - c.x);
        let rot = m.startRotation + (a - m.startAngle);
        if (e.shiftKey) rot = Math.round(rot / (Math.PI / 36)) * (Math.PI / 36); // 5° snaps
        sim.updateBody(m.id, (b) => ({ ...b, rotation: rot }), { transient: true });
        break;
      }
      case 'box':
        m.current = p;
        break;
      case 'node': {
        const body = sim.scene.bodies.find((b) => b.id === m.bodyId);
        if (!body) break;
        const delta = vsub(worldToBody(body, screenToWorld(p, this.vp())), m.startLocal);
        m.moved = true;
        sim.updateBody(m.bodyId, (b) => ({ ...b, geometry: moveNodes(m.start, m.ids, delta) }), { transient: true });
        break;
      }
      case 'handle': {
        const body = sim.scene.bodies.find((b) => b.id === m.bodyId);
        const node = m.start.nodes.find((n) => n.id === m.nodeId);
        if (!body || !node) break;
        const offset = vsub(worldToBody(body, screenToWorld(p, this.vp())), node.position);
        sim.updateBody(m.bodyId, (b) => ({ ...b, geometry: setNodeHandle(m.start, m.nodeId, m.side, offset) }), { transient: true });
        break;
      }
      case 'pen': {
        const body = sim.scene.bodies.find((b) => b.id === m.bodyId);
        if (!body) break;
        const out = vsub(worldToBody(body, screenToWorld(p, this.vp())), m.anchor);
        // A drag turns the click into a smooth node whose handles mirror each other.
        if (!m.dragged && Math.hypot(out.x, out.y) * this.vp().scale * body.scale < DRAG_THRESHOLD) break;
        m.dragged = true;
        const g = setNodeHandle(setNodeType(m.base, m.nodeId, 'symmetric'), m.nodeId, 'out', out);
        sim.updateBody(m.bodyId, (b) => ({ ...b, geometry: g }), { transient: true });
        break;
      }
      case 'pinch': {
        if (this.pointers.size < 2) break;
        const [a, b] = Array.from(this.pointers.values());
        const dist = Math.hypot(a.x - b.x, a.y - b.y);
        const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
        if (m.lastDist > 0) vpStore.zoom(mid, dist / m.lastDist);
        vpStore.pan(mid.x - m.lastMid.x, mid.y - m.lastMid.y);
        m.lastDist = dist;
        m.lastMid = mid;
        break;
      }
    }
    this.onChange();
  };

  private onUp = (e: PointerEvent) => {
    const p = this.local(e);
    this.pointers.delete(e.pointerId);
    const ui = useUIStore.getState();
    const sim = useSimulationStore.getState();
    const m = this.mode;
    switch (m.kind) {
      case 'pressed':
        // In the node tool a click inside the body being edited keeps its node selection.
        if (m.id && ui.tool === 'node' && ui.selectedIds.length === 1 && ui.selectedIds[0] === m.id) break;
        if (m.id) ui.select([m.id], m.additive);
        else if (!m.additive) ui.clearSelection();
        break;
      case 'drag':
        sim.commitTransaction();
        break;
      case 'rotate':
        sim.commitTransaction();
        break;
      case 'node':
        sim.commitTransaction();
        // A plain click (no drag) on one node of a multi-selection narrows to it; shift-click toggles it off.
        if (!m.moved && m.wasSelected) {
          if (m.additive) ui.selectNodes(ui.selectedNodeIds.filter((id) => id !== m.clicked));
          else ui.selectNodes([m.clicked]);
        }
        break;
      case 'handle':
      case 'pen':
        sim.commitTransaction();
        break;
      case 'box': {
        const target = ui.tool === 'node' ? editTarget(sim.scene, ui.selectedIds) : null;
        if (target && target.geometry.kind === 'bezier') {
          const vp = this.vp();
          const [x0, x1] = [Math.min(m.start.x, p.x), Math.max(m.start.x, p.x)];
          const [y0, y1] = [Math.min(m.start.y, p.y), Math.max(m.start.y, p.y)];
          const ids = target.geometry.nodes
            .filter((n) => {
              const s = nodeScreenOf(target, n, vp);
              return s.x >= x0 && s.x <= x1 && s.y >= y0 && s.y <= y1;
            })
            .map((n) => n.id);
          ui.selectNodes(ids, m.additive);
          break;
        }
        const vp = this.vp();
        const minX = Math.min(m.start.x, p.x);
        const maxX = Math.max(m.start.x, p.x);
        const minY = Math.min(m.start.y, p.y);
        const maxY = Math.max(m.start.y, p.y);
        const ids: string[] = [];
        for (const el of sim.scene.elements) {
          const pos = elementPosition(el.element);
          if (!pos || !el.visible) continue;
          const s = worldToScreen(pos, vp);
          if (s.x >= minX && s.x <= maxX && s.y >= minY && s.y <= maxY) ids.push(el.id);
        }
        for (const b of sim.scene.bodies) {
          if (!b.visible) continue;
          const s = worldToScreen(b.position, vp);
          if (s.x >= minX && s.x <= maxX && s.y >= minY && s.y <= maxY) ids.push(b.id);
        }
        ui.select(ids, m.additive);
        break;
      }
      default:
        break;
    }
    this.mode = this.pointers.size >= 2 ? this.mode : { kind: 'idle' };
    this.onChange();
  };

  private onLeave = () => {
    this.pointer = null;
    useUIStore.getState().setHover(null);
    useSolverStore.getState().setProbe(null);
    this.onChange();
  };

  private onWheel = (e: WheelEvent) => {
    if (!this.onSurface(e)) return;
    e.preventDefault();
    const p = this.local(e);
    const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 100 : 1;
    const delta = e.deltaY * unit;
    const factor = Math.exp(-delta * (e.ctrlKey ? 0.01 : 0.0018));
    useViewportStore.getState().zoom(p, factor);
    this.onChange();
  };

  private onDblClick = (e: MouseEvent) => {
    if (!this.onSurface(e)) return;
    const ui = useUIStore.getState();
    const sim = useSimulationStore.getState();
    const target = ui.tool === 'node' ? editTarget(sim.scene, ui.selectedIds) : null;
    const nodeHit = target ? hitBezier(target, this.local(e), this.vp(), ui.selectedNodeIds) : null;
    if (target && nodeHit?.kind === 'node' && target.geometry.kind === 'bezier') {
      // Double-click a node: toggle between a sharp corner and a smooth node.
      const n = target.geometry.nodes.find((x) => x.id === nodeHit.nodeId);
      const g = target.geometry;
      if (n) sim.updateBody(target.id, (b) => ({ ...b, geometry: setNodeType(g, n.id, n.nodeType === 'corner' ? 'smooth' : 'corner') }));
      return;
    }
    const hit = this.hitTest(this.local(e));
    if (hit) {
      useUIStore.getState().select([hit.id]);
      useUIStore.getState().setPanel('right', true);
    }
  };
}
