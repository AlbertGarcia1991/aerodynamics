/**
 * Scene construction helpers and small pure utilities over the domain model.
 * Everything here is deterministic and side-effect free so it can be unit
 * tested without a browser.
 */
import type {
  BezierGeometry,
  BodyGeometry,
  Element,
  ElementKind,
  FlowConditions,
  Scene,
  SceneBody,
  SceneElement,
  SceneObject,
  Vec2,
} from './types';
import { BASE_FORMAT_VERSION, SCENE_FORMAT_VERSION } from './types';

export const DEG = Math.PI / 180;
export const toDegrees = (rad: number): number => rad / DEG;
export const toRadians = (deg: number): number => deg * DEG;

export const vec2 = (x: number, y: number): Vec2 => ({ x, y });
export const vadd = (a: Vec2, b: Vec2): Vec2 => ({ x: a.x + b.x, y: a.y + b.y });
export const vsub = (a: Vec2, b: Vec2): Vec2 => ({ x: a.x - b.x, y: a.y - b.y });
export const vscale = (a: Vec2, s: number): Vec2 => ({ x: a.x * s, y: a.y * s });
export const vlen = (a: Vec2): number => Math.hypot(a.x, a.y);
export const vdist = (a: Vec2, b: Vec2): number => Math.hypot(a.x - b.x, a.y - b.y);
export const vrotate = (a: Vec2, angle: number): Vec2 => {
  const c = Math.cos(angle);
  const s = Math.sin(angle);
  return { x: a.x * c - a.y * s, y: a.x * s + a.y * c };
};

let counter = 0;
/** Short unique id; `crypto.randomUUID` where available, a counter fallback in tests. */
export function newId(prefix = 'obj'): string {
  const rnd =
    typeof crypto !== 'undefined' && 'randomUUID' in crypto
      ? crypto.randomUUID().slice(0, 8)
      : (++counter).toString(36).padStart(6, '0');
  return `${prefix}_${rnd}`;
}

export const DEFAULT_CONDITIONS: FlowConditions = {
  velocity: 10,
  angle: 0,
  density: 1.225,
  pressure: 101325,
};

export function createScene(name = 'Untitled simulation', conditions: Partial<FlowConditions> = {}): Scene {
  return {
    version: BASE_FORMAT_VERSION,
    name,
    conditions: { ...DEFAULT_CONDITIONS, ...conditions },
    elements: [],
    bodies: [],
  };
}

/** Human-readable default names: "Source 01", "Vortex 02", … */
export const ELEMENT_LABELS: Record<ElementKind, string> = {
  uniformFlow: 'Uniform Flow',
  source: 'Source',
  sink: 'Sink',
  vortex: 'Vortex',
  doublet: 'Doublet',
};

export function nextName(scene: Scene, base: string): string {
  const taken = new Set([...scene.elements, ...scene.bodies].map((o) => o.name));
  for (let i = 1; i < 1000; i++) {
    const candidate = `${base} ${String(i).padStart(2, '0')}`;
    if (!taken.has(candidate)) return candidate;
  }
  return `${base} ${Date.now()}`;
}

/** Sensible default parameters relative to the freestream so new objects are visible. */
export function defaultElement(kind: ElementKind, position: Vec2, conditions: FlowConditions): Element {
  const u = Math.max(Math.abs(conditions.velocity), 1);
  switch (kind) {
    case 'uniformFlow':
      return { type: 'uniformFlow', velocity: u * 0.5, direction: Math.PI / 2 };
    case 'source':
      return { type: 'source', position, strength: 2 * u };
    case 'sink':
      return { type: 'sink', position, strength: 2 * u };
    case 'vortex':
      // Clockwise by default: the lifting sense under the Γ > 0 CCW convention.
      return { type: 'vortex', position, circulation: -2 * u };
    case 'doublet':
      // A unit-radius cylinder when aligned with the freestream.
      return { type: 'doublet', position, strength: 2 * Math.PI * u * 1, orientation: conditions.angle };
  }
}

export function createElement(scene: Scene, kind: ElementKind, position: Vec2): SceneElement {
  return {
    id: newId('el'),
    name: nextName(scene, ELEMENT_LABELS[kind]),
    visible: true,
    locked: false,
    element: defaultElement(kind, position, scene.conditions),
  };
}

export function createBody(scene: Scene, geometry: BodyGeometry, name?: string, position: Vec2 = vec2(0, 0)): SceneBody {
  const base =
    name ??
    (geometry.kind === 'naca4'
      ? `NACA ${geometry.code}`
      : geometry.kind === 'circle'
        ? 'Cylinder'
        : geometry.kind === 'ellipse'
          ? 'Ellipse'
          : geometry.kind === 'joukowski'
            ? 'Joukowski'
            : geometry.kind === 'bezier'
              ? 'Custom body'
              : 'Body');
  return {
    id: newId('body'),
    name: name ?? nextName(scene, base),
    visible: true,
    locked: false,
    geometry,
    position,
    rotation: 0,
    scale: 1,
    circulation: { mode: 'auto' },
    panels: { count: 120, distribution: 'auto' },
    reference: {},
    sourceName: null,
  };
}

export function elementPosition(e: Element): Vec2 | null {
  return e.type === 'uniformFlow' ? null : e.position;
}

export function withElementPosition(e: Element, position: Vec2): Element {
  return e.type === 'uniformFlow' ? e : { ...e, position };
}

/** The element's primary magnitude (Λ, Γ, κ or U). */
export function elementStrength(e: Element): number {
  switch (e.type) {
    case 'uniformFlow':
      return e.velocity;
    case 'vortex':
      return e.circulation;
    default:
      return e.strength;
  }
}

