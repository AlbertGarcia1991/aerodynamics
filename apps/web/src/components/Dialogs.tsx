/** Modal dialogs: welcome, examples, import, geometry, export, shortcuts, about. */
import { useEffect, useMemo, useRef, useState, type ReactElement, type ReactNode } from 'react';
import { useUIStore, type DialogKind } from '@/state/uiStore';
import { useSimulationStore } from '@/state/simulationStore';
import { useSolverStore } from '@/state/solverStore';
import { useViewportStore } from '@/state/viewportStore';
import { EXAMPLES } from '@/domain/examples';
import type { BodyGeometry, GeometryIssue, PanelDistribution, ValidationResult, Vec2 } from '@/domain/types';
import { createBody, serialiseScene } from '@/domain/scene';
import { solverRef } from '@/App';
import { downloadBlob, downloadText, safeFilename } from '@/export/download';
import { fieldCsv, forcesCsv, geometryCsv, surfaceCsv } from '@/export/csv';
import { bezierWorldPolygon, boundsOf, fitPoints, sampleBezier, templateGeometry, TEMPLATE_LABELS, type BezierTemplate } from '@/domain/bezier';
import { SHORTCUTS } from '@/hooks/useShortcuts';
import { fitToScene } from '@/canvas/fit';
import { IconClose, IconDownload, IconInfo, IconWarning } from './icons';

const FIRST_RUN_KEY = 'aeroflow.seen-welcome';

function Dialog({ title, children, footer, width }: { title: string; children: ReactNode; footer?: ReactNode; width?: 'narrow' | 'wide' }) {
  const close = useUIStore((s) => s.closeDialog);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const first = ref.current?.querySelector<HTMLElement>('button, input, textarea, select');
    first?.focus();
  }, []);
  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div className={`dialog${width ? ` dialog--${width}` : ''}`} role="dialog" aria-modal="true" aria-label={title} ref={ref}>
        <header className="dialog__header">
          <h2>{title}</h2>
          <button className="icon-btn" onClick={close} aria-label="Close dialog"><IconClose /></button>
        </header>
        <div className="dialog__body">{children}</div>
        {footer && <footer className="dialog__footer">{footer}</footer>}
      </div>
    </div>
  );
}

function markSeen() {
  try {
    localStorage.setItem(FIRST_RUN_KEY, '1');
  } catch {
    /* ignore */
  }
}

function loadExample(id: string) {
  const ex = EXAMPLES.find((e) => e.id === id);
  if (!ex) return;
  useSimulationStore.getState().loadScene(ex.build());
  useUIStore.getState().clearSelection();
  const vp = useViewportStore.getState();
  const hw = ex.view.halfWidth;
  const hh = (hw * vp.height) / Math.max(vp.width, 1);
  vp.fit({ min: { x: ex.view.center.x - hw, y: ex.view.center.y - hh }, max: { x: ex.view.center.x + hw, y: ex.view.center.y + hh } }, 0);
  useUIStore.getState().toast(ex.highlight, 'info');
}

