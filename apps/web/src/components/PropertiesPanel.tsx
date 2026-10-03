/**
 * Properties / results panel (PRD §24, §33). Element forms are generated from
 * the solver's descriptors; body forms expose geometry, transform, panels,
 * circulation and reference values; results show per-body forces with the
 * inviscid-drag caveat shown contextually (PRD §25).
 */
import { useMemo } from 'react';
import { useSimulationStore } from '@/state/simulationStore';
import { useUIStore } from '@/state/uiStore';
import { useSolverStore } from '@/state/solverStore';
import type { BodyGeometry, BodyResult, CirculationSetting, PanelDistribution, SceneBody, SceneElement } from '@/domain/types';
import { readParameter, toDegrees, toRadians, writeParameter } from '@/domain/scene';
import { NumberField } from './NumberField';
import { IconCopy, IconHelp, IconInfo, IconTrash, IconWarning } from './icons';

/** Stable empty fallbacks: zustand selectors must not return a fresh array each call. */
const EMPTY: never[] = [];

export const fmt = (v: number | null | undefined, digits = 3): string => {
  if (v === null || v === undefined || !Number.isFinite(v)) return '—';
  const a = Math.abs(v);
  if (a !== 0 && (a >= 1e5 || a < 1e-3)) return v.toExponential(2);
  return v.toFixed(digits);
};

/** Convert the solver's world-space default reference point back to body-local coordinates. */
function quarterChordLocal(b: SceneBody, world?: { x: number; y: number }): { x: number; y: number } {
  if (!world) return { x: 0.25, y: 0 };
  const dx = (world.x - b.position.x) / (b.scale || 1);
  const dy = (world.y - b.position.y) / (b.scale || 1);
  const c = Math.cos(-b.rotation);
  const s = Math.sin(-b.rotation);
  const r = (v: number) => Math.round(v * 1e6) / 1e6;
  return { x: r(dx * c - dy * s), y: r(dx * s + dy * c) };
}

function HelpButton({ topic }: { topic: string }) {
  const open = useUIStore((s) => s.openHelp);
  return (
    <button className="icon-btn" onClick={() => open(topic)} aria-label="Help" title="Explain">
      <IconHelp size={14} />
    </button>
  );
}

function FreestreamForm() {
  const c = useSimulationStore((s) => s.scene.conditions);
  const set = useSimulationStore((s) => s.setConditions);
  return (
    <div className="form">
      <div className="form__section">
        <h3>
          Freestream <HelpButton topic="uniformFlow" />
        </h3>
        <NumberField label="Speed" symbol="U∞" unit="m/s" value={c.velocity} step={0.5} sliderMin={0} sliderMax={50} onChange={(v, t) => set({ velocity: v }, { transient: t })} />
        <NumberField label="Angle of attack" symbol="α" unit="°" value={toDegrees(c.angle)} step={0.5} sliderMin={-20} sliderMax={20} digits={2} onChange={(v, t) => set({ angle: toRadians(v) }, { transient: t })} />
        <NumberField label="Density" symbol="ρ" unit="kg/m³" value={c.density} step={0.01} min={1e-6} onChange={(v, t) => set({ density: v }, { transient: t })} />
        <NumberField label="Static pressure" symbol="p∞" unit="Pa" value={c.pressure} step={100} digits={1} onChange={(v, t) => set({ pressure: v }, { transient: t })} />
        <p className="form__hint">+x right, +y up, positive α counter-clockwise. Γ &gt; 0 is counter-clockwise, so lift is L = −ρU∞Γ.</p>
      </div>
      <div className="form__section">
        <h3>
          Model <HelpButton topic="assumptions" />
        </h3>
        <div className="callout">
          <IconInfo size={14} />
          <span>Steady, 2D, incompressible, inviscid potential flow. Forces are per unit span. Drag of a closed body is zero by theory; viscous effects are not modelled.</span>
        </div>
      </div>
    </div>
  );
}

