/**
 * SimulationState + HistoryState (PRD §49, §50).
 *
 * The scene is the single source of truth for *what the user built*; the
 * solver's results live in `solverStore` (scene state ≠ solution state, PRD
 * §86.4). Undo/redo is snapshot based: scenes are small (a few KB even with
 * imported geometry), so snapshots are simpler and more robust than inverse
 * operations. A *transaction* brackets a drag so that one gesture produces one
 * undo entry rather than sixty.
 */
import { create } from 'zustand';
import type { BodyGeometry, ElementKind, FlowConditions, Scene, SceneBody, SceneElement, Vec2 } from '@/domain/types';
import { cloneScene, createBody, createElement, createScene, newId, nextName, withElementPosition } from '@/domain/scene';

const HISTORY_LIMIT = 100;

export interface UpdateOptions {
  /** Do not record an undo entry (used for every step of a drag). */
  transient?: boolean;
}

export interface SimulationStore {
  scene: Scene;
  past: Scene[];
  future: Scene[];
  /** Snapshot taken at the start of an interactive gesture. */
  transaction: Scene | null;
  /** Incremented on every change; the bridge uses it to detect scene edits cheaply. */
  revision: number;
  dirty: boolean;

  loadScene(scene: Scene): void;
  update(mutate: (scene: Scene) => void, opts?: UpdateOptions): void;
  beginTransaction(): void;
  commitTransaction(): void;
  cancelTransaction(): void;
  undo(): void;
  redo(): void;
  markSaved(): void;

  setConditions(patch: Partial<FlowConditions>, opts?: UpdateOptions): void;
  addElement(kind: ElementKind, position: Vec2): string;
  addBody(geometry: BodyGeometry, name?: string, position?: Vec2): string;
  removeObjects(ids: string[]): void;
  duplicateObject(id: string): string | null;
  updateElement(id: string, fn: (e: SceneElement) => SceneElement, opts?: UpdateOptions): void;
  updateBody(id: string, fn: (b: SceneBody) => SceneBody, opts?: UpdateOptions): void;
  moveObject(id: string, position: Vec2, opts?: UpdateOptions): void;
  renameObject(id: string, name: string): void;
  setVisible(id: string, visible: boolean): void;
  setLocked(id: string, locked: boolean): void;
}

