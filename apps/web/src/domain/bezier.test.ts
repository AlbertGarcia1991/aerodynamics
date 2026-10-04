import { describe, expect, it } from 'vitest';
import {
  appendNode,
  bodyToWorld,
  cubicAt,
  cubicDerivative,
  deleteNodes,
  fitPoints,
  insertNodeAt,
  mirrorGeometry,
  moveNodes,
  nacaPoints,
  nearestOnCurve,
  polygonSelfIntersections,
  sampleBezier,
  segmentCubic,
  setClosed,
  setNodeHandle,
  setNodeType,
  signedArea,
  splitCubic,
  templateGeometry,
  toSolverScene,
  validateBezier,
  worldToBody,
  emptyBezier,
  makeNode,
  type Cubic,
} from './bezier';
import { createBody, createScene, parseScene, serialiseScene } from './scene';
import type { BezierGeometry, Vec2 } from './types';

const C: Cubic = [{ x: 0, y: 0 }, { x: 1, y: 2 }, { x: 3, y: 2 }, { x: 4, y: 0 }];
const close = (a: Vec2, b: Vec2, tol = 1e-12) => {
  expect(a.x).toBeCloseTo(b.x, -Math.log10(tol));
  expect(a.y).toBeCloseTo(b.y, -Math.log10(tol));
};
const len = (v: Vec2) => Math.hypot(v.x, v.y);
const cross = (a: Vec2, b: Vec2) => a.x * b.y - a.y * b.x;

describe('cubic maths', () => {
  it('GEOM-TEST-001: evaluates endpoints exactly', () => {
    close(cubicAt(C, 0), C[0]);
    close(cubicAt(C, 1), C[3]);
    close(cubicAt(C, 0.5), { x: 2, y: 1.5 });
  });

  it('GEOM-TEST-002: the tangent at the ends points at the control points', () => {
    close(cubicDerivative(C, 0), { x: 3, y: 6 });
    close(cubicDerivative(C, 1), { x: 3, y: -6 });
  });

  it('GEOM-TEST-003: subdivision reproduces the original curve on both halves', () => {
    const [a, b] = splitCubic(C, 0.3);
    for (const k of [0, 0.25, 0.6, 1]) {
      close(cubicAt(a, k), cubicAt(C, 0.3 * k));
      close(cubicAt(b, k), cubicAt(C, 0.3 + 0.7 * k));
    }
  });
});

const circle = () => templateGeometry('circle');

describe('node editing', () => {
  it('GEOM-TEST-004: inserting a node leaves the curve unchanged', () => {
    const g = templateGeometry('naca2412');
    const before = sampleBezier(g, 2000);
    const { geometry } = insertNodeAt(g, 2, 0.37);
    expect(geometry.nodes.length).toBe(g.nodes.length + 1);
    const nearest = (p: Vec2) => nearestOnCurve(geometry, p)!.distance;
    for (const p of before.filter((_, i) => i % 40 === 0)) expect(nearest(p)).toBeLessThan(1e-9);
  });

  it('inserting at the closing segment appends and keeps the loop closed', () => {
    const g = circle();
    const { geometry } = insertNodeAt(g, g.nodes.length - 1, 0.5);
    expect(geometry.nodes.length).toBe(5);
    expect(geometry.closed).toBe(true);
    expect(validateBezier(geometry).status).toBe('ok');
  });

  it('GEOM-TEST-005: deletion keeps a valid topology and refuses to go below the minimum', () => {
    let g: BezierGeometry = templateGeometry('circle');
    g = deleteNodes(g, [g.nodes[0].id])!;
    expect(g.nodes.length).toBe(3);
    expect(deleteNodes(g, [g.nodes[0].id])).toBeNull();
    const open = setClosed(g, false);
    expect(deleteNodes(open, [open.nodes[0].id])?.nodes.length).toBe(2);
    expect(deleteNodes(open, [open.nodes[0].id, open.nodes[1].id])).toBeNull();
  });

  it('GEOM-TEST-006: dragging a smooth node handle keeps the tangent continuous', () => {
    const g = circle();
    const n = g.nodes[0];
    const moved = setNodeHandle(g, n.id, 'out', { x: 0.2, y: 0.5 }).nodes[0];
    expect(Math.abs(cross(moved.outHandle, moved.inHandle))).toBeLessThan(1e-12);
    expect(len(moved.inHandle)).toBeCloseTo(len(n.inHandle)); // other handle keeps its length
  });

  it('symmetric nodes mirror the other handle fully', () => {
    const base = circle();
    const g = setNodeType(base, base.nodes[0].id, 'symmetric');
    const n = setNodeHandle(g, g.nodes[0].id, 'in', { x: 0.1, y: -0.3 }).nodes[0];
    close(n.outHandle, { x: -0.1, y: 0.3 });
  });

  it('GEOM-TEST-007: a corner node allows a tangent discontinuity', () => {
    const base = circle();
    const g = setNodeType(base, base.nodes[0].id, 'corner');
    const n = setNodeHandle(g, g.nodes[0].id, 'out', { x: 0.5, y: 0.1 }).nodes[0];
    expect(Math.abs(cross(n.outHandle, n.inHandle))).toBeGreaterThan(0.01);
  });

  it('converting a corner with no handles to smooth gives it tangent-continuous handles', () => {
    const g = templateGeometry('roundedRect');
    const corner = g.nodes[0]; // arc start: straight in, curved out
    const s = setNodeType(g, corner.id, 'smooth').nodes[0];
    expect(len(s.inHandle)).toBeGreaterThan(0);
    expect(Math.abs(cross(s.inHandle, s.outHandle))).toBeLessThan(1e-9);
  });

  it('moves nodes with their handles', () => {
    const g = circle();
    const m = moveNodes(g, [g.nodes[1].id], { x: 0.1, y: 0.2 });
    close(m.nodes[1].position, { x: 0.1, y: 0.7 });
    close(m.nodes[1].outHandle, g.nodes[1].outHandle);
  });

  it('GEOM-TEST-008: a closed curve closes on its start and sampling does not repeat it', () => {
    const g = circle();
    close(segmentCubic(g, g.nodes.length - 1)[3], g.nodes[0].position);
    const pts = sampleBezier(g);
    expect(Math.hypot(pts[0].x - pts[pts.length - 1].x, pts[0].y - pts[pts.length - 1].y)).toBeGreaterThan(1e-6);
    expect(Math.abs(signedArea(pts)) / (Math.PI * 0.25)).toBeCloseTo(1, 3);
  });

  it('closing onto a coincident last node merges it into the first', () => {
    let g = emptyBezier();
    for (const p of [[0, 0], [1, 0], [1, 1], [0, 0]]) g = appendNode(g, { x: p[0], y: p[1] }).geometry;
    const closed = setClosed(g, true);
    expect(closed.nodes.length).toBe(3);
    expect(validateBezier(closed).status).toBe('ok');
  });

  it('the pen drag creates a symmetric node with equal and opposite handles', () => {
    const { geometry } = appendNode(emptyBezier(), { x: 1, y: 1 }, { x: 0.3, y: 0 });
    close(geometry.nodes[0].inHandle, { x: -0.3, y: 0 });
    expect(geometry.nodes[0].nodeType).toBe('symmetric');
  });
});