function ElementForm({ item }: { item: SceneElement }) {
  const descriptor = useSolverStore((s) => s.metadata?.descriptors.find((d) => d.kind === item.element.type));
  const updateElement = useSimulationStore((s) => s.updateElement);
  const removeObjects = useSimulationStore((s) => s.removeObjects);
  const duplicateObject = useSimulationStore((s) => s.duplicateObject);
  const allWarnings = useSolverStore((s) => s.solution?.warnings ?? EMPTY);
  const warnings = useMemo(() => allWarnings.filter((w) => w.objectId === item.id), [allWarnings, item.id]);
  const select = useUIStore((s) => s.select);
  if (!descriptor) return <p className="panel__empty">Loading…</p>;
  return (
    <div className="form">
      <div className="form__section">
        <h3>
          {descriptor.name} <HelpButton topic={descriptor.help.id} />
        </h3>
        <p className="form__hint">{descriptor.summary}</p>
        {descriptor.parameters.map((p) => {
          const isAngle = p.kind === 'angle';
          const raw = readParameter(item.element, p.key);
          const shown = isAngle ? toDegrees(raw) : raw;
          const isPos = p.kind === 'positionX' || p.kind === 'positionY';
          return (
            <NumberField
              key={p.key}
              label={p.label}
              symbol={p.symbol}
              unit={p.unit}
              value={shown}
              step={p.step}
              digits={isAngle ? 2 : 4}
              disabled={isPos && item.locked}
              sliderMin={isPos ? undefined : p.softMin}
              sliderMax={isPos ? undefined : p.softMax}
              help={p.help}
              onChange={(v, transient) =>
                updateElement(item.id, (e) => ({ ...e, element: writeParameter(e.element, p.key, isAngle ? toRadians(v) : v) }), { transient })
              }
            />
          );
        })}
      </div>
      {warnings.map((w, i) => (
        <div key={i} className={`callout callout--${w.severity === 'warning' ? 'warning' : ''}`}>
          <IconWarning size={14} />
          <span>{w.message}</span>
        </div>
      ))}
      <div className="form__row">
        <button className="btn btn--sm" onClick={() => { const id = duplicateObject(item.id); if (id) select([id]); }}>
          <IconCopy size={14} /> Duplicate
        </button>
        <button className="btn btn--sm btn--danger" onClick={() => { removeObjects([item.id]); select([]); }}>
          <IconTrash size={14} /> Delete
        </button>
      </div>
    </div>
  );
}

function GeometryFields({ body, update }: { body: SceneBody; update: (fn: (b: SceneBody) => SceneBody, transient?: boolean) => void }) {
  const g = body.geometry;
  const setGeom = (patch: Partial<BodyGeometry>, transient = false) =>
    update((b) => ({ ...b, geometry: { ...b.geometry, ...patch } as BodyGeometry }), transient);
  switch (g.kind) {
    case 'naca4':
      return (
        <>
          <label className="field field--inline">
            <span className="field__label">NACA code</span>
            <span className="field__input">
              <input
                type="text"
                defaultValue={g.code}
                maxLength={4}
                pattern="[0-9]{4}"
                aria-label="NACA 4-digit code"
                onBlur={(e) => {
                  const v = e.target.value.replace(/\D/g, '');
                  if (v.length === 4 && v !== g.code) setGeom({ code: v });
                  else e.target.value = g.code;
                }}
                onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()}
              />
            </span>
          </label>
          <NumberField label="Chord" symbol="c" unit="m" value={g.chord} step={0.1} min={1e-6} sliderMin={0.1} sliderMax={5} onChange={(v, t) => setGeom({ chord: v }, t)} />
        </>
      );
    case 'circle':
      return <NumberField label="Radius" symbol="r" unit="m" value={g.radius} step={0.1} min={1e-6} sliderMin={0.1} sliderMax={5} onChange={(v, t) => setGeom({ radius: v }, t)} />;
    case 'ellipse':
      return (
        <>
          <NumberField label="Semi-axis x" symbol="a" unit="m" value={g.semiAxisX} step={0.1} min={1e-6} sliderMin={0.1} sliderMax={5} onChange={(v, t) => setGeom({ semiAxisX: v }, t)} />
          <NumberField label="Semi-axis y" symbol="b" unit="m" value={g.semiAxisY} step={0.1} min={1e-6} sliderMin={0.05} sliderMax={5} onChange={(v, t) => setGeom({ semiAxisY: v }, t)} />
        </>
      );
    case 'joukowski':
      return (
        <>
          <NumberField label="Thickness parameter" symbol="ε" value={g.thickness} step={0.01} min={0} sliderMin={0} sliderMax={0.3} onChange={(v, t) => setGeom({ thickness: v }, t)} />
          <NumberField label="Camber parameter" symbol="δ" value={g.camber} step={0.01} sliderMin={-0.2} sliderMax={0.2} onChange={(v, t) => setGeom({ camber: v }, t)} />
          <p className="form__hint">Normalised to unit chord. A true cusped trailing edge: use ≥ 400 panels for accurate pressure-integrated lift.</p>
        </>
      );
    case 'points':
      return (
        <p className="form__hint">
          Imported contour with {g.points.length} points{body.sourceName ? ` (“${body.sourceName}”)` : ''}. Re-panelise below to change the resolution.
        </p>
      );
  }
}