export const useSimulationStore = create<SimulationStore>((set, get) => {
  const commit = (next: Scene, opts?: UpdateOptions) => {
    const s = get();
    const recordHistory = !opts?.transient && s.transaction === null;
    set({
      scene: next,
      revision: s.revision + 1,
      dirty: true,
      past: recordHistory ? [...s.past.slice(-(HISTORY_LIMIT - 1)), s.scene] : s.past,
      future: recordHistory ? [] : s.future,
    });
  };

  return {
    scene: createScene(),
    past: [],
    future: [],
    transaction: null,
    revision: 0,
    dirty: false,

    loadScene(scene) {
      set((s) => ({ scene, past: [], future: [], transaction: null, revision: s.revision + 1, dirty: false }));
    },

    update(mutate, opts) {
      const next = cloneScene(get().scene);
      mutate(next);
      commit(next, opts);
    },

    beginTransaction() {
      if (get().transaction === null) set({ transaction: cloneScene(get().scene) });
    },

    commitTransaction() {
      const s = get();
      if (s.transaction === null) return;
      const changed = JSON.stringify(s.transaction) !== JSON.stringify(s.scene);
      set({
        transaction: null,
        past: changed ? [...s.past.slice(-(HISTORY_LIMIT - 1)), s.transaction] : s.past,
        future: changed ? [] : s.future,
      });
    },

    cancelTransaction() {
      const s = get();
      if (s.transaction === null) return;
      set({ scene: s.transaction, transaction: null, revision: s.revision + 1 });
    },

    undo() {
      const s = get();
      const prev = s.past[s.past.length - 1];
      if (!prev) return;
      set({
        scene: prev,
        past: s.past.slice(0, -1),
        future: [s.scene, ...s.future],
        revision: s.revision + 1,
        dirty: true,
        transaction: null,
      });
    },

    redo() {
      const s = get();
      const next = s.future[0];
      if (!next) return;
      set({
        scene: next,
        past: [...s.past, s.scene],
        future: s.future.slice(1),
        revision: s.revision + 1,
        dirty: true,
        transaction: null,
      });
    },

    markSaved() {
      set({ dirty: false });
    },

    setConditions(patch, opts) {
      get().update((sc) => {
        sc.conditions = { ...sc.conditions, ...patch };
      }, opts);
    },

    addElement(kind, position) {
      const el = createElement(get().scene, kind, position);
      get().update((sc) => {
        sc.elements.push(el);
      });
      return el.id;
    },

    addBody(geometry, name, position) {
      const body = createBody(get().scene, geometry, name, position);
      get().update((sc) => {
        sc.bodies.push(body);
      });
      return body.id;
    },

    removeObjects(ids) {
      const set_ = new Set(ids);
      get().update((sc) => {
        sc.elements = sc.elements.filter((e) => !set_.has(e.id));
        sc.bodies = sc.bodies.filter((b) => !set_.has(b.id));
      });
    },

    duplicateObject(id) {
      const sc = get().scene;
      const el = sc.elements.find((e) => e.id === id);
      if (el) {
        const copy: SceneElement = {
          ...el,
          id: newId('el'),
          name: nextName(sc, el.name.replace(/\s\d+$/, '')),
          element:
            el.element.type === 'uniformFlow'
              ? el.element
              : withElementPosition(el.element, { x: el.element.position.x + 0.25, y: el.element.position.y - 0.25 }),
        };
        get().update((s) => {
          s.elements.push(copy);
        });
        return copy.id;
      }
      const body = sc.bodies.find((b) => b.id === id);
      if (body) {
        const copy: SceneBody = {
          ...cloneScene({ ...sc, bodies: [body] }).bodies[0],
          id: newId('body'),
          name: nextName(sc, body.name.replace(/\s\d+$/, '')),
          position: { x: body.position.x + 0.25, y: body.position.y - 0.5 },
        };
        get().update((s) => {
          s.bodies.push(copy);
        });
        return copy.id;
      }
      return null;
    },

    updateElement(id, fn, opts) {
      get().update((sc) => {
        sc.elements = sc.elements.map((e) => (e.id === id ? fn(e) : e));
      }, opts);
    },

    updateBody(id, fn, opts) {
      get().update((sc) => {
        sc.bodies = sc.bodies.map((b) => (b.id === id ? fn(b) : b));
      }, opts);
    },

    moveObject(id, position, opts) {
      get().update((sc) => {
        sc.elements = sc.elements.map((e) =>
          e.id === id ? { ...e, element: withElementPosition(e.element, position) } : e,
        );
        sc.bodies = sc.bodies.map((b) => (b.id === id ? { ...b, position } : b));
      }, opts);
    },

    renameObject(id, name) {
      get().update((sc) => {
        sc.elements = sc.elements.map((e) => (e.id === id ? { ...e, name } : e));
        sc.bodies = sc.bodies.map((b) => (b.id === id ? { ...b, name } : b));
      });
    },

    setVisible(id, visible) {
      get().update((sc) => {
        sc.elements = sc.elements.map((e) => (e.id === id ? { ...e, visible } : e));
        sc.bodies = sc.bodies.map((b) => (b.id === id ? { ...b, visible } : b));
      });
    },

    setLocked(id, locked) {
      get().update((sc) => {
        sc.elements = sc.elements.map((e) => (e.id === id ? { ...e, locked } : e));
        sc.bodies = sc.bodies.map((b) => (b.id === id ? { ...b, locked } : b));
      });
    },
  };
});

export const selectCanUndo = (s: SimulationStore): boolean => s.past.length > 0;
export const selectCanRedo = (s: SimulationStore): boolean => s.future.length > 0;