function ExamplePreview({ id }: { id: string }) {
  // Tiny schematic thumbnails, drawn as SVG so they need no solver round trip.
  const common = { stroke: 'var(--accent)', fill: 'none', strokeWidth: 1.5 } as const;
  switch (id) {
    case 'cylinder':
    case 'magnus':
    case 'doublet':
      return <svg viewBox="0 0 120 64" className="card__preview"><path d="M0 16c30-6 60-6 120 0M0 48c30 6 60 6 120 0M0 32h40M80 32h40" {...common} /><circle cx="60" cy="32" r="14" fill="var(--body-fill)" />{id === 'magnus' && <path d="M52 20a14 14 0 0 1 16 0" stroke="var(--select)" strokeWidth="2" fill="none" />}</svg>;
    case 'naca0012':
    case 'naca2412':
      return <svg viewBox="0 0 120 64" className="card__preview"><path d="M0 20c40-8 80-6 120 2M0 44c40 6 80 4 120-4" {...common} /><path d="M20 34c20-10 60-12 86-4-26 4-60 6-86 4Z" fill="var(--body-fill)" /></svg>;
    case 'biplane':
      return <svg viewBox="0 0 120 64" className="card__preview"><path d="M20 22c20-8 60-9 86-3-26 3-60 5-86 3ZM20 44c20-8 60-9 86-3-26 3-60 5-86 3Z" fill="var(--body-fill)" /></svg>;
    case 'source-sink':
      return <svg viewBox="0 0 120 64" className="card__preview"><ellipse cx="60" cy="32" rx="34" ry="14" {...common} /><circle cx="42" cy="32" r="3" fill="var(--select)" /><circle cx="78" cy="32" r="3" fill="var(--accent)" /><path d="M0 14c30-5 60-5 120 0M0 50c30 5 60 5 120 0" {...common} /></svg>;
    case 'vortex':
      return <svg viewBox="0 0 120 64" className="card__preview"><path d="M0 14c30 0 40 10 60 10s30-10 60-10M0 50c30 0 40-10 60-10s30 10 60 10" {...common} /><circle cx="60" cy="32" r="9" {...common} stroke="var(--select)" /><circle cx="60" cy="32" r="2" fill="var(--select)" /></svg>;
    case 'half-body':
      return <svg viewBox="0 0 120 64" className="card__preview"><path d="M30 32c10-14 40-16 90-14M30 32c10 14 40 16 90 14M0 32h30" {...common} /><circle cx="40" cy="32" r="3" fill="var(--select)" /></svg>;
    default:
      return <svg viewBox="0 0 120 64" className="card__preview"><path d="M0 16h120M0 32h120M0 48h120" {...common} /></svg>;
  }
}

function WelcomeDialog() {
  const close = useUIStore((s) => s.closeDialog);
  const openDialog = useUIStore((s) => s.openDialog);
  return (
    <Dialog
      title="Welcome to AeroFlow"
      width="narrow"
      footer={
        <>
          <button className="btn" onClick={() => { markSeen(); useSimulationStore.getState().loadScene({ ...useSimulationStore.getState().scene, elements: [], bodies: [], name: 'Untitled simulation' }); close(); }}>Start empty</button>
          <button className="btn btn--primary" onClick={() => { markSeen(); openDialog('examples'); }}>Try an example</button>
        </>
      }
    >
      <p style={{ fontSize: 'var(--fs-lg)', marginTop: 0 }}>Build a flow in seconds.</p>
      <p>
        Place sources, sinks, vortices and doublets, drop in an airfoil, or draw and reshape your own body with Bézier curves, and watch the potential-flow solution respond as you drag. The solver is a
        Hess–Smith panel method running in WebAssembly; every result comes with its diagnostics.
      </p>
      <ul style={{ paddingLeft: 18, color: 'var(--text-muted)' }}>
        <li><b>Drag</b> any object; <b>scroll</b> to zoom; drag the background to pan.</li>
        <li><b>Draw your own shape:</b> <i>Add → Create geometry → Editable (Bézier)</i>, then drag its nodes (<kbd>N</kbd>) or sketch with the pen (<kbd>P</kbd>).</li>
        <li>Switch fields with the toolbar on the canvas, or keys <kbd>1</kbd>–<kbd>7</kbd>.</li>
        <li>Select a body to read lift, drag, CL, CD, Cm — and why drag is ~0.</li>
        <li>Press <kbd>?</kbd> for all shortcuts; the book icon at the top right opens the full user guide.</li>
      </ul>
      <p className="muted" style={{ fontSize: 'var(--fs-xs)' }}>A cylinder is already loaded behind this dialog.</p>
    </Dialog>
  );
}