/** Read a descriptor `key` such as `strength` or `position.x`. */
export function readParameter(e: Element, key: string): number {
  if (key === 'position.x') return elementPosition(e)?.x ?? 0;
  if (key === 'position.y') return elementPosition(e)?.y ?? 0;
  const v = (e as unknown as Record<string, unknown>)[key];
  return typeof v === 'number' ? v : 0;
}

export function writeParameter(e: Element, key: string, value: number): Element {
  if (key === 'position.x') {
    const p = elementPosition(e);
    return p ? withElementPosition(e, { x: value, y: p.y }) : e;
  }
  if (key === 'position.y') {
    const p = elementPosition(e);
    return p ? withElementPosition(e, { x: p.x, y: value }) : e;
  }
  return { ...e, [key]: value } as Element;
}

export function findObject(scene: Scene, id: string): SceneObject | null {
  const el = scene.elements.find((e) => e.id === id);
  if (el) return { kind: 'element', object: el };
  const body = scene.bodies.find((b) => b.id === id);
  if (body) return { kind: 'body', object: body };
  return null;
}

/** A content-based key for the objects that affect the panel system — mirrors the Rust geometry hash. */
export function sceneSignature(scene: Scene): string {
  return JSON.stringify(scene);
}

export function cloneScene(scene: Scene): Scene {
  return typeof structuredClone === 'function' ? structuredClone(scene) : JSON.parse(JSON.stringify(scene));
}

/**
 * Parse a `.aeroflow.json` document. Throws with a readable message on
 * structural problems; numerical validation happens in the solver.
 */
export function parseScene(text: string): Scene {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch (e) {
    throw new Error(`The file is not valid JSON: ${(e as Error).message}`);
  }
  if (typeof raw !== 'object' || raw === null) throw new Error('The file does not contain a simulation object.');
  const obj = raw as Partial<Scene> & { version?: number };
  if (typeof obj.version !== 'number') throw new Error('The file has no format version; it is not an AeroFlow simulation.');
  if (obj.version > SCENE_FORMAT_VERSION) {
    throw new Error(`This file uses format version ${obj.version}; this build understands up to ${SCENE_FORMAT_VERSION}.`);
  }
  if (!obj.conditions || typeof obj.conditions.velocity !== 'number') {
    throw new Error('The file has no flow conditions.');
  }
  return {
    version: obj.version,
    name: typeof obj.name === 'string' ? obj.name : 'Imported simulation',
    conditions: { ...DEFAULT_CONDITIONS, ...obj.conditions },
    elements: Array.isArray(obj.elements) ? obj.elements : [],
    bodies: Array.isArray(obj.bodies) ? obj.bodies.map(normaliseBody) : [],
  };
}

const finiteVec = (v: unknown): Vec2 | null => {
  const p = v as Partial<Vec2> | null | undefined;
  return p && typeof p.x === 'number' && typeof p.y === 'number' && Number.isFinite(p.x) && Number.isFinite(p.y) ? { x: p.x, y: p.y } : null;
};

/** Untrusted file → well-formed Bézier geometry, or a readable error (never a crash later). */
function normaliseBezier(g: BezierGeometry, body: string): BezierGeometry {
  if (!Array.isArray(g.nodes)) throw new Error(`Body “${body}” has Bézier geometry but no node list.`);
  const nodes = g.nodes.map((n, i) => {
    const position = finiteVec(n?.position);
    if (!position) throw new Error(`Body “${body}”: node ${i + 1} has no valid position.`);
    return {
      id: typeof n.id === 'string' && n.id ? n.id : newId('node'),
      position,
      inHandle: finiteVec(n.inHandle) ?? vec2(0, 0),
      outHandle: finiteVec(n.outHandle) ?? vec2(0, 0),
      nodeType: n.nodeType === 'smooth' || n.nodeType === 'symmetric' ? n.nodeType : ('corner' as const),
    };
  });
  return { kind: 'bezier', closed: g.closed !== false, nodes, fitError: typeof g.fitError === 'number' ? g.fitError : null };
}

function normaliseBody(b: SceneBody): SceneBody {
  return {
    ...b,
    geometry: b.geometry?.kind === 'bezier' ? normaliseBezier(b.geometry, b.name ?? 'Body') : b.geometry,
    visible: b.visible ?? true,
    locked: b.locked ?? false,
    position: b.position ?? vec2(0, 0),
    rotation: b.rotation ?? 0,
    scale: b.scale ?? 1,
    circulation: b.circulation ?? { mode: 'auto' },
    panels: b.panels ?? { count: 120, distribution: 'auto' },
    reference: b.reference ?? {},
  };
}

/** Version follows content: only scenes that need Bézier support are written as v2. */
export function serialiseScene(scene: Scene): string {
  const version = scene.bodies.some((b) => b.geometry.kind === 'bezier') ? SCENE_FORMAT_VERSION : BASE_FORMAT_VERSION;
  return JSON.stringify({ ...scene, version }, null, 2);
}

/** World-space bounding box of everything in the scene, for fit-to-view. */
export function sceneExtent(scene: Scene, bodyPolygons: Map<string, Vec2[]>): { min: Vec2; max: Vec2 } | null {
  let min = vec2(Infinity, Infinity);
  let max = vec2(-Infinity, -Infinity);
  let any = false;
  const take = (p: Vec2) => {
    any = true;
    min = vec2(Math.min(min.x, p.x), Math.min(min.y, p.y));
    max = vec2(Math.max(max.x, p.x), Math.max(max.y, p.y));
  };
  for (const e of scene.elements) {
    const p = elementPosition(e.element);
    if (p) take(p);
  }
  for (const b of scene.bodies) {
    const poly = bodyPolygons.get(b.id);
    if (poly) poly.forEach(take);
    else take(b.position);
  }
  return any ? { min, max } : null;
}
