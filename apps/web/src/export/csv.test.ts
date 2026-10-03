import { describe, expect, it } from 'vitest';
import { fieldCsv, forcesCsv, geometryCsv, sweepCsv } from './csv';
import type { ScalarFieldResult, Solution } from '@/domain/types';

const solution: Solution = {
  status: 'success',
  bodies: [
    {
      id: 'b', name: 'Wing, A', index: 0, panelCount: 10, kuttaApplicable: true, trailingEdge: null, polygon: [{ x: 0, y: 0 }, { x: 1, y: 0 }, { x: 0.5, y: 0.1 }],
      circulationMode: { mode: 'kutta' }, vortexStrength: 0, netSourceOutflow: 0, notes: [], surface: [],
      forces: { fx: 1, fy: 2, lift: 2, drag: 1, moment: -0.5, cl: 0.6, cd: 0.001, cm: -0.05, circulation: -3, liftKuttaJoukowski: 2.01, liftConsistency: 0.001, reference: { chord: 1, area: 1, point: { x: 0.25, y: 0 } } },
    },
  ],
  total: { fx: 1, fy: 2, lift: 2, drag: 1, moment: -0.5, circulation: -3 },
  diagnostics: { panel: null, timings: { prepareMs: 0, assembleMs: 0, solveMs: 0, forcesMs: 0, totalMs: 0 }, systemReused: false, elementCount: 0, bodyCount: 1, totalCirculation: -3, netOutflow: 0 },
  warnings: [], error: null, assumptions: [],
  elements: [{ id: 'v', name: 'Vortex 01', position: { x: 1, y: 0 }, force: { x: 0, y: 3.675 }, lift: 3.675, drag: 0, externalVelocity: { x: 1, y: 0 } }],
};

describe('csv exporters', () => {
  it('writes forces with a quoted body name and a TOTAL row', () => {
    const csv = forcesCsv(solution);
    const lines = csv.trim().split('\n');
    expect(lines[0]).toMatch(/^body,lift_N_per_m/);
    expect(lines[1]).toContain('"Wing, A"');
    expect(lines[2]).toMatch(/^TOTAL,2,1,/);
    // Element (Lagally) section after a blank line.
    expect(lines[3]).toBe('');
    expect(lines[4]).toMatch(/^element,lift_N_per_m/);
    expect(lines[5]).toBe('"Vortex 01",3.675,0,0,3.675,1,0,1,0');
  });

  it('writes geometry as x,y rows', () => {
    expect(geometryCsv([{ x: 1, y: 2 }, { x: 3.5, y: -4 }])).toBe('x,y\n1,2\n3.5,-4\n');
  });

  it('writes field samples with coordinates and a mask column', () => {
    const f: ScalarFieldResult = { values: new Float32Array([1, 2, 3, NaN]), mask: new Uint8Array([0, 0, 0, 1]), bounds: { min: { x: 0, y: 0 }, max: { x: 1, y: 1 } }, nx: 2, ny: 2, min: 1, max: 3, robustMin: 1, robustMax: 3, softened: false, field: 'cp' };
    const lines = fieldCsv(f).trim().split('\n');
    expect(lines[0]).toBe('x,y,cp,mask');
    expect(lines[1]).toBe('0,0,1,0');
    expect(lines[4]).toBe('1,1,,1');
  });

  it('writes a polar with one column group per body', () => {
    const csv = sweepCsv([
      { index: 0, value: 0, status: 'success', totalLift: 1, totalDrag: 0, error: null, bodies: [{ id: 'b', name: 'W', lift: 1, drag: 0, moment: 0, cl: 0.1, cd: 0, cm: 0, circulation: -1 }] },
    ]);
    const lines = csv.trim().split('\n');
    expect(lines[0]).toBe('"alpha [deg]",solver_value,W_CL,W_CD,W_Cm,W_lift,W_circulation,total_lift,total_drag');
    expect(lines[1].startsWith('0,0,0.1,')).toBe(true);
  });
});