function ExamplesDialog() {
  const close = useUIStore((s) => s.closeDialog);
  return (
    <Dialog title="Examples" width="wide">
      <div className="card-grid">
        {EXAMPLES.map((ex) => (
          <button key={ex.id} className="card" onClick={() => { markSeen(); loadExample(ex.id); close(); }}>
            <ExamplePreview id={ex.id} />
            <h3>{ex.title}</h3>
            <p>{ex.description}</p>
          </button>
        ))}
      </div>
    </Dialog>
  );
}

function IssueList({ issues }: { issues: GeometryIssue[] }) {
  if (issues.length === 0) return <div className="callout"><IconInfo size={14} /><span>No problems found.</span></div>;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
      {issues.map((i, k) => (
        <div key={k} className={`callout${i.severity === 'error' ? ' callout--danger' : i.severity === 'warning' ? ' callout--warning' : ''}`}>
          {i.severity === 'info' ? <IconInfo size={14} /> : <IconWarning size={14} />}
          <span>{i.message}{i.additionalOccurrences > 0 ? ` (+${i.additionalOccurrences} more)` : ''}</span>
        </div>
      ))}
    </div>
  );
}

function PointsPreview({ points }: { points: Vec2[] }) {
  const d = useMemo(() => {
    if (points.length < 2) return '';
    const xs = points.map((p) => p.x);
    const ys = points.map((p) => p.y);
    const minX = Math.min(...xs), maxX = Math.max(...xs), minY = Math.min(...ys), maxY = Math.max(...ys);
    const s = 280 / Math.max(maxX - minX, maxY - minY, 1e-9);
    const ox = (300 - (maxX - minX) * s) / 2, oy = (140 - (maxY - minY) * s) / 2;
    return points.map((p, i) => `${i ? 'L' : 'M'}${(ox + (p.x - minX) * s).toFixed(1)},${(140 - oy - (p.y - minY) * s).toFixed(1)}`).join('') + 'Z';
  }, [points]);
  return (
    <svg viewBox="0 0 300 140" style={{ width: '100%', height: 140, background: 'var(--canvas-bg)', borderRadius: 6, border: '1px solid var(--border)' }} aria-label="Geometry preview">
      <path d={d} fill="var(--body-fill)" fillOpacity={0.25} stroke="var(--accent)" strokeWidth={1.2} />
    </svg>
  );
}

