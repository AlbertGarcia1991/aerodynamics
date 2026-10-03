/** CSV builders for the export dialog (PRD §52). All numbers in SI units. */
import type { BodyResult, FlowConditions, ScalarFieldResult, Solution, SweepPoint } from '@/domain/types';
import { toDegrees } from '@/domain/scene';

const n = (v: number | null | undefined, digits = 9): string => (v === null || v === undefined || !Number.isFinite(v) ? '' : Number(v.toPrecision(digits)).toString());

export function surfaceCsv(body: BodyResult, c: FlowConditions): string {
  const rows = ['panel,x,y,x_over_c,surface,arc_length,panel_length,nx,ny,V_t,V_n,V_over_Uinf,p,Cp,sigma'];
  const u = c.velocity || NaN;
  body.surface.forEach((s, i) => {
    rows.push(
      [
        i + 1, n(s.position.x), n(s.position.y), n(s.xOverC), s.surface, n(s.arcLength), n(s.panelLength), n(s.normal.x), n(s.normal.y),
        n(s.tangentialVelocity), n(s.normalVelocity), n(Math.abs(s.tangentialVelocity) / u), n(s.pressure), n(s.cp), n(s.sourceStrength),
      ].join(','),
    );
  });
  return rows.join('\n') + '\n';
}

export function forcesCsv(sol: Solution): string {
  const rows = ['body,lift_N_per_m,drag_N_per_m,Fx_N_per_m,Fy_N_per_m,moment_Nm_per_m,CL,CD,Cm,circulation_m2_per_s,lift_kutta_joukowski_N_per_m,reference_chord_m,panels,circulation_mode'];
  for (const b of sol.bodies) {
    const f = b.forces;
    rows.push([
      JSON.stringify(b.name), n(f.lift), n(f.drag), n(f.fx), n(f.fy), n(f.moment), n(f.cl), n(f.cd), n(f.cm), n(f.circulation), n(f.liftKuttaJoukowski), n(f.reference.chord), b.panelCount, b.circulationMode.mode,
    ].join(','));
  }
  const t = sol.total;
  rows.push(['TOTAL', n(t.lift), n(t.drag), n(t.fx), n(t.fy), n(t.moment), '', '', '', n(t.circulation), '', '', '', ''].join(','));
  return rows.join('\n') + '\n';
}

/**
 * Sweep results, one column group per body. `axis` labels the swept parameter
 * and `fromSolver` converts its stored value to display units (α: rad → deg).
 */
export function sweepCsv(points: SweepPoint[], axis = 'alpha [deg]', fromSolver: (v: number) => number = toDegrees): string {
  const bodies = points[0]?.bodies ?? [];
  const head = [JSON.stringify(axis), 'solver_value'];
  for (const b of bodies) head.push(`${b.name}_CL`, `${b.name}_CD`, `${b.name}_Cm`, `${b.name}_lift`, `${b.name}_circulation`);
  head.push('total_lift', 'total_drag');
  const rows = [head.join(',')];
  for (const p of points) {
    const r = [n(fromSolver(p.value)), n(p.value)];
    for (const b of p.bodies) r.push(n(b.cl), n(b.cd), n(b.cm), n(b.lift), n(b.circulation));
    r.push(n(p.totalLift), n(p.totalDrag));
    rows.push(r.join(','));
  }
  return rows.join('\n') + '\n';
}

export function fieldCsv(f: ScalarFieldResult): string {
  // mask: 0 = fluid, 1 = inside a body, 2 = near-wall band (value extrapolated from neighbours)
  const rows = [`x,y,${f.field},mask`];
  const dx = (f.bounds.max.x - f.bounds.min.x) / (f.nx - 1);
  const dy = (f.bounds.max.y - f.bounds.min.y) / (f.ny - 1);
  for (let j = 0; j < f.ny; j++) {
    for (let i = 0; i < f.nx; i++) {
      const k = j * f.nx + i;
      rows.push(`${n(f.bounds.min.x + i * dx)},${n(f.bounds.min.y + j * dy)},${f.mask[k] === 1 ? '' : n(f.values[k])},${f.mask[k]}`);
    }
  }
  return rows.join('\n') + '\n';
}

export function geometryCsv(points: { x: number; y: number }[]): string {
  return 'x,y\n' + points.map((p) => `${n(p.x)},${n(p.y)}`).join('\n') + '\n';
}