describe('transforms', () => {
  it('GEOM-TEST-009: body↔world transforms are inverse and keep distances up to scale', () => {
    const b = { position: { x: 2, y: -1 }, rotation: 0.7, scale: 2 };
    const p = { x: 0.3, y: 0.4 };
    close(worldToBody(b, bodyToWorld(b, p)), p);
    const q = { x: -0.2, y: 0.9 };
    expect(len({ x: bodyToWorld(b, p).x - bodyToWorld(b, q).x, y: bodyToWorld(b, p).y - bodyToWorld(b, q).y })).toBeCloseTo(2 * Math.hypot(p.x - q.x, p.y - q.y));
  });

  it('mirroring flips the shape in place and preserves winding and area', () => {
    const g = templateGeometry('naca2412');
    const m = mirrorGeometry(g, 'horizontal');
    const a = sampleBezier(g);
    const b = sampleBezier(m);
    expect(Math.sign(signedArea(a))).toBe(Math.sign(signedArea(b)));
    expect(signedArea(b)).toBeCloseTo(signedArea(a), 6);
    const ya = a.map((p) => p.y);
    const yb = b.map((p) => p.y);
    expect(Math.min(...yb) + Math.max(...yb)).toBeCloseTo(Math.min(...ya) + Math.max(...ya), 4);
    expect(validateBezier(m).status).toBe('ok');
  });
});

describe('sampling and the solver mapping', () => {
  it('GEOM-TEST-010: sampling is deterministic', () => {
    const g = templateGeometry('naca0012');
    expect(sampleBezier(g)).toEqual(sampleBezier(g));
  });

  it('replaces Bézier bodies with sampled points and the v1 solver format', () => {
    const scene = createScene();
    const body = createBody(scene, circle());
    scene.bodies.push(body);
    const out = toSolverScene(scene);
    expect(out.version).toBe(1);
    expect(out.bodies[0].geometry.kind).toBe('points');
    expect(scene.bodies[0].geometry.kind).toBe('bezier'); // the source is never modified
  });

  it('leaves open and invalid bodies out of the solve instead of solving something else', () => {
    const scene = createScene();
    scene.bodies.push(createBody(scene, setClosed(circle(), false)));
    const bow = circle();
    scene.bodies.push(createBody(scene, { ...bow, nodes: [bow.nodes[0], bow.nodes[2], bow.nodes[1], bow.nodes[3]] }));
    expect(toSolverScene(scene).bodies).toHaveLength(0);
  });

  it('caps the panel count only for the preview pass', () => {
    const scene = createScene();
    scene.bodies.push(createBody(scene, circle()));
    scene.bodies[0].panels = { count: 256, distribution: 'cosine' };
    expect(toSolverScene(scene).bodies[0].panels.count).toBe(256);
    expect(toSolverScene(scene, 64).bodies[0].panels.count).toBe(64);
  });
});