function BodyResults({ result }: { result: BodyResult }) {
  const f = result.forces;
  const openHelp = useUIStore((s) => s.openHelp);
  const consistencyPct = (100 * f.liftConsistency).toFixed(2);
  return (
    <div className="results">
      <h3 style={{ margin: 0, fontSize: 'var(--fs-xs)', textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--text-faint)' }}>Results (per unit span)</h3>
      <dl className="kv kv--big">
        <dt>Lift</dt><dd className="val">{fmt(f.lift)}</dd><dd className="unit">N/m</dd>
        <dt>Drag</dt><dd className="val">{fmt(f.drag)}</dd><dd className="unit">N/m</dd>
      </dl>
      <dl className="kv">
        <dt>Fx</dt><dd>{fmt(f.fx)}</dd><dd className="unit">N/m</dd>
        <dt>Fy</dt><dd>{fmt(f.fy)}</dd><dd className="unit">N/m</dd>
        <dt>Moment (nose-up +)</dt><dd>{fmt(f.moment)}</dd><dd className="unit">N·m/m</dd>
        <dt>CL</dt><dd>{fmt(f.cl, 4)}</dd><dd className="unit" />
        <dt>CD</dt><dd>{fmt(f.cd, 5)}</dd><dd className="unit" />
        <dt>Cm (ref. point)</dt><dd>{fmt(f.cm, 4)}</dd><dd className="unit" />
        <dt>Circulation Γ</dt><dd>{fmt(f.circulation)}</dd><dd className="unit">m²/s</dd>
        <dt>Lift from −ρU∞Γ</dt><dd>{fmt(f.liftKuttaJoukowski)}</dd><dd className="unit">N/m</dd>
        <dt>Reference chord</dt><dd>{fmt(f.reference.chord)}</dd><dd className="unit">m</dd>
      </dl>
      <div className={`callout${f.liftConsistency > 0.01 ? ' callout--warning' : ''}`}>
        <IconInfo size={14} />
        <span>
          Pressure-integrated lift and Kutta–Joukowski lift agree to {consistencyPct}%.{' '}
          {f.liftConsistency > 0.01 ? 'Increase the panel count for a cleaner result.' : ''}
        </span>
      </div>
      <div className="callout">
        <IconInfo size={14} />
        <span>
          Potential-flow drag is inviscid pressure drag and is zero in theory (d&apos;Alembert). The value above is discretisation error, not a prediction of real drag.{' '}
          <button className="link-btn" onClick={() => openHelp('drag')}>Why?</button>
        </span>
      </div>
    </div>
  );
}

function BodyForm({ body }: { body: SceneBody }) {
  const updateBody = useSimulationStore((s) => s.updateBody);
  const removeObjects = useSimulationStore((s) => s.removeObjects);
  const duplicateObject = useSimulationStore((s) => s.duplicateObject);
  const select = useUIStore((s) => s.select);
  const result = useSolverStore((s) => s.solution?.bodies.find((b) => b.id === body.id));
  const allWarnings = useSolverStore((s) => s.solution?.warnings ?? EMPTY);
  const warnings = useMemo(() => allWarnings.filter((w) => w.objectId === body.id), [allWarnings, body.id]);
  const update = (fn: (b: SceneBody) => SceneBody, transient = false) => updateBody(body.id, fn, { transient });
  const circ = body.circulation;
  const setCirc = (c: CirculationSetting) => update((b) => ({ ...b, circulation: c }));

  return (
    <div className="form">
      <div className="form__section">
        <h3>
          Geometry <HelpButton topic="panelMethod" />
        </h3>
        <GeometryFields body={body} update={update} />
      </div>
      <div className="form__section">
        <h3>Transform</h3>
        <NumberField label="Position X" symbol="x" unit="m" value={body.position.x} step={0.05} disabled={body.locked} onChange={(v, t) => update((b) => ({ ...b, position: { ...b.position, x: v } }), t)} />
        <NumberField label="Position Y" symbol="y" unit="m" value={body.position.y} step={0.05} disabled={body.locked} onChange={(v, t) => update((b) => ({ ...b, position: { ...b.position, y: v } }), t)} />
        <NumberField label="Rotation" symbol="θ" unit="°" value={toDegrees(body.rotation)} step={0.5} digits={2} sliderMin={-30} sliderMax={30} onChange={(v, t) => update((b) => ({ ...b, rotation: toRadians(v) }), t)} />
        <NumberField label="Scale" value={body.scale} step={0.1} min={1e-6} sliderMin={0.1} sliderMax={4} onChange={(v, t) => update((b) => ({ ...b, scale: v }), t)} />
        <p className="form__hint">Rotating the body counter-clockwise by θ is equivalent to pitching it nose-down; use α on the freestream for angle of attack.</p>
      </div>
      <div className="form__section">
        <h3>Panels</h3>
        <NumberField label="Panel count" symbol="N" value={body.panels.count} step={10} min={16} max={2000} digits={0} sliderMin={20} sliderMax={500} onChange={(v, t) => update((b) => ({ ...b, panels: { ...b.panels, count: Math.round(v) } }), t)} />
        <label className="field field--inline">
          <span className="field__label">Distribution</span>
          <span className="field__input">
            <select value={body.panels.distribution} onChange={(e) => update((b) => ({ ...b, panels: { ...b.panels, distribution: e.target.value as PanelDistribution } }))}>
              <option value="auto">Auto</option>
              <option value="cosine">Cosine (airfoils)</option>
              <option value="uniform">Uniform arc length</option>
              <option value="curvature">Curvature weighted</option>
              {body.geometry.kind === 'points' && <option value="asImported">As imported</option>}
            </select>
          </span>
        </label>
        {result && (
          <p className="form__hint">
            Solved with {result.panelCount} panels. {result.notes.join(' ')}
          </p>
        )}
      </div>
      <div className="form__section">
        <h3>
          Circulation <HelpButton topic="kuttaCondition" />
        </h3>
        <div className="segmented" role="radiogroup" aria-label="Circulation mode">
          {(['auto', 'kutta', 'none', 'prescribed'] as const).map((m) => (
            <button
              key={m}
              role="radio"
              aria-checked={circ.mode === m}
              className={circ.mode === m ? 'is-active' : ''}
              onClick={() => setCirc(m === 'prescribed' ? { mode: 'prescribed', circulation: result?.forces.circulation ?? 0 } : { mode: m })}
            >
              {m === 'auto' ? 'Auto' : m === 'kutta' ? 'Kutta' : m === 'none' ? 'None' : 'Prescribed'}
            </button>
          ))}
        </div>
        {circ.mode === 'prescribed' && (
          <NumberField label="Circulation" symbol="Γ" unit="m²/s" value={circ.circulation} step={0.1} sliderMin={-20} sliderMax={20} onChange={(v, t) => update((b) => ({ ...b, circulation: { mode: 'prescribed', circulation: v } }), t)} />
        )}
        {result && (
          <p className="form__hint">
            {result.kuttaApplicable
              ? `Sharp trailing edge detected (included angle ${result.trailingEdge ? toDegrees(result.trailingEdge.includedAngle).toFixed(1) : '—'}°). `
              : 'No sharp trailing edge: the body is non-lifting unless a circulation is prescribed. '}
            Solved with <span className={`pill${result.circulationMode.mode === 'kutta' ? ' pill--kutta' : ''}`}>{result.circulationMode.mode}</span>.
          </p>
        )}
      </div>
      <div className="form__section">
        <h3>Reference values</h3>
        <NumberField
          label="Chord override"
          symbol="c"
          unit="m"
          value={body.reference.chord ?? result?.forces.reference.chord ?? 1}
          step={0.1}
          min={1e-6}
          onChange={(v) => update((b) => ({ ...b, reference: { ...b.reference, chord: v } }))}
          help="Reference length for CL, CD, Cm. Leave at the detected chord unless you need a different normalisation."
        />
        <label className="checkbox">
          <input
            type="checkbox"
            checked={!!body.reference.point}
            onChange={(e) =>
              update((b) => ({
                ...b,
                reference: { ...b.reference, point: e.target.checked ? quarterChordLocal(b, result?.forces.reference.point) : null },
              }))
            }
          />
          Custom moment reference point
        </label>
        {body.reference.point ? (
          <>
            <NumberField label="Reference x (body)" symbol="xᵣ" unit="m" value={body.reference.point.x} step={0.05} onChange={(v) => update((b) => ({ ...b, reference: { ...b.reference, point: { x: v, y: b.reference.point?.y ?? 0 } } }))} />
            <NumberField label="Reference y (body)" symbol="yᵣ" unit="m" value={body.reference.point.y} step={0.05} onChange={(v) => update((b) => ({ ...b, reference: { ...b.reference, point: { x: b.reference.point?.x ?? 0, y: v } } }))} />
            <p className="form__hint">In body-local coordinates, so the point moves and rotates with the body.</p>
          </>
        ) : (
          <p className="form__hint">Moment and Cm are taken about the quarter chord.</p>
        )}
      </div>
      {warnings.map((w, i) => (
        <div key={i} className={`callout${w.severity === 'warning' ? ' callout--warning' : ''}`}>
          <IconWarning size={14} />
          <span>{w.message}</span>
        </div>
      ))}
      <div className="form__row">
        <button className="btn btn--sm" onClick={() => { const id = duplicateObject(body.id); if (id) select([id]); }}>
          <IconCopy size={14} /> Duplicate
        </button>
        <button className="btn btn--sm btn--danger" onClick={() => { removeObjects([body.id]); select([]); }}>
          <IconTrash size={14} /> Delete
        </button>
      </div>
      {result && <BodyResults result={result} />}
    </div>
  );
}

function TotalResults() {
  const solution = useSolverStore((s) => s.solution);
  if (!solution || solution.bodies.length === 0) return null;
  const t = solution.total;
  return (
    <div className="results">
      <h3 style={{ margin: 0, fontSize: 'var(--fs-xs)', textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--text-faint)' }}>Total over {solution.bodies.length} bod{solution.bodies.length === 1 ? 'y' : 'ies'}</h3>
      <dl className="kv">
        <dt>Lift</dt><dd>{fmt(t.lift)}</dd><dd className="unit">N/m</dd>
        <dt>Drag</dt><dd>{fmt(t.drag)}</dd><dd className="unit">N/m</dd>
        <dt>Fx</dt><dd>{fmt(t.fx)}</dd><dd className="unit">N/m</dd>
        <dt>Fy</dt><dd>{fmt(t.fy)}</dd><dd className="unit">N/m</dd>
        <dt>Moment</dt><dd>{fmt(t.moment)}</dd><dd className="unit">N·m/m</dd>
        <dt>Circulation</dt><dd>{fmt(t.circulation)}</dd><dd className="unit">m²/s</dd>
      </dl>
    </div>
  );
}

export function PropertiesPanel() {
  const selected = useUIStore((s) => s.selectedIds);
  const scene = useSimulationStore((s) => s.scene);
  const compact = useUIStore((s) => s.compact);
  const removeObjects = useSimulationStore((s) => s.removeObjects);
  const clear = useUIStore((s) => s.clearSelection);

  let title = 'Freestream';
  let body: React.ReactNode;
  if (selected.length === 0) {
    body = (
      <>
        <FreestreamForm />
        <TotalResults />
      </>
    );
  } else if (selected.length === 1) {
    const el = scene.elements.find((e) => e.id === selected[0]);
    const bd = scene.bodies.find((b) => b.id === selected[0]);
    if (el) {
      title = el.name;
      body = <ElementForm item={el} />;
    } else if (bd) {
      title = bd.name;
      body = <BodyForm body={bd} />;
    } else {
      body = <p className="panel__empty">The selected object no longer exists.</p>;
    }
  } else {
    title = `${selected.length} objects`;
    body = (
      <div className="form">
        <p className="form__hint">Multiple objects selected. Drag them together on the canvas, or:</p>
        <div className="form__row">
          <button className="btn btn--sm btn--danger" onClick={() => { removeObjects(selected); clear(); }}>
            <IconTrash size={14} /> Delete all
          </button>
          <button className="btn btn--sm btn--ghost" onClick={clear}>Clear selection</button>
        </div>
      </div>
    );
  }

  return (
    <aside className="panel panel--right" aria-label="Properties and results">
      <header className="panel__header">
        <h2>{selected.length === 0 ? 'Properties' : 'Properties'}</h2>
        <span className="panel__count" style={{ fontFamily: 'var(--font-sans)', color: 'var(--text)' }}>{title}</span>
        {compact && (
          <button className="icon-btn" onClick={() => useUIStore.getState().setPanel('right', false)} aria-label="Close panel">×</button>
        )}
      </header>
      <div className="panel__body">{body}</div>
    </aside>
  );
}
