/** Bottom panel (PRD §28–§30, §44): surface plots, polars, diagnostics, data. */
import { useEffect, useMemo, useState } from 'react';
import { ResizeHandle } from './ResizeHandle';
import { useSimulationStore } from '@/state/simulationStore';
import { useSolverStore } from '@/state/solverStore';
import { useUIStore, type BottomTab } from '@/state/uiStore';
import type { BodyResult, SweepConfig } from '@/domain/types';
import { toDegrees, toRadians } from '@/domain/scene';
import { LineChart, type Series } from '@/charts/LineChart';
import { solverRef } from '@/App';
import { downloadText, safeFilename } from '@/export/download';
import { forcesCsv, sweepCsv, surfaceCsv } from '@/export/csv';
import { fmt } from './PropertiesPanel';
import { IconChart, IconDownload, IconInfo, IconPlay, IconStop, IconWarning } from './icons';

/** Stable empty fallbacks: zustand selectors must not return a fresh array each call. */
const EMPTY: never[] = [];

const UPPER = 'var(--accent)';
const LOWER = 'var(--select)';

function useFocusBody(): BodyResult | null {
  const selected = useUIStore((s) => s.selectedIds);
  const bodies = useSolverStore((s) => s.solution?.bodies ?? EMPTY);
  return useMemo(() => bodies.find((b) => selected.includes(b.id)) ?? bodies[0] ?? null, [bodies, selected]);
}

type SurfaceQuantity = 'cp' | 'velocity' | 'pressure';

function SurfaceTab() {
  const body = useFocusBody();
  const bodies = useSolverStore((s) => s.solution?.bodies ?? EMPTY);
  const conditions = useSimulationStore((s) => s.scene.conditions);
  const select = useUIStore((s) => s.select);
  const [quantity, setQuantity] = useState<SurfaceQuantity>('cp');

  if (!body) {
    return (
      <div className="panel__empty" style={{ flex: 1 }}>
        Surface plots appear here when a body is in the scene. Add a NACA airfoil or a cylinder from <b>+ Add → Geometry</b>, or try an example.
      </div>
    );
  }

  const u = conditions.velocity || 1;
  const value = (sp: BodyResult['surface'][number]) =>
    quantity === 'cp' ? (sp.cp ?? NaN) : quantity === 'velocity' ? Math.abs(sp.tangentialVelocity) / u : sp.pressure - conditions.pressure;
  // Plot in contour order, not sorted by x: a strongly cambered or reflexed surface can fold back in x,
  // and re-sorting would interleave its points into a zigzag. The contour runs TE → upper → LE → lower → TE.
  const upper = body.surface.filter((s) => s.surface === 'upper').map((s) => ({ x: s.xOverC, y: value(s) })).reverse();
  const lower = body.surface.filter((s) => s.surface === 'lower').map((s) => ({ x: s.xOverC, y: value(s) }));
  const series: Series[] = [
    { id: 'upper', label: 'Upper', color: UPPER, points: upper },
    { id: 'lower', label: 'Lower', color: LOWER, points: lower },
  ];
  const yLabel = quantity === 'cp' ? 'Cp' : quantity === 'velocity' ? '|V| / U∞' : 'p − p∞  [Pa]';

  return (
    <>
      <div className="side-list">
        <label className="field field--inline">
          <span className="field__label">Body</span>
          <span className="field__input">
            <select value={body.id} onChange={(e) => select([e.target.value])}>
              {bodies.map((b) => (
                <option key={b.id} value={b.id}>{b.name}</option>
              ))}
            </select>
          </span>
        </label>
        <div className="segmented" role="radiogroup" aria-label="Quantity">
          {(['cp', 'velocity', 'pressure'] as const).map((q) => (
            <button key={q} role="radio" aria-checked={quantity === q} className={quantity === q ? 'is-active' : ''} onClick={() => setQuantity(q)}>
              {q === 'cp' ? 'Cp' : q === 'velocity' ? 'V/U∞' : 'Δp'}
            </button>
          ))}
        </div>
        <dl className="kv">
          <dt>Panels</dt><dd>{body.panelCount}</dd><dd className="unit" />
          <dt>Cp min</dt><dd>{fmt(Math.min(...body.surface.map((s) => s.cp ?? Infinity)), 3)}</dd><dd className="unit" />
          <dt>Cp max</dt><dd>{fmt(Math.max(...body.surface.map((s) => s.cp ?? -Infinity)), 3)}</dd><dd className="unit" />
          <dt>Max |Vₙ|/U∞</dt><dd>{fmt(Math.max(...body.surface.map((s) => Math.abs(s.normalVelocity))) / u, 2)}</dd><dd className="unit" />
        </dl>
        <button className="btn btn--sm" onClick={() => downloadText(`${safeFilename(body.name)}_surface.csv`, surfaceCsv(body, conditions), 'text/csv')}>
          <IconDownload size={14} /> Surface CSV
        </button>
        <p className="form__hint">{quantity === 'cp' ? 'Cp axis inverted: suction (negative Cp) plots upward, as in airfoil data sheets.' : 'x/c runs from the leading edge along the chord.'}</p>
      </div>
      <div className="chart">
        <div className="chart__legend" style={{ paddingTop: 8 }}>
          <span><span className="swatch" style={{ background: UPPER }} />Upper surface</span>
          <span><span className="swatch" style={{ background: LOWER }} />Lower surface</span>
        </div>
        <LineChart exportName={`${body.name.replace(/\s+/g, '_')}_${quantity}.svg`} series={series} xLabel="x/c" yLabel={yLabel} invertY={quantity === 'cp'} xDomain={[0, 1]} referenceY={[quantity === 'velocity' ? 1 : 0]} />
      </div>
    </>
  );
}