describe('validation', () => {
  it('accepts every template that has an area', () => {
    for (const t of ['circle', 'ellipse', 'naca0012', 'naca2412', 'flatPlate', 'roundedPlate', 'roundedRect'] as const) {
      expect(validateBezier(templateGeometry(t)).status, t).toBe('ok');
    }
  });

  it('flags a figure-eight as a self-intersection and reports where', () => {
    const c = circle();
    const bow = { ...c, nodes: [c.nodes[0], c.nodes[2], c.nodes[1], c.nodes[3]] };
    const v = validateBezier(bow);
    expect(v.status).toBe('invalid');
    expect(v.intersections.length).toBeGreaterThan(0);
  });

  it('never throws on NaN or tiny geometries', () => {
    const nan = { ...circle(), nodes: circle().nodes.map((n, i) => (i ? n : { ...n, position: { x: NaN, y: 0 } })) };
    expect(validateBezier(nan).status).toBe('invalid');
    expect(validateBezier(emptyBezier()).status).toBe('open');
    expect(validateBezier({ kind: 'bezier', closed: true, nodes: [makeNode({ x: 0, y: 0 }), makeNode({ x: 1, y: 0 })] }).status).toBe('invalid');
  });

  it('polygonSelfIntersections ignores shared vertices', () => {
    expect(polygonSelfIntersections([{ x: 0, y: 0 }, { x: 1, y: 0 }, { x: 1, y: 1 }, { x: 0, y: 1 }], true)).toHaveLength(0);
  });
});

describe('fitting coordinates', () => {
  it('fits a circle within tolerance using few nodes', () => {
    const pts = Array.from({ length: 200 }, (_, i) => ({ x: Math.cos((2 * Math.PI * i) / 200), y: Math.sin((2 * Math.PI * i) / 200) }));
    const { geometry, maxError } = fitPoints(pts, 1e-3);
    expect(maxError).toBeLessThanOrEqual(1e-3 * 1.0001);
    expect(geometry.nodes.length).toBeLessThanOrEqual(12);
    expect(geometry.nodes.every((n) => n.nodeType === 'smooth')).toBe(true);
  });

  it('keeps a sharp trailing edge as a corner node and fits an airfoil closely', () => {
    const pts = nacaPoints('2412', 100);
    const { geometry, maxError } = fitPoints(pts, 2e-4);
    expect(maxError).toBeLessThan(2.1e-4);
    const te = geometry.nodes.reduce((a, n) => (n.position.x > a.position.x ? n : a));
    expect(te.nodeType).toBe('corner');
    expect(geometry.nodes.length).toBeLessThan(40);
    // Fit error reported against the source really is the worst distance.
    const worst = Math.max(...pts.map((p) => nearestOnCurve(geometry, p)!.distance));
    expect(worst).toBeLessThan(maxError + 1e-4);
  });

  it('a tighter tolerance never needs fewer nodes', () => {
    const pts = nacaPoints('4415', 80);
    expect(fitPoints(pts, 1e-4).geometry.nodes.length).toBeGreaterThanOrEqual(fitPoints(pts, 5e-3).geometry.nodes.length);
  });

  it('rejects fewer than three distinct points', () => {
    expect(() => fitPoints([{ x: 0, y: 0 }, { x: 0, y: 0 }, { x: 1, y: 1 }], 1e-3)).toThrow();
  });
});

describe('save and load (FILE-001, FILE-002, FILE-006)', () => {
  it('round-trips control points exactly and writes format v2 only when a Bézier body is present', () => {
    const scene = createScene();
    expect(JSON.parse(serialiseScene(scene)).version).toBe(1);
    const body = createBody(scene, templateGeometry('naca2412'));
    body.position = { x: 1.5, y: -0.25 };
    body.rotation = 0.3;
    scene.bodies.push(body);
    const text = serialiseScene(scene);
    expect(JSON.parse(text).version).toBe(2);
    const loaded = parseScene(text);
    expect(loaded.bodies[0].geometry).toEqual(body.geometry);
    expect(loaded.bodies[0].position).toEqual(body.position);
  });

  it('rejects a Bézier body without a node list, or with a bad node, with a readable message', () => {
    const scene = createScene();
    scene.bodies.push(createBody(scene, circle()));
    const doc = JSON.parse(serialiseScene(scene));
    doc.bodies[0].geometry = { kind: 'bezier', closed: true };
    expect(() => parseScene(JSON.stringify(doc))).toThrow(/no node list/);
    doc.bodies[0].geometry = { kind: 'bezier', closed: true, nodes: [{ position: { x: 'a', y: 0 } }] };
    expect(() => parseScene(JSON.stringify(doc))).toThrow(/node 1/);
  });

  it('fills in missing handles and types instead of failing', () => {
    const scene = createScene();
    scene.bodies.push(createBody(scene, circle()));
    const doc = JSON.parse(serialiseScene(scene));
    doc.bodies[0].geometry.nodes = doc.bodies[0].geometry.nodes.map((n: any) => ({ position: n.position }));
    const g = parseScene(JSON.stringify(doc)).bodies[0].geometry;
    expect(g.kind === 'bezier' && g.nodes.every((n) => n.nodeType === 'corner' && n.id)).toBe(true);
  });
});
