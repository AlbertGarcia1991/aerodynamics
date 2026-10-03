/** Bundled example simulations (PRD §57, §81–§83). No upload required. */
import type { Scene, Vec2 } from './types';
import { createBody, createElement, createScene } from './scene';

export interface Example {
  id: string;
  title: string;
  description: string;
  /** What to look at first. */
  highlight: string;
  build(): Scene;
  view: { center: Vec2; halfWidth: number };
}

export const EXAMPLES: Example[] = [
  {
    id: 'cylinder',
    title: 'Flow around a cylinder',
    description: 'A unit cylinder in a 1 m/s stream, solved with the panel method. Symmetric Cp, zero lift, zero inviscid drag — the classical validation case.',
    highlight: 'Switch to Cp and watch the stagnation points at Cp = 1 and the crown at Cp = −3.',
    build() {
      const s = createScene('Flow around a cylinder', { velocity: 1, angle: 0, density: 1.225 });
      s.bodies.push(createBody(s, { kind: 'circle', radius: 1 }, 'Cylinder 01'));
      return s;
    },
    view: { center: { x: 0, y: 0 }, halfWidth: 3.2 },
  },
  {
    id: 'naca0012',
    title: 'NACA 0012 at 5°',
    description: 'The canonical panel-method demonstration: a symmetric airfoil at 10 m/s and 5° angle of attack with the Kutta condition applied.',
    highlight: 'Select the airfoil to read CL ≈ 0.60 and see the Cp distribution in the Surface tab.',
    build() {
      const s = createScene('NACA 0012 airfoil', { velocity: 10, angle: (5 * Math.PI) / 180, density: 1.225 });
      const b = createBody(s, { kind: 'naca4', code: '0012', chord: 1 }, 'NACA 0012');
      b.panels.count = 160;
      s.bodies.push(b);
      return s;
    },
    view: { center: { x: 0.5, y: 0 }, halfWidth: 1.6 },
  },
  {
    id: 'source-sink',
    title: 'Source and sink',
    description: 'A source and an equal sink in a freestream close into a Rankine oval — a solid body made from nothing but singularities.',
    highlight: 'Drag the sink and watch the oval reshape; the two stagnation points mark its nose and tail.',
    build() {
      const s = createScene('Source and sink', { velocity: 1, angle: 0 });
      const src = createElement(s, 'source', { x: -1, y: 0 });
      src.element = { type: 'source', position: { x: -1, y: 0 }, strength: 2 };
      s.elements.push(src);
      const snk = createElement(s, 'sink', { x: 1, y: 0 });
      snk.element = { type: 'sink', position: { x: 1, y: 0 }, strength: 2 };
      s.elements.push(snk);
      return s;
    },
    view: { center: { x: 0, y: 0 }, halfWidth: 3.5 },
  },
  {
    id: 'vortex',
    title: 'Vortex in a freestream',
    description: 'A clockwise point vortex superposed on a uniform stream: the flow speeds up above the vortex and stalls below it.',
    highlight: 'Switch to the Vorticity view — the fluid is irrotational everywhere except the vortex core.',
    build() {
      const s = createScene('Vortex and freestream', { velocity: 1, angle: 0 });
      const v = createElement(s, 'vortex', { x: 0, y: 0 });
      v.element = { type: 'vortex', position: { x: 0, y: 0 }, circulation: -3 };
      s.elements.push(v);
      return s;
    },
    view: { center: { x: 0, y: 0 }, halfWidth: 3 },
  },
  {
    id: 'uniform',
    title: 'Uniform flow',
    description: 'Just the freestream. Add elements to see superposition at work.',
    highlight: 'Use + Add to place a source; the stagnation point appears upstream of it.',
    build() {
      return createScene('Uniform flow', { velocity: 10, angle: 0 });
    },
    view: { center: { x: 0, y: 0 }, halfWidth: 3 },
  },
  {
    id: 'magnus',
    title: 'Spinning cylinder (Magnus effect)',
    description: 'A cylinder with prescribed clockwise circulation Γ = −4 m²/s: the stagnation points move down and lift appears with no change of shape. The panel solution matches L = −ρU∞Γ to within half a percent.',
    highlight: 'Compare the reported lift with the Kutta–Joukowski value in the Results panel.',
    build() {
      const s = createScene('Spinning cylinder', { velocity: 1, angle: 0, density: 1.225 });
      const b = createBody(s, { kind: 'circle', radius: 1 }, 'Spinning cylinder');
      b.circulation = { mode: 'prescribed', circulation: -4 };
      b.panels.count = 160;
      s.bodies.push(b);
      return s;
    },
    view: { center: { x: 0, y: 0 }, halfWidth: 3.2 },
  },
  {
    id: 'doublet',
    title: 'Doublet cylinder (analytic)',
    description: 'A doublet aligned with the stream reproduces the cylinder flow in closed form — no panels, no linear system.',
    highlight: 'Compare with the panel-method cylinder: the fields are identical outside r = 1.',
    build() {
      const s = createScene('Doublet cylinder', { velocity: 1, angle: 0 });
      const d = createElement(s, 'doublet', { x: 0, y: 0 });
      d.element = { type: 'doublet', position: { x: 0, y: 0 }, strength: 2 * Math.PI, orientation: 0 };
      s.elements.push(d);
      return s;
    },
    view: { center: { x: 0, y: 0 }, halfWidth: 3.2 },
  },
  {
    id: 'biplane',
    title: 'Biplane: two NACA 0012 wings',
    description: "Two lifting airfoils one chord apart, solved as one coupled system. Each loses lift to the other's downwash.",
    highlight: 'Select each wing: the lower one keeps more of its lift than the upper one.',
    build() {
      const s = createScene('Biplane', { velocity: 10, angle: (4 * Math.PI) / 180 });
      const upper = createBody(s, { kind: 'naca4', code: '0012', chord: 1 }, 'Upper wing');
      upper.position = { x: 0, y: 0.5 };
      const lower = createBody(s, { kind: 'naca4', code: '0012', chord: 1 }, 'Lower wing');
      lower.position = { x: 0, y: -0.5 };
      // Interference loads the wings unevenly; 160 panels keeps the pressure and
      // Kutta–Joukowski lift within the 1 % consistency threshold.
      upper.panels.count = 160;
      lower.panels.count = 160;
      s.bodies.push(upper, lower);
      return s;
    },
    view: { center: { x: 0.5, y: 0 }, halfWidth: 1.8 },
  },
  {
    id: 'half-body',
    title: 'Rankine half-body',
    description: 'A single source in a freestream makes an open body extending downstream forever.',
    highlight: 'The dividing streamline passes through the stagnation point at x = −Λ/(2πU).',
    build() {
      const s = createScene('Rankine half-body', { velocity: 1, angle: 0 });
      const src = createElement(s, 'source', { x: 0, y: 0 });
      src.element = { type: 'source', position: { x: 0, y: 0 }, strength: 3 };
      s.elements.push(src);
      return s;
    },
    view: { center: { x: 1, y: 0 }, halfWidth: 3.5 },
  },
  {
    id: 'naca2412',
    title: 'Cambered NACA 2412',
    description: 'A cambered section at zero angle of attack still lifts (CL ≈ 0.25) and carries a nose-down moment about the quarter chord.',
    highlight: 'Run an angle-of-attack sweep in the Polar tab to find the zero-lift angle near −2°.',
    build() {
      const s = createScene('NACA 2412', { velocity: 10, angle: 0 });
      const b = createBody(s, { kind: 'naca4', code: '2412', chord: 1 }, 'NACA 2412');
      b.panels.count = 160;
      s.bodies.push(b);
      return s;
    },
    view: { center: { x: 0.5, y: 0 }, halfWidth: 1.6 },
  },
];

export function findExample(id: string): Example | undefined {
  return EXAMPLES.find((e) => e.id === id);
}