type SweepKind = SweepConfig['parameter'];

interface SweepMeta {
  label: string;
  axis: string;
  /** UI value → solver value (degrees → radians for α). */
  toSolver: (v: number) => number;
  fromSolver: (v: number) => number;
  defaults: [number, number];
  unit: string;
}

const SWEEP_META: Record<SweepKind, SweepMeta> = {
  angleOfAttack: { label: 'Angle of attack α', axis: 'α [deg]', toSolver: toRadians, fromSolver: toDegrees, defaults: [-10, 10], unit: '°' },
  freestreamSpeed: { label: 'Freestream speed U∞', axis: 'U∞ [m/s]', toSolver: (v) => v, fromSolver: (v) => v, defaults: [1, 30], unit: 'm/s' },
  bodyCirculation: { label: 'Body circulation Γ', axis: 'Γ [m²/s]', toSolver: (v) => v, fromSolver: (v) => v, defaults: [-5, 5], unit: 'm²/s' },
  elementStrength: { label: 'Element strength', axis: 'strength', toSolver: (v) => v, fromSolver: (v) => v, defaults: [-10, 10], unit: '' },
};

function PolarTab() {
  const sweep = useSolverStore((s) => s.sweep);
  const bodies = useSolverStore((s) => s.solution?.bodies ?? EMPTY);
  const elements = useSimulationStore((s) => s.scene.elements);
  const focus = useFocusBody();
  const [kind, setKind] = useState<SweepKind>('angleOfAttack');
  const [targetElement, setTargetElement] = useState<string>('');
  const [start, setStart] = useState(-10);
  const [end, setEnd] = useState(10);
  const [steps, setSteps] = useState(21);
  const [cancel, setCancel] = useState<(() => void) | null>(null);
  const meta = SWEEP_META[kind];
  const sweepable = elements.filter((e) => e.visible);
  const elementId = targetElement || sweepable[0]?.id || '';

  const chooseKind = (k: SweepKind) => {
    setKind(k);
    setStart(SWEEP_META[k].defaults[0]);
    setEnd(SWEEP_META[k].defaults[1]);
  };

  const run = () => {
    const client = solverRef.client;
    if (!client) return;
    const config: SweepConfig = { parameter: kind, start: meta.toSolver(start), end: meta.toSolver(end), steps };
    if (kind === 'bodyCirculation') config.bodyId = focus?.id;
    if (kind === 'elementStrength') config.elementId = elementId;
    const store = useSolverStore.getState();
    store.sweepStart(config, steps);
    const handle = client.sweep(config, (point) => useSolverStore.getState().sweepProgress(point));
    setCancel(() => handle.cancel);
    handle.result
      .then((r) => useSolverStore.getState().sweepDone(r))
      .catch((e: Error) => useSolverStore.getState().sweepError(e.message))
      .finally(() => setCancel(null));
  };

  // Plot what was actually swept, which may differ from the current form.
  const ranKind: SweepKind = sweep.config?.parameter ?? kind;
  const ranMeta = SWEEP_META[ranKind];
  const bodyId = focus?.id;
  const pts = sweep.points.map((p) => {
    const b = bodyId ? p.bodies.find((x) => x.id === bodyId) : undefined;
    return { x: ranMeta.fromSolver(p.value), cl: b?.cl ?? NaN, cd: b?.cd ?? NaN, cm: b?.cm ?? NaN, lift: b?.lift ?? NaN };
  });
  const mk = (id: string, label: string, key: 'cl' | 'cd' | 'cm' | 'lift'): Series => ({
    id,
    label,
    color: UPPER,
    markers: true,
    points: pts.map((p) => ({ x: p.x, y: p[key] })),
  });
  // Coefficients are meaningless when the freestream speed itself is swept
  // (they stay constant by construction); show dimensional lift instead.
  const showLift = ranKind === 'freestreamSpeed';
  const needsBody = bodies.length === 0;
  const needsElement = kind === 'elementStrength' && sweepable.length === 0;
  const needsPrescribed = kind === 'bodyCirculation' && !focus;

  return (
    <>
      <div className="side-list">
        <label className="field field--inline">
          <span className="field__label">Sweep</span>
          <span className="field__input">
            <select value={kind} onChange={(e) => chooseKind(e.target.value as SweepKind)} aria-label="Sweep parameter">
              {(Object.keys(SWEEP_META) as SweepKind[]).map((k) => (
                <option key={k} value={k}>{SWEEP_META[k].label}</option>
              ))}
            </select>
          </span>
        </label>
        {kind === 'elementStrength' && sweepable.length > 0 && (
          <label className="field field--inline">
            <span className="field__label">Element</span>
            <span className="field__input">
              <select value={elementId} onChange={(e) => setTargetElement(e.target.value)} aria-label="Element to sweep">
                {sweepable.map((e) => <option key={e.id} value={e.id}>{e.name}</option>)}
              </select>
            </span>
          </label>
        )}
        <p className="form__hint" style={{ margin: 0 }}>
          {kind === 'bodyCirculation'
            ? `Prescribes Γ on “${focus?.name ?? '—'}” at each point: lift should follow L = −ρU∞Γ.`
            : `Results for “${focus?.name ?? '—'}”.`}
        </p>
        <label className="field field--inline"><span className="field__label">From</span><span className="field__input"><input type="number" value={start} onChange={(e) => setStart(Number(e.target.value))} aria-label="Sweep start" /><span className="field__unit">{meta.unit}</span></span></label>
        <label className="field field--inline"><span className="field__label">To</span><span className="field__input"><input type="number" value={end} onChange={(e) => setEnd(Number(e.target.value))} aria-label="Sweep end" /><span className="field__unit">{meta.unit}</span></span></label>
        <label className="field field--inline"><span className="field__label">Points</span><span className="field__input"><input type="number" min={2} max={201} value={steps} onChange={(e) => setSteps(Math.max(2, Math.min(201, Number(e.target.value))))} aria-label="Sweep points" /><span className="field__unit" /></span></label>
        {sweep.running ? (
          <>
            <div className="progress" role="progressbar" aria-label="Sweep progress" aria-valuemin={0} aria-valuemax={sweep.total} aria-valuenow={sweep.points.length}><div style={{ width: `${(100 * sweep.points.length) / Math.max(sweep.total, 1)}%` }} /></div>
            <button className="btn btn--sm" onClick={() => cancel?.()}><IconStop size={14} /> Cancel ({sweep.points.length}/{sweep.total})</button>
          </>
        ) : (
          <button className="btn btn--sm btn--primary" onClick={run} disabled={needsBody || needsElement || needsPrescribed}>
            <IconPlay size={14} /> Run sweep
          </button>
        )}
        {needsElement && <p className="form__hint">Add a flow element to sweep its strength.</p>}
        {kind === 'bodyCirculation' && focus?.kuttaApplicable && (
          <div className="callout callout--warning">
            <IconWarning size={14} />
            <span>
              “{focus.name}” has a sharp trailing edge. Prescribing Γ overrides the Kutta condition, so except at the Kutta value the flow turns the edge at unbounded speed: lift still follows L = −ρU∞Γ, but CD and Cm are not reliable.
            </span>
          </div>
        )}
        {sweep.result && !sweep.result.completed && <p className="form__hint" role="status">Cancelled after {sweep.result.points.length} points.</p>}
        {sweep.error && <div className="callout callout--danger"><IconWarning size={14} /><span>{sweep.error}</span></div>}
        {sweep.points.length > 0 && (
          <button className="btn btn--sm" onClick={() => downloadText('sweep.csv', sweepCsv(sweep.points, ranMeta.axis, ranMeta.fromSolver), 'text/csv')}>
            <IconDownload size={14} /> Sweep CSV
          </button>
        )}
        <div className="callout"><IconInfo size={14} /><span>CD here is inviscid pressure drag — a discretisation residual, not a drag prediction.</span></div>
      </div>
      {sweep.points.length === 0 ? (
        <div className="panel__empty" style={{ flex: 1 }}>{needsBody ? 'Add a body to run a sweep.' : 'Run a sweep to plot the aerodynamic coefficients against the chosen parameter.'}</div>
      ) : (
        <div style={{ flex: 1, display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', minWidth: 0 }}>
          {showLift ? (
            <div className="chart"><LineChart exportName="lift_vs_speed.svg" series={[mk('lift', 'Lift', 'lift')]} xLabel={ranMeta.axis} yLabel="Lift [N/m]" referenceY={[0]} /></div>
          ) : (
            <div className="chart"><LineChart exportName="CL_sweep.svg" series={[mk('cl', 'CL', 'cl')]} xLabel={ranMeta.axis} yLabel="CL" referenceY={[0]} /></div>
          )}
          <div className="chart"><LineChart exportName="Cm_sweep.svg" series={[mk('cm', 'Cm', 'cm')]} xLabel={ranMeta.axis} yLabel="Cm" referenceY={[0]} /></div>
          <div className="chart">
            <div className="chart__legend" style={{ paddingTop: 6 }}>
              CD ≈ 0 is the correct inviscid result (d&apos;Alembert); the spread is discretisation error.
            </div>
            <LineChart
              exportName="drag_polar.svg"
              series={[{ id: 'polar', label: 'CL', color: LOWER, markers: true, points: pts.map((p) => ({ x: p.cd, y: p.cl })) }]}
              xLabel="CD (inviscid residual)"
              yLabel="CL"
              referenceY={[0]}
              minSpanX={0.02}
            />
          </div>
        </div>
      )}
    </>
  );
}

function DiagnosticsTab() {
  const solution = useSolverStore((s) => s.solution);
  const roundTrip = useSolverStore((s) => s.lastRoundTripMs);
  const sampleMs = useSolverStore((s) => s.lastSampleMs);
  const select = useUIStore((s) => s.select);
  const openHelp = useUIStore((s) => s.openHelp);
  if (!solution) return <div className="panel__empty" style={{ flex: 1 }}>No solution yet.</div>;
  const d = solution.diagnostics;
  const p = d.panel;
  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0 }}>
      <div className="side-list" style={{ width: 300 }} tabIndex={0} role="region" aria-label="Solver timings and numerical diagnostics">
        <dl className="kv">
          <dt>Status</dt><dd>{solution.status}</dd><dd className="unit" />
          <dt>Solve (WASM)</dt><dd>{fmt(d.timings.totalMs, 2)}</dd><dd className="unit">ms</dd>
          <dt>· prepare</dt><dd>{fmt(d.timings.prepareMs, 2)}</dd><dd className="unit">ms</dd>
          <dt>· assemble + LU</dt><dd>{d.systemReused ? 'reused' : fmt(d.timings.assembleMs, 2)}</dd><dd className="unit">{d.systemReused ? '' : 'ms'}</dd>
          <dt>· solve</dt><dd>{fmt(d.timings.solveMs, 2)}</dd><dd className="unit">ms</dd>
          <dt>· forces</dt><dd>{fmt(d.timings.forcesMs, 2)}</dd><dd className="unit">ms</dd>
          <dt>Field sampling</dt><dd>{fmt(sampleMs, 1)}</dd><dd className="unit">ms</dd>
          <dt>Round trip</dt><dd>{fmt(roundTrip, 1)}</dd><dd className="unit">ms</dd>
          <dt>Elements</dt><dd>{d.elementCount}</dd><dd className="unit" />
          <dt>Bodies</dt><dd>{d.bodyCount}</dd><dd className="unit" />
          <dt>Total circulation</dt><dd>{fmt(d.totalCirculation)}</dd><dd className="unit">m²/s</dd>
          <dt>Net outflow</dt><dd>{fmt(d.netOutflow)}</dd><dd className="unit">m²/s</dd>
        </dl>
        {p && (
          <dl className="kv">
            <dt>Panels</dt><dd>{p.panelCount}</dd><dd className="unit" />
            <dt>Unknowns</dt><dd>{p.unknownCount}</dd><dd className="unit" />
            <dt>Condition number</dt><dd>{p.conditionNumber.toExponential(2)}</dd><dd className="unit" />
            <dt>Relative residual</dt><dd>{p.relativeResidual.toExponential(2)}</dd><dd className="unit" />
            <dt>Max |Vₙ|</dt><dd>{p.maxNormalVelocity.toExponential(2)}</dd><dd className="unit">m/s</dd>
            <dt>Panel length</dt><dd>{fmt(p.minPanelLength, 4)} – {fmt(p.maxPanelLength, 4)}</dd><dd className="unit">m</dd>
          </dl>
        )}
      </div>
      <div style={{ flex: 1, padding: '10px 14px', overflow: 'auto' }} tabIndex={0} role="region" aria-label="Warnings and model assumptions">
        <h3 style={{ margin: '0 0 6px', fontSize: 'var(--fs-xs)', textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--text-faint)' }}>Warnings</h3>
        {solution.error && <div className="callout callout--danger"><IconWarning size={14} /><span>{solution.error}</span></div>}
        {solution.warnings.length === 0 && !solution.error && <p className="muted" style={{ margin: 0 }}>None. The solution satisfies flow tangency to round-off.</p>}
        {solution.warnings.map((w, i) => (
          <div key={i} className={`callout${w.severity === 'warning' ? ' callout--warning' : ''}`} style={{ marginBottom: 6 }}>
            <IconWarning size={14} />
            <span>
              {w.message}{' '}
              {w.objectId && <button className="link-btn" onClick={() => select([w.objectId!])}>Select object</button>}
            </span>
          </div>
        ))}
        <h3 style={{ margin: '14px 0 6px', fontSize: 'var(--fs-xs)', textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--text-faint)' }}>
          Assumptions <button className="link-btn" onClick={() => openHelp('assumptions')}>explain</button>
        </h3>
        <ul style={{ margin: 0, paddingLeft: 18, fontSize: 'var(--fs-sm)', color: 'var(--text-muted)' }}>
          {solution.assumptions.map((a) => <li key={a}>{a}</li>)}
        </ul>
      </div>
    </div>
  );
}

function DataTab() {
  const solution = useSolverStore((s) => s.solution);
  const scene = useSimulationStore((s) => s.scene);
  const openExport = () => useUIStore.getState().openDialog('export');
  const openHelp = useUIStore((s) => s.openHelp);
  const hasBodies = !!solution && solution.bodies.length > 0;
  const hasElements = !!solution && (solution.elements ?? []).length > 0;
  if (!solution || (!hasBodies && !hasElements)) {
    return (
      <div className="panel__empty" style={{ flex: 1 }}>
        Force tables appear here when bodies or flow elements are present. <button className="link-btn" onClick={openExport}>Export field data or the simulation file</button> at any time.
      </div>
    );
  }
  return (
    <div style={{ flex: 1, padding: '8px 12px', overflow: 'auto' }} tabIndex={0} role="region" aria-label="Force tables">
      {hasBodies && (
        <table className="table" aria-label="Forces per body">
          <thead>
            <tr><th>Body</th><th>Lift [N/m]</th><th>Drag [N/m]</th><th>Fx [N/m]</th><th>Fy [N/m]</th><th>M [N·m/m]</th><th>CL</th><th>CD</th><th>Cm</th><th>Γ [m²/s]</th><th>Panels</th></tr>
          </thead>
          <tbody>
            {solution.bodies.map((b) => (
              <tr key={b.id}>
                <td>{b.name}</td><td>{fmt(b.forces.lift)}</td><td>{fmt(b.forces.drag)}</td><td>{fmt(b.forces.fx)}</td><td>{fmt(b.forces.fy)}</td><td>{fmt(b.forces.moment)}</td>
                <td>{fmt(b.forces.cl, 4)}</td><td>{fmt(b.forces.cd, 5)}</td><td>{fmt(b.forces.cm, 4)}</td><td>{fmt(b.forces.circulation)}</td><td>{b.panelCount}</td>
              </tr>
            ))}
            <tr className="is-total">
              <td>Total (bodies)</td><td>{fmt(solution.total.lift)}</td><td>{fmt(solution.total.drag)}</td><td>{fmt(solution.total.fx)}</td><td>{fmt(solution.total.fy)}</td><td>{fmt(solution.total.moment)}</td><td /><td /><td /><td>{fmt(solution.total.circulation)}</td><td />
            </tr>
          </tbody>
        </table>
      )}
      {hasElements && (
        <>
          <table className="table" aria-label="Forces on elements" style={{ marginTop: hasBodies ? 14 : 0 }}>
            <thead>
              <tr><th>Element</th><th>Lift [N/m]</th><th>Drag [N/m]</th><th>Fx [N/m]</th><th>Fy [N/m]</th><th>|F| [N/m]</th><th>Vₓ ext [m/s]</th><th>V_y ext [m/s]</th></tr>
            </thead>
            <tbody>
              {solution.elements.map((e) => (
                <tr key={e.id}>
                  <td>{e.name}</td><td>{fmt(e.lift)}</td><td>{fmt(e.drag)}</td><td>{fmt(e.force.x)}</td><td>{fmt(e.force.y)}</td><td>{fmt(Math.hypot(e.force.x, e.force.y))}</td>
                  <td>{fmt(e.externalVelocity.x)}</td><td>{fmt(e.externalVelocity.y)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="form__hint" style={{ marginTop: 6 }}>
            Element forces follow the Lagally theorem: the force needed to hold each singularity fixed, from the velocity induced at it by everything else.{' '}
            <button className="link-btn" onClick={() => openHelp('elementForces')}>Explain</button>
          </p>
        </>
      )}
      <div className="form__row" style={{ marginTop: 10 }}>
        <button className="btn btn--sm" onClick={() => downloadText(`${safeFilename(scene.name)}_forces.csv`, forcesCsv(solution), 'text/csv')}><IconDownload size={14} /> Forces CSV</button>
        <button className="btn btn--sm" onClick={openExport}><IconDownload size={14} /> More exports…</button>
      </div>
    </div>
  );
}

export function AnalysisPanel() {
  const tab = useUIStore((s) => s.bottomTab);
  const open = useUIStore((s) => s.bottomPanelOpen);
  const setTab = useUIStore((s) => s.setBottomTab);
  const togglePanel = useUIStore((s) => s.togglePanel);
  const warningCount = useSolverStore((s) => s.solution?.warnings.filter((w) => w.severity === 'warning').length ?? 0);

  // Keep the surface tab meaningful: when a body appears, show it.
  const hasBodies = useSolverStore((s) => (s.solution?.bodies.length ?? 0) > 0);
  useEffect(() => {
    if (hasBodies && tab === 'surface') void 0;
  }, [hasBodies, tab]);

  const tabs: { id: BottomTab; label: string }[] = [
    { id: 'surface', label: 'Surface' },
    { id: 'polar', label: 'Polar' },
    { id: 'diagnostics', label: warningCount ? `Diagnostics (${warningCount})` : 'Diagnostics' },
    { id: 'data', label: 'Data' },
  ];
  return (
    <section className="bottom" aria-label="Analysis">
      {open && <ResizeHandle panel="bottom" label="Resize analysis panel" />}
      <div className="bottom__tabs">
        <IconChart size={15} style={{ color: 'var(--text-faint)', marginRight: 4 }} />
        <div className="bottom__tablist" role="tablist" aria-label="Analysis views">
          {tabs.map((t) => (
            <button key={t.id} role="tab" aria-selected={tab === t.id && open} className={`tab${tab === t.id && open ? ' is-active' : ''}`} onClick={() => (tab === t.id && open ? togglePanel('bottom') : setTab(t.id))}>
              {t.label}
            </button>
          ))}
        </div>
        <span className="bottom__spacer" />
        <button className="btn btn--ghost btn--sm" onClick={() => togglePanel('bottom')}>{open ? 'Collapse' : 'Expand'}</button>
      </div>
      <div className="bottom__body" role="tabpanel">
        {tab === 'surface' && <SurfaceTab />}
        {tab === 'polar' && <PolarTab />}
        {tab === 'diagnostics' && <DiagnosticsTab />}
        {tab === 'data' && <DataTab />}
      </div>
    </section>
  );
}
