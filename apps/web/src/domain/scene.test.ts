import { describe, expect, it } from 'vitest';
import {
  createBody,
  createElement,
  createScene,
  defaultElement,
  nextName,
  parseScene,
  readParameter,
  serialiseScene,
  toDegrees,
  toRadians,
  writeParameter,
} from './scene';
import { BASE_FORMAT_VERSION, type Element } from './types';

describe('scene helpers', () => {
  it('creates a versioned scene with default conditions', () => {
    const s = createScene('Test');
    expect(s.version).toBe(BASE_FORMAT_VERSION);
    expect(s.conditions.density).toBeCloseTo(1.225);
    expect(s.elements).toEqual([]);
  });

  it('numbers new object names without collisions', () => {
    const s = createScene();
    expect(nextName(s, 'Source')).toBe('Source 01');
    s.elements.push(createElement(s, 'source', { x: 0, y: 0 }));
    expect(s.elements[0].name).toBe('Source 01');
    expect(nextName(s, 'Source')).toBe('Source 02');
  });

  it('gives a clockwise default vortex so new vortices lift', () => {
    const e = defaultElement('vortex', { x: 0, y: 0 }, createScene().conditions);
    expect(e.type).toBe('vortex');
    if (e.type === 'vortex') expect(e.circulation).toBeLessThan(0);
  });

  it('builds a unit cylinder doublet aligned with the freestream', () => {
    const c = { ...createScene().conditions, velocity: 3, angle: 0.4 };
    const e = defaultElement('doublet', { x: 0, y: 0 }, c);
    if (e.type !== 'doublet') throw new Error('wrong type');
    expect(e.strength).toBeCloseTo(2 * Math.PI * 3);
    expect(e.orientation).toBeCloseTo(0.4);
  });

  it('reads and writes descriptor parameter keys including positions', () => {
    const e: Element = { type: 'source', position: { x: 1, y: 2 }, strength: 5 };
    expect(readParameter(e, 'position.x')).toBe(1);
    expect(readParameter(e, 'strength')).toBe(5);
    const moved = writeParameter(e, 'position.y', 7);
    expect(moved.type === 'source' && moved.position.y).toBe(7);
    const stronger = writeParameter(e, 'strength', 9);
    expect(stronger.type === 'source' && stronger.strength).toBe(9);
    // Uniform flow has no position: writes are no-ops, never crashes.
    const u: Element = { type: 'uniformFlow', velocity: 1, direction: 0 };
    expect(writeParameter(u, 'position.x', 3)).toEqual(u);
  });

  it('round-trips through JSON', () => {
    const s = createScene('Round trip', { velocity: 4, angle: toRadians(5) });
    s.bodies.push(createBody(s, { kind: 'naca4', code: '2412', chord: 1 }));
    s.elements.push(createElement(s, 'vortex', { x: -1, y: 0.5 }));
    const back = parseScene(serialiseScene(s));
    expect(back).toEqual(s);
  });

  it('rejects files that are not simulations with readable errors', () => {
    expect(() => parseScene('not json')).toThrow(/valid JSON/);
    expect(() => parseScene('{"foo":1}')).toThrow(/format version/);
    expect(() => parseScene(JSON.stringify({ version: 99, conditions: { velocity: 1 } }))).toThrow(/format version 99/);
    expect(() => parseScene(JSON.stringify({ version: 1 }))).toThrow(/flow conditions/);
  });

  it('fills defaults for bodies saved by older minimal writers', () => {
    const s = parseScene(
      JSON.stringify({ version: 1, conditions: { velocity: 1, angle: 0, density: 1, pressure: 0 }, bodies: [{ id: 'b', name: 'B', geometry: { kind: 'circle', radius: 1 } }] }),
    );
    expect(s.bodies[0].scale).toBe(1);
    expect(s.bodies[0].visible).toBe(true);
    expect(s.bodies[0].circulation).toEqual({ mode: 'auto' });
  });

  it('converts degrees and radians consistently', () => {
    expect(toDegrees(toRadians(37.5))).toBeCloseTo(37.5, 10);
    expect(toRadians(180)).toBeCloseTo(Math.PI);
  });
});