function ImportDialog() {
  const close = useUIStore((s) => s.closeDialog);
  const toast = useUIStore((s) => s.toast);
  const [text, setText] = useState('');
  const [fileName, setFileName] = useState('');
  const [validation, setValidation] = useState<ValidationResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [count, setCount] = useState(120);
  const [distribution, setDistribution] = useState<PanelDistribution>('auto');
  const [busy, setBusy] = useState(false);
  const [asBezier, setAsBezier] = useState(false);
  /** Fit tolerance as a percentage of the shape's largest dimension. */
  const [tolPct, setTolPct] = useState(0.05);

  useEffect(() => {
    const client = solverRef.client;
    if (!client || !text.trim()) {
      setValidation(null);
      setError(null);
      return;
    }
    const t = setTimeout(() => {
      client.validateGeometry(text).then((v) => { setValidation(v); setError(null); }).catch((e: Error) => { setValidation(null); setError(e.message); });
    }, 150);
    return () => clearTimeout(t);
  }, [text]);

  const readFile = (f: File) => {
    if (f.size > 10 * 1024 * 1024) { setError('The file is larger than 10 MB.'); return; }
    setFileName(f.name);
    f.text().then(setText).catch(() => setError('The file could not be read.'));
  };

  const doImport = async () => {
    const client = solverRef.client;
    if (!client) return;
    setBusy(true);
    try {
      const r = await client.importGeometry(text, count, distribution);
      const name = r.name ?? fileName.replace(/\.[^.]+$/, '') ?? 'Imported body';
      const sim = useSimulationStore.getState();
      // Build the body fully before inserting it, so the import is one undo step.
      let body;
      let approximation = '';
      if (asBezier) {
        // Fit from the cleaned source contour, not the re-panelled one, so the error is measured against what the user supplied.
        const source = validation?.points ?? r.points;
        const bb = boundsOf(source);
        const fit = fitPoints(source, (tolPct / 100) * Math.max(bb.max.x - bb.min.x, bb.max.y - bb.min.y));
        body = createBody(sim.scene, fit.geometry, name);
        body.panels = { count, distribution: 'auto' };
        approximation = ` Imported coordinate geometry has been approximated using Bézier curves (${fit.geometry.nodes.length} nodes, largest deviation ${fit.maxError.toPrecision(2)} m).`;
      } else {
        body = createBody(sim.scene, { kind: 'points', points: r.points }, name);
        body.panels = { count: r.panelCount, distribution: 'asImported' };
      }
      body.sourceName = r.name ?? fileName;
      const id = body.id;
      sim.update((s) => void s.bodies.push(body));
      useUIStore.getState().select([id]);
      toast(`Imported “${name}” with ${asBezier ? count : r.panelCount} panels. ${r.kuttaApplicable ? 'Sharp trailing edge found — Kutta condition applied.' : 'No sharp trailing edge — treated as non-lifting.'}${approximation}`, 'success');
      close();
      setTimeout(fitToScene, 400);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const hasErrors = validation?.hasErrors ?? false;
  return (
    <Dialog
      title="Import geometry"
      footer={
        <>
          <button className="btn" onClick={close}>Cancel</button>
          <button className="btn btn--primary" disabled={!validation || hasErrors || busy} onClick={doImport}>{busy ? 'Importing…' : 'Import'}</button>
        </>
      }
    >
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
        <div>
          <p className="form__hint" style={{ marginTop: 0 }}>
            Paste coordinates or choose a <code>.csv</code>, <code>.dat</code> or <code>.txt</code> file. Selig and Lednicer airfoil formats, CSV headers, comments and Fortran exponents are all understood. The contour is cleaned, oriented and re-panelised; nothing in the file is executed.
          </p>
          <div
            onDragOver={(e) => e.preventDefault()}
            onDrop={(e) => { e.preventDefault(); const f = e.dataTransfer.files[0]; if (f) readFile(f); }}
            style={{ border: '1px dashed var(--border-strong)', borderRadius: 8, padding: 10, marginBottom: 8, textAlign: 'center', fontSize: 'var(--fs-xs)', color: 'var(--text-muted)' }}
          >
            Drop a file here, or <label className="link-btn" style={{ cursor: 'pointer' }}>browse<input type="file" accept=".csv,.dat,.txt,text/plain,text/csv" className="visually-hidden" onChange={(e) => { const f = e.target.files?.[0]; if (f) readFile(f); }} /></label>
            {fileName && <div className="mono" style={{ marginTop: 4 }}>{fileName}</div>}
          </div>
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={'x,y\n1.0000,0.0000\n0.9500,0.0114\n…'}
            style={{ width: '100%', height: 160, fontFamily: 'var(--font-mono)', fontSize: 12, padding: 8, border: '1px solid var(--border)', borderRadius: 6, background: 'var(--surface-2)', resize: 'vertical' }}
            aria-label="Coordinate text"
          />
          <div className="form__section" style={{ marginTop: 10 }}>
            <label className="checkbox"><input type="checkbox" checked={asBezier} onChange={() => setAsBezier((v) => !v)} />Convert to editable Bézier curves</label>
            {asBezier && (
              <>
                <label className="field field--inline"><span className="field__label">Fit tolerance</span><span className="field__input"><input type="number" min={0.001} max={5} step={0.01} value={tolPct} onChange={(e) => setTolPct(Math.max(0.001, Math.min(5, Number(e.target.value))))} aria-label="Approximation tolerance, percent of size" /><span className="field__unit">% of size</span></span></label>
                <p className="form__hint">Imported coordinate geometry will be approximated using Bézier curves. Sharp corners (such as a trailing edge) stay sharp; a smaller tolerance uses more nodes.</p>
              </>
            )}
            <label className="field field--inline"><span className="field__label">Panels</span><span className="field__input"><input type="number" min={16} max={2000} value={count} onChange={(e) => setCount(Math.max(16, Math.min(2000, Number(e.target.value))))} /><span className="field__unit" /></span></label>
            <label className="field field--inline"><span className="field__label">Distribution</span><span className="field__input">
              <select value={distribution} onChange={(e) => setDistribution(e.target.value as PanelDistribution)}>
                <option value="auto">Auto (cosine if sharp edge, else uniform)</option>
                <option value="cosine">Cosine</option>
                <option value="uniform">Uniform</option>
                <option value="curvature">Curvature weighted</option>
                <option value="asImported">As imported</option>
              </select>
            </span></label>
          </div>
        </div>
        <div>
          {validation && <PointsPreview points={validation.points} />}
          {!validation && !error && <div className="card__preview" style={{ height: 140, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--text-faint)' }}>Preview</div>}
          <div style={{ marginTop: 10 }}>
            {error && <div className="callout callout--danger"><IconWarning size={14} /><span>{error}</span></div>}
            {validation && (
              <>
                <p className="form__hint" style={{ margin: '0 0 6px' }}>
                  {validation.points.length} points{validation.name ? ` · “${validation.name}”` : ''}. {validation.notes.join(' ')}
                </p>
                <IssueList issues={validation.issues} />
              </>
            )}
          </div>
        </div>
      </div>
    </Dialog>
  );
}

function GeometryDialog() {
  const close = useUIStore((s) => s.closeDialog);
  const [kind, setKind] = useState<'bezier' | 'naca4' | 'circle' | 'ellipse' | 'joukowski'>('bezier');
  const [template, setTemplate] = useState<BezierTemplate>('naca2412');
  const [code, setCode] = useState('2412');
  const [chord, setChord] = useState(1);
  const [radius, setRadius] = useState(1);
  const [a, setA] = useState(1);
  const [b, setB] = useState(0.5);
  const [thickness, setThickness] = useState(0.1);
  const [camber, setCamber] = useState(0.05);
  const [count, setCount] = useState(140);
  const [preview, setPreview] = useState<Vec2[]>([]);
  const [error, setError] = useState<string | null>(null);

  const geometry: BodyGeometry = useMemo(() => {
    switch (kind) {
      case 'bezier': return templateGeometry(template);
      case 'naca4': return { kind, code, chord };
      case 'circle': return { kind, radius };
      case 'ellipse': return { kind, semiAxisX: a, semiAxisY: b };
      case 'joukowski': return { kind, thickness, camber };
    }
  }, [kind, template, code, chord, radius, a, b, thickness, camber]);

  useEffect(() => {
    // Bézier templates are sampled locally: no solver round trip needed.
    if (geometry.kind === 'bezier') {
      setPreview(sampleBezier(geometry));
      setError(null);
      return;
    }
    const client = solverRef.client;
    if (!client) return;
    const t = setTimeout(() => {
      client.generateGeometry(geometry, count).then((p) => { setPreview(p); setError(null); }).catch((e: Error) => { setError(e.message); setPreview([]); });
    }, 120);
    return () => clearTimeout(t);
  }, [geometry, count]);

  const create = () => {
    const sim = useSimulationStore.getState();
    const vp = useViewportStore.getState();
    const centre = { x: vp.center.x, y: vp.center.y };
    const body = createBody(sim.scene, geometry, undefined, kind === 'naca4' || kind === 'joukowski' ? { x: centre.x - chord / 2, y: centre.y } : centre);
    body.panels = { count, distribution: 'auto' };
    const id = body.id;
    sim.update((s) => void s.bodies.push(body));
    const ui = useUIStore.getState();
    ui.select([id]);
    // A Bézier body opens straight into editing: nodes to drag, or the pen for a blank one.
    if (kind === 'bezier') ui.setTool(template === 'blank' ? 'pen' : 'node');
    close();
  };

  return (
    <Dialog title="Create geometry" footer={<><button className="btn" onClick={close}>Cancel</button><button className="btn btn--primary" onClick={create} disabled={!!error}>Create</button></>}>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
        <div className="form" style={{ padding: 0 }}>
          <div className="segmented" role="radiogroup" aria-label="Shape">
            {(['bezier', 'naca4', 'circle', 'ellipse', 'joukowski'] as const).map((k) => (
              <button key={k} role="radio" aria-checked={kind === k} className={kind === k ? 'is-active' : ''} onClick={() => setKind(k)}>
                {k === 'bezier' ? 'Editable (Bézier)' : k === 'naca4' ? 'NACA 4-digit' : k === 'circle' ? 'Cylinder' : k === 'ellipse' ? 'Ellipse' : 'Joukowski'}
              </button>
            ))}
          </div>
          {kind === 'bezier' && (
            <>
              <label className="field field--inline"><span className="field__label">Template</span><span className="field__input">
                <select value={template} onChange={(e) => setTemplate(e.target.value as BezierTemplate)} aria-label="Bézier template">
                  {(Object.keys(TEMPLATE_LABELS) as BezierTemplate[]).map((t) => <option key={t} value={t}>{TEMPLATE_LABELS[t]}</option>)}
                </select>
              </span></label>
              <p className="form__hint">Built from cubic Bézier curves you can reshape: drag nodes and handles with the Nodes tool and watch the flow respond. “Blank” starts an empty path for the Pen tool.</p>
            </>
          )}
          {kind === 'naca4' && (
            <>
              <label className="field field--inline"><span className="field__label">Code</span><span className="field__input"><input type="text" value={code} maxLength={4} onChange={(e) => setCode(e.target.value.replace(/\D/g, ''))} aria-label="NACA code" /></span></label>
              <label className="field field--inline"><span className="field__label">Chord <span className="sym">c</span></span><span className="field__input"><input type="number" step={0.1} min={0.01} value={chord} onChange={(e) => setChord(Number(e.target.value))} /><span className="field__unit">m</span></span></label>
              <p className="form__hint">First digit: max camber (% chord). Second: its position (tenths). Last two: thickness (% chord). 0012 is symmetric; 2412 is the classic cambered section.</p>
            </>
          )}
          {kind === 'circle' && <label className="field field--inline"><span className="field__label">Radius <span className="sym">r</span></span><span className="field__input"><input type="number" step={0.1} min={0.01} value={radius} onChange={(e) => setRadius(Number(e.target.value))} /><span className="field__unit">m</span></span></label>}
          {kind === 'ellipse' && (
            <>
              <label className="field field--inline"><span className="field__label">Semi-axis <span className="sym">a</span></span><span className="field__input"><input type="number" step={0.1} min={0.01} value={a} onChange={(e) => setA(Number(e.target.value))} /><span className="field__unit">m</span></span></label>
              <label className="field field--inline"><span className="field__label">Semi-axis <span className="sym">b</span></span><span className="field__input"><input type="number" step={0.1} min={0.01} value={b} onChange={(e) => setB(Number(e.target.value))} /><span className="field__unit">m</span></span></label>
            </>
          )}
          {kind === 'joukowski' && (
            <>
              <label className="field field--inline"><span className="field__label">Thickness <span className="sym">ε</span></span><span className="field__input"><input type="number" step={0.01} min={0} value={thickness} onChange={(e) => setThickness(Number(e.target.value))} /><span className="field__unit" /></span></label>
              <label className="field field--inline"><span className="field__label">Camber <span className="sym">δ</span></span><span className="field__input"><input type="number" step={0.01} value={camber} onChange={(e) => setCamber(Number(e.target.value))} /><span className="field__unit" /></span></label>
              <p className="form__hint">Conformal-map airfoil with an exact lift solution. Its cusped trailing edge needs many panels; 400 or more is recommended.</p>
            </>
          )}
          <label className="field field--inline"><span className="field__label">Panels <span className="sym">N</span></span><span className="field__input"><input type="number" min={16} max={2000} value={count} onChange={(e) => setCount(Math.max(16, Math.min(2000, Number(e.target.value))))} /><span className="field__unit" /></span></label>
        </div>
        <div>
          <PointsPreview points={preview} />
          {error && <div className="callout callout--danger" style={{ marginTop: 8 }}><IconWarning size={14} /><span>{error}</span></div>}
          {!error && preview.length > 0 && <p className="form__hint">{preview.length} points. Created at the view centre; drag it afterwards.</p>}
        </div>
      </div>
    </Dialog>
  );
}

function ExportDialog() {
  const scene = useSimulationStore((s) => s.scene);
  const solution = useSolverStore((s) => s.solution);
  const scalar = useSolverStore((s) => s.scalar);
  const toast = useUIStore((s) => s.toast);
  const base = safeFilename(scene.name);
  const exportPng = () => {
    const canvases = Array.from(document.querySelectorAll<HTMLCanvasElement>('.canvas-stack canvas'));
    if (canvases.length === 0) return;
    const out = document.createElement('canvas');
    out.width = canvases[0].width;
    out.height = canvases[0].height;
    const ctx = out.getContext('2d')!;
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--canvas-bg') || '#fff';
    ctx.fillRect(0, 0, out.width, out.height);
    canvases.forEach((c) => ctx.drawImage(c, 0, 0));
    out.toBlob((blob) => blob && downloadBlob(`${base}.png`, blob));
  };
  const Row = ({ label, hint, onClick, disabled }: { label: string; hint: string; onClick: () => void; disabled?: boolean }) => (
    <button className="card" onClick={onClick} disabled={disabled} style={{ opacity: disabled ? 0.5 : 1 }}>
      <h3 style={{ display: 'flex', alignItems: 'center', gap: 6 }}><IconDownload size={14} />{label}</h3>
      <p>{hint}</p>
    </button>
  );
  return (
    <Dialog title="Export" width="wide">
      <div className="card-grid">
        <Row label="Simulation (.aeroflow.json)" hint="The complete scene: flow conditions, elements, bodies, solver settings. Versioned; re-open with File → Open." onClick={() => downloadText(`${base}.aeroflow.json`, serialiseScene(scene), 'application/json')} />
        <Row label="Forces (CSV)" hint="Lift, drag, Fx, Fy, moment, CL, CD, Cm and circulation per body and in total." disabled={!solution || solution.bodies.length === 0} onClick={() => solution && downloadText(`${base}_forces.csv`, forcesCsv(solution), 'text/csv')} />
        <Row label="Surface Cp (CSV)" hint="Panel midpoints with x/c, surface side, tangential velocity, pressure, Cp and source strength — one file per body." disabled={!solution || solution.bodies.length === 0} onClick={() => solution?.bodies.forEach((b) => downloadText(`${base}_${safeFilename(b.name)}_surface.csv`, surfaceCsv(b, scene.conditions), 'text/csv'))} />
        <Row label="Geometry (CSV)" hint="World-space contour of each body as x,y rows, ready to re-import." disabled={!solution || solution.bodies.length === 0} onClick={() => solution?.bodies.forEach((b) => downloadText(`${base}_${safeFilename(b.name)}_geometry.csv`, geometryCsv(b.polygon), 'text/csv'))} />
        <Row label="Field data (CSV)" hint={`The currently displayed scalar field on its sample grid${scalar ? ` (${scalar.nx}×${scalar.ny})` : ''}, with a body mask column.`} disabled={!scalar} onClick={() => scalar && downloadText(`${base}_${scalar.field}.csv`, fieldCsv(scalar), 'text/csv')} />
        <Row label="Bézier outline (CSV)" hint="Sampled world-space outline of each Bézier body, before panelisation." disabled={!scene.bodies.some((b) => b.geometry.kind === 'bezier')} onClick={() => scene.bodies.filter((b) => b.geometry.kind === 'bezier').forEach((b) => downloadText(`${base}_${safeFilename(b.name)}_outline.csv`, geometryCsv(bezierWorldPolygon(b) ?? []), 'text/csv'))} />
        <Row label="Canvas image (PNG)" hint="What you see: field, streamlines, geometry and overlays at screen resolution." onClick={() => { exportPng(); toast('Image exported.', 'success'); }} />
      </div>
      <p className="form__hint" style={{ marginTop: 12 }}>All exports are generated locally in your browser; nothing is uploaded.</p>
    </Dialog>
  );
}

function ShortcutsDialog() {
  return (
    <Dialog title="Keyboard shortcuts" width="narrow">
      <table className="table" aria-label="Shortcuts">
        <tbody>
          {SHORTCUTS.map((s) => (
            <tr key={s.keys}><td style={{ fontFamily: 'var(--font-mono)', whiteSpace: 'normal' }}>{s.keys}</td><td style={{ textAlign: 'left', fontFamily: 'var(--font-sans)' }}>{s.action}</td></tr>
          ))}
        </tbody>
      </table>
    </Dialog>
  );
}

function AboutDialog() {
  const meta = useSolverStore((s) => s.metadata);
  return (
    <Dialog title="About AeroFlow" width="narrow">
      <p>
        AeroFlow is an interactive 2D potential-flow simulator. Elementary singularities are evaluated analytically; arbitrary closed bodies are solved with a
        Hess–Smith constant-strength source/vortex panel method, assembled as one global system for all bodies, with a Kutta condition at sharp trailing edges.
        Bodies can be generated (NACA, cylinder, ellipse, Joukowski), imported from coordinates, or drawn and edited as cubic Bézier curves.
      </p>
      <dl className="kv">
        <dt>Solver core</dt><dd>Rust → WebAssembly</dd><dd className="unit" />
        <dt>Solver version</dt><dd>{meta?.version ?? '—'}</dd><dd className="unit" />
        <dt>Precision</dt><dd>f64 (render buffers f32)</dd><dd className="unit" />
        <dt>Conventions</dt><dd>+x right, +y up, α CCW, Γ CCW&gt;0</dd><dd className="unit" />
      </dl>
      <h3 style={{ fontSize: 'var(--fs-sm)', marginBottom: 4 }}>Validated against</h3>
      <ul style={{ paddingLeft: 18, color: 'var(--text-muted)', margin: 0 }}>
        <li>Analytic cylinder (exact at panel midpoints), ellipse (second-order convergence)</li>
        <li>Kutta–Joukowski lift for a spinning cylinder (−0.4 %)</li>
        <li>NACA 0012 at 5°: CL 0.6025 vs XFOIL ≈ 0.600; NACA 2412 Cm −0.054</li>
        <li>Joukowski conformal-mapping lift (documented first-order convergence)</li>
        <li>Bézier bodies: a circle gives Cp<sub>min</sub> ≈ −3 with no lift; a Bézier NACA 2412 gives CL 0.254 vs 0.254 from the generated section</li>
      </ul>
      <h3 style={{ fontSize: 'var(--fs-sm)', marginBottom: 4 }}>Assumptions</h3>
      <ul style={{ paddingLeft: 18, color: 'var(--text-muted)', margin: 0 }}>
        {(meta?.assumptions ?? []).map((a) => <li key={a}>{a}</li>)}
      </ul>
    </Dialog>
  );
}

export function Dialogs() {
  const dialog = useUIStore((s) => s.dialog);
  const map: Record<DialogKind, () => ReactElement> = {
    welcome: WelcomeDialog,
    examples: ExamplesDialog,
    import: ImportDialog,
    geometry: GeometryDialog,
    about: AboutDialog,
    shortcuts: ShortcutsDialog,
    export: ExportDialog,
    sweep: () => <Dialog title="Sweep">Use the Polar tab.</Dialog>,
  };
  if (!dialog) return null;
  const C = map[dialog];
  return <C />;
}
