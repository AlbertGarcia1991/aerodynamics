/**
 * Pointer interaction for the canvas (PRD §9, §34): select, drag, rotate,
 * box-select, pan, zoom, pinch, place-on-click, seed placement, hover probe.
 *
 * A drag gesture is wrapped in a simulation-store *transaction* so that one
 * drag produces one undo entry and every intermediate step is a transient
 * update (PRD §9.2, §50).
 */
import type { Vec2 } from '@/domain/types';
import { elementPosition } from '@/domain/scene';
import { useSimulationStore } from '@/state/simulationStore';
import { useUIStore } from '@/state/uiStore';
import { useViewportStore } from '@/state/viewportStore';
import { useVisualizationStore } from '@/state/visualizationStore';
import { useSolverStore } from '@/state/solverStore';
import { solverRef } from '@/App';
import { currentPolygon, pointInPolygon } from './bodyCache';
import { GLYPH_RADIUS, rotationHandleScreen } from './overlay';
import { screenToWorld, worldToScreen, type Viewport } from './viewport';

type Mode =
  | { kind: 'idle' }
  | { kind: 'pan'; last: Vec2 }
  | { kind: 'drag'; ids: string[]; startWorld: Vec2; origins: Map<string, Vec2>; moved: boolean }
  | { kind: 'rotate'; id: string; startAngle: number; startRotation: number }
  | { kind: 'box'; start: Vec2; current: Vec2; additive: boolean }
  | { kind: 'pinch'; pointers: Map<number, Vec2>; lastDist: number; lastMid: Vec2 }
  | { kind: 'pressed'; id: string | null; start: Vec2; additive: boolean; button: number };

export type Cursor = 'default' | 'grab' | 'grabbing' | 'move' | 'crosshair' | 'pointer';

export interface InteractionView {
  pointer: Vec2 | null;
  box: { a: Vec2; b: Vec2 } | null;
  cursor: Cursor;
}

const DRAG_THRESHOLD = 3;

export class CanvasInteraction {
  private mode: Mode = { kind: 'idle' };
  private pointers = new Map<number, Vec2>();
  pointer: Vec2 | null = null;
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
    else if (m.kind === 'drag' || m.kind === 'rotate') cursor = 'move';
    else if (ui.tool === 'pan') cursor = 'grab';
    else if (ui.pendingAdd || ui.tool === 'seed') cursor = 'crosshair';
    else if (ui.hoverId) cursor = 'pointer';
    return {
      pointer: this.pointer,
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
        } else if (e.shiftKey || m.additive) {
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
        if (m.id) ui.select([m.id], m.additive);
        else if (!m.additive) ui.clearSelection();
        break;
      case 'drag':
        sim.commitTransaction();
        break;
      case 'rotate':
        sim.commitTransaction();
        break;
      case 'box': {
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
    const hit = this.hitTest(this.local(e));
    if (hit) {
      useUIStore.getState().select([hit.id]);
      useUIStore.getState().setPanel('right', true);
    }
  };
}
