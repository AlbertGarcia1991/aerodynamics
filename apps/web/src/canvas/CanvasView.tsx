/**
 * The simulation canvas: a WebGL2 field layer under a Canvas 2D overlay, with
 * a visualisation toolbar, dynamic legend (PRD §36) and hover read-out.
 */
import { useEffect, useMemo, useRef, useState } from 'react';
import { useSimulationStore } from '@/state/simulationStore';
import { useSolverStore } from '@/state/solverStore';
import { useUIStore } from '@/state/uiStore';
import { useViewportStore } from '@/state/viewportStore';
import { useVisualizationStore, type ColormapId } from '@/state/visualizationStore';
import type { FieldType } from '@/domain/types';
import { FieldLayer } from '@/render/fieldLayer';
import { COLORMAPS, colormapLUT, defaultColormapFor, sampleColormap } from '@/render/colormaps';
import { validateCached } from '@/domain/bezier';
import { drawOverlay, readThemeColors, type ThemeColors } from './overlay';
import { CanvasInteraction, type Cursor } from './interaction';
import { syncBodyCache } from './bodyCache';
import { fitToScene } from './fit';
import { IconFit, IconGrid, IconHand, IconLayers, IconPointer, IconSeed } from '@/components/icons';

/** Stable empty fallbacks: zustand selectors must not return a fresh array each call. */
const EMPTY: never[] = [];

/** Colour range for the current field, honouring the range mode and field character. */
export function fieldRange(
  scalar: { robustMin: number; robustMax: number; min: number; max: number } | null,
  viz: { rangeMode: 'robust' | 'minmax' | 'manual'; manualMin: number; manualMax: number },
  diverging: boolean,
): [number, number] {
  if (viz.rangeMode === 'manual') return [viz.manualMin, viz.manualMax];
  if (!scalar) return [0, 1];
  let lo = viz.rangeMode === 'robust' ? scalar.robustMin : scalar.min;
  let hi = viz.rangeMode === 'robust' ? scalar.robustMax : scalar.max;
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) return [0, 1];
  if (diverging) {
    const a = Math.max(Math.abs(lo), Math.abs(hi), 1e-12);
    lo = -a;
    hi = a;
  }
  if (hi - lo < 1e-12) {
    lo -= 0.5;
    hi += 0.5;
  }
  return [lo, hi];
}

function parseCssColor(c: string): [number, number, number] {
  const m = c.match(/rgba?\(([^)]+)\)/);
  if (m) {
    const [r, g, b] = m[1].split(',').map((v) => parseFloat(v));
    return [r / 255, g / 255, b / 255];
  }
  const h = c.replace('#', '');
  if (h.length >= 6) return [parseInt(h.slice(0, 2), 16) / 255, parseInt(h.slice(2, 4), 16) / 255, parseInt(h.slice(4, 6), 16) / 255];
  return [0.2, 0.2, 0.2];
}

function Legend({ range }: { range: [number, number] }) {
  const field = useVisualizationStore((s) => s.field);
  const colormap = useVisualizationStore((s) => s.colormap);
  const scalar = useSolverStore((s) => s.scalar);
  const info = useSolverStore((s) => s.metadata?.fieldTypes.find((f) => f.id === field));
  const preview = useSolverStore((s) => s.preview);
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const ctx = c.getContext('2d')!;
    c.width = 256;
    c.height = 1;
    const img = ctx.createImageData(256, 1);
    for (let i = 0; i < 256; i++) {
      const [r, g, b] = sampleColormap(colormap, i / 255);
      img.data.set([r * 255, g * 255, b * 255, 255], i * 4);
    }
    ctx.putImageData(img, 0, 0);
  }, [colormap]);
  if (field === 'none' || !info) return null;
  const fmt = (v: number) => (Math.abs(v) >= 1e4 || (Math.abs(v) < 1e-2 && v !== 0) ? v.toExponential(1) : v.toFixed(Math.abs(range[1] - range[0]) < 1 ? 3 : 2));
  const ticks = [0, 0.25, 0.5, 0.75, 1].map((t) => range[0] + t * (range[1] - range[0]));
  return (
    <div className="legend" role="group" aria-label={`Legend: ${info.label}`}>
      <div className="legend__title">
        <span>{info.label} <span className="mono">{info.symbol}</span></span>
        <span className="unit">{info.unit}</span>
      </div>
      <canvas ref={ref} aria-hidden="true" />
      <div className="legend__ticks">
        {ticks.map((t, i) => <span key={i}>{fmt(t)}</span>)}
      </div>
      <div className="legend__note">
        {scalar?.softened ? 'Singularity cores softened for display. ' : ''}
        {preview ? 'Preview resolution — refining… ' : ''}
        {info.note}
      </div>
    </div>
  );
}

/**
 * Solver freshness and geometry validity (PRD2 §50, §62). The previous solution
 * stays on screen while a new one is computed, but never silently: it is marked
 * as updating until a result for the current scene revision arrives.
 */
function StatusBadges() {
  const scene = useSimulationStore((s) => s.scene);
  const sceneRev = useSimulationStore((s) => s.revision);
  const solvedRev = useSolverStore((s) => s.solvedRevision);
  const status = useSolverStore((s) => s.status);
  const stale = status !== 'error' && (solvedRev !== sceneRev || status === 'running');
  const invalid = scene.bodies.filter((b) => b.geometry.kind === 'bezier' && validateCached(b.geometry).status === 'invalid');
  const open = scene.bodies.filter((b) => b.geometry.kind === 'bezier' && validateCached(b.geometry).status === 'open');
  if (!stale && invalid.length === 0 && open.length === 0) return null;
  return (
    <div className="status-badges" role="status" aria-live="polite">
      {invalid.length > 0 && <span className="badge badge--danger">Invalid geometry: {invalid.map((b) => b.name).join(', ')} excluded from the solve</span>}
      {open.length > 0 && <span className="badge badge--warning">Open path: {open.map((b) => b.name).join(', ')} not solved until closed</span>}
      {stale && <span className="badge">Updating flow…</span>}
    </div>
  );
}

function ProbeReadout() {
  const probe = useSolverStore((s) => s.probe);
  const scene = useSimulationStore((s) => s.scene);
  if (!probe) return null;
  const f = (v: number | null, d = 3) => (v === null || !Number.isFinite(v) ? '—' : Math.abs(v) >= 1e4 ? v.toExponential(2) : v.toFixed(d));
  const bodyName = probe.bodyId ? scene.bodies.find((b) => b.id === probe.bodyId)?.name : null;
  return (
    <div className="probe" aria-live="polite" aria-label="Flow at the cursor">
      <span>x, y</span><b>{f(probe.position.x)}, {f(probe.position.y)} m</b>
      {bodyName ? (
        <><span>inside</span><b>{bodyName}</b></>
      ) : (
        <>
          <span>|V|</span><b>{f(probe.speed)} m/s</b>
          <span>u, v</span><b>{f(probe.velocity.x)}, {f(probe.velocity.y)}</b>
          <span>Cp</span><b>{f(probe.cp, 4)}</b>
          <span>p − p∞</span><b>{f(probe.pressure - scene.conditions.pressure, 1)} Pa</b>
          <span>ψ</span><b>{f(probe.streamFunction)}</b>
        </>
      )}
    </div>
  );
}

function VizToolbar() {
  const viz = useVisualizationStore();
  const fields = useSolverStore((s) => s.metadata?.fieldTypes ?? EMPTY);
  const tool = useUIStore((s) => s.tool);
  const setTool = useUIStore((s) => s.setTool);
  const openHelp = useUIStore((s) => s.openHelp);
  const [more, setMore] = useState(false);
  const setField = (id: FieldType | 'none') => {
    viz.setField(id);
    const info = fields.find((f) => f.id === id);
    if (info) {
      const divergingMap = COLORMAPS.find((c) => c.id === viz.colormap)?.diverging ?? false;
      if (info.diverging !== divergingMap) viz.setColormap(defaultColormapFor(info.diverging));
    }
  };
  const fieldHelp: Partial<Record<FieldType, string>> = { cp: 'cp', pressure: 'cp', vorticity: 'vorticity', streamFunction: 'streamlines', velocityMagnitude: 'streamlines' };
  return (
    <div className="viz-toolbar" role="toolbar" aria-label="Visualisation">
      <div className="viz-toolbar__row">
        <button className={`btn${viz.field === 'none' ? ' is-active' : ''}`} onClick={() => setField('none')} title="No field (2)">None</button>
        {fields.map((f) => (
          <button key={f.id} className={`btn${viz.field === f.id ? ' is-active' : ''}`} onClick={() => setField(f.id)} title={f.note}>
            {f.id === 'velocityMagnitude' ? 'Velocity' : f.id === 'velocityU' ? 'u' : f.id === 'velocityV' ? 'v' : f.id === 'streamFunction' ? 'ψ' : f.id === 'potential' ? 'φ' : f.label}
          </button>
        ))}
        {viz.field !== 'none' && fieldHelp[viz.field] && (
          <button className="btn" onClick={() => openHelp(fieldHelp[viz.field as FieldType]!)} title="Explain this field">?</button>
        )}
      </div>
      <div className="viz-toolbar__row">
        <button className={`btn${viz.streamlines.show ? ' is-active' : ''}`} onClick={() => viz.updateStreamlines({ show: !viz.streamlines.show })} title="Streamlines (L)">Streamlines</button>
        <button className={`btn${viz.streamlines.particles ? ' is-active' : ''}`} onClick={() => viz.updateStreamlines({ particles: !viz.streamlines.particles })} title="Animate particles along streamlines" disabled={!viz.streamlines.show}>Particles</button>
        <button className={`btn${viz.vectors.show ? ' is-active' : ''}`} onClick={() => viz.updateVectors({ show: !viz.vectors.show })} title="Velocity vectors (V)">Vectors</button>
        <button className={`btn${viz.showForces ? ' is-active' : ''}`} onClick={() => viz.toggle('showForces')} title="Force arrows on bodies and elements (O)" aria-pressed={viz.showForces}>Forces</button>
        <button className={`btn${viz.showPanels ? ' is-active' : ''}`} onClick={() => viz.toggle('showPanels')} title="Show the panel mesh: vertices, normals, indices (K)" aria-pressed={viz.showPanels}>Panels</button>
        <button className={`btn${viz.showGrid ? ' is-active' : ''}`} onClick={() => viz.toggle('showGrid')} title="Grid (G)"><IconGrid size={14} /></button>
        <button className={`btn${more ? ' is-active' : ''}`} onClick={() => setMore((m) => !m)} title="More display options"><IconLayers size={14} /></button>
        <span style={{ width: 1, background: 'var(--border)', margin: '2px 2px' }} />
        <button className={`btn${tool === 'select' ? ' is-active' : ''}`} onClick={() => setTool('select')} title="Select / drag (Esc)"><IconPointer size={14} /></button>
        <button className={`btn${tool === 'pan' ? ' is-active' : ''}`} onClick={() => setTool('pan')} title="Pan (hold Space)"><IconHand size={14} /></button>
        <button className={`btn${tool === 'seed' ? ' is-active' : ''}`} onClick={() => setTool(tool === 'seed' ? 'select' : 'seed')} title="Place streamline seeds"><IconSeed size={14} /></button>
        <button className={`btn${tool === 'node' ? ' is-active' : ''}`} onClick={() => setTool(tool === 'node' ? 'select' : 'node')} title="Edit Bézier nodes and handles (N)" aria-pressed={tool === 'node'}>Nodes</button>
        <button className={`btn${tool === 'pen' ? ' is-active' : ''}`} onClick={() => setTool(tool === 'pen' ? 'select' : 'pen')} title="Pen: click for a node, click-drag for a smooth node, click the first node to close (P)" aria-pressed={tool === 'pen'}>Pen</button>
        <button className="btn" onClick={fitToScene} title="Fit to scene (F)"><IconFit size={14} /></button>
      </div>
      {more && (
        <div className="viz-toolbar__row" style={{ gap: 10, padding: '8px 10px', alignItems: 'center' }}>
          <label className="form__hint" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            Colour map
            <select className="input" value={viz.colormap} onChange={(e) => viz.setColormap(e.target.value as ColormapId)} style={{ height: 24 }}>
              {COLORMAPS.map((c) => <option key={c.id} value={c.id}>{c.label}</option>)}
            </select>
          </label>
          <label className="form__hint" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            Range
            <select className="input" value={viz.rangeMode} onChange={(e) => viz.setRange(e.target.value as 'robust' | 'minmax' | 'manual')} style={{ height: 24 }}>
              <option value="robust">Robust (1–99 %)</option>
              <option value="minmax">Min – max</option>
              <option value="manual">Manual</option>
            </select>
          </label>
          {viz.rangeMode === 'manual' && (
            <>
              <input className="input" type="number" value={viz.manualMin} onChange={(e) => viz.setRange('manual', Number(e.target.value), undefined)} style={{ width: 80, height: 24 }} aria-label="Range minimum" />
              <input className="input" type="number" value={viz.manualMax} onChange={(e) => viz.setRange('manual', undefined, Number(e.target.value))} style={{ width: 80, height: 24 }} aria-label="Range maximum" />
            </>
          )}
          <label className="form__hint" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            Opacity
            <input type="range" min={0.1} max={1} step={0.05} value={viz.fieldOpacity} onChange={(e) => viz.setFieldOpacity(Number(e.target.value))} aria-label="Field opacity" />
          </label>
          <label className="form__hint" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            Bands
            <input type="range" min={0} max={24} step={1} value={viz.contourBands} onChange={(e) => viz.setContourBands(Number(e.target.value))} aria-label="Contour bands (0 = smooth)" />
            <span className="mono">{viz.contourBands || 'smooth'}</span>
          </label>
          <label className="checkbox"><input type="checkbox" checked={viz.isolines} disabled={viz.contourBands === 0} onChange={() => viz.setIsolines(!viz.isolines)} />Isolines</label>
          <label className="checkbox"><input type="checkbox" checked={viz.streamlines.colorBySpeed} onChange={() => viz.updateStreamlines({ colorBySpeed: !viz.streamlines.colorBySpeed })} />Colour streamlines by speed</label>
          <label className="form__hint" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            Streamline spacing
            <input type="range" min={0.015} max={0.1} step={0.005} value={viz.streamlines.separation} onChange={(e) => viz.updateStreamlines({ separation: Number(e.target.value) })} aria-label="Streamline separation" />
          </label>
          <label className="form__hint" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            Seeding
            <select className="input" value={viz.streamlines.strategy} onChange={(e) => viz.updateStreamlines({ strategy: e.target.value as 'inflow' | 'grid' | 'evenlySpaced' })} style={{ height: 24 }}>
              <option value="evenlySpaced">Evenly spaced</option>
              <option value="inflow">Inflow rake</option>
              <option value="grid">Grid</option>
            </select>
          </label>
          <label className="form__hint" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            Vector spacing
            <input type="range" min={20} max={90} step={2} value={viz.vectors.spacingPx} onChange={(e) => viz.updateVectors({ spacingPx: Number(e.target.value) })} aria-label="Vector spacing" />
          </label>
          <label className="form__hint" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            Resolution
            <select className="input" value={String(viz.resolution)} onChange={(e) => viz.setResolution(e.target.value === 'auto' ? 'auto' : (Number(e.target.value) as 128 | 192 | 256 | 384))} style={{ height: 24 }}>
              <option value="auto">Auto</option><option value="128">128</option><option value="192">192</option><option value="256">256</option><option value="384">384</option>
            </select>
          </label>
          {viz.seeds.length > 0 && <button className="btn btn--sm" onClick={viz.clearSeeds}>Clear {viz.seeds.length} seed{viz.seeds.length > 1 ? 's' : ''}</button>}
        </div>
      )}
    </div>
  );
}

export function CanvasView() {
  const containerRef = useRef<HTMLDivElement>(null);
  const glRef = useRef<HTMLCanvasElement>(null);
  const overlayRef = useRef<HTMLCanvasElement>(null);
  const [cursor, setCursor] = useState<Cursor>('default');
  const [glSupported, setGlSupported] = useState(true);
  const scene = useSimulationStore((s) => s.scene);
  const scalar = useSolverStore((s) => s.scalar);
  const rangeMode = useVisualizationStore((s) => s.rangeMode);
  const manualMin = useVisualizationStore((s) => s.manualMin);
  const manualMax = useVisualizationStore((s) => s.manualMax);
  const field = useVisualizationStore((s) => s.field);
  const fieldInfo = useSolverStore((s) => s.metadata?.fieldTypes.find((f) => f.id === field));
  const range = useMemo(
    () => fieldRange(scalar, { rangeMode, manualMin, manualMax }, fieldInfo?.diverging ?? false),
    [scalar, rangeMode, manualMin, manualMax, fieldInfo],
  );
  const pendingAdd = useUIStore((s) => s.pendingAdd);
  const tool = useUIStore((s) => s.tool);
  // Hide the empty-state card while the user is placing something, so the
  // centre of the canvas is clickable.
  const empty = scene.elements.length === 0 && scene.bodies.length === 0 && !pendingAdd && tool === 'select';
  const openDialog = useUIStore((s) => s.openDialog);

  useEffect(() => {
    const container = containerRef.current!;
    const glCanvas = glRef.current!;
    const overlay = overlayRef.current!;
    const ctx = overlay.getContext('2d')!;
    let layer: FieldLayer | null = null;
    try {
      layer = new FieldLayer(glCanvas);
      setGlSupported(layer.supported);
    } catch (e) {
      console.error(e);
      setGlSupported(false);
    }

    let dirty = true;
    let colors: ThemeColors = readThemeColors();
    let lastLut: ColormapId | null = null;
    let lastScalar: unknown = null;
    const markDirty = () => {
      dirty = true;
    };
    const interaction = new CanvasInteraction(container, () => {
      markDirty();
      setCursor(interaction.view().cursor);
    });

    const ro = new ResizeObserver(() => {
      const w = container.clientWidth;
      const h = container.clientHeight;
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      overlay.width = Math.round(w * dpr);
      overlay.height = Math.round(h * dpr);
      layer?.resize(w, h, dpr);
      useViewportStore.getState().setSize(w, h);
      markDirty();
    });
    ro.observe(container);

    const unsubs = [
      useSimulationStore.subscribe(markDirty),
      useViewportStore.subscribe(markDirty),
      useVisualizationStore.subscribe(markDirty),
      useUIStore.subscribe(markDirty),
      useSolverStore.subscribe((s) => {
        syncBodyCache(s.solution, useSimulationStore.getState().scene.bodies);
        markDirty();
      }),
    ];
    const themeObserver = new MutationObserver(() => {
      colors = readThemeColors();
      markDirty();
    });
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    const reducedMotionMq = window.matchMedia('(prefers-reduced-motion: reduce)');

    let raf = 0;
    const frame = (time: number) => {
      raf = requestAnimationFrame(frame);
      const viz = useVisualizationStore.getState();
      const solver = useSolverStore.getState();
      const animating = viz.streamlines.show && viz.streamlines.particles && !reducedMotionMq.matches && !!solver.streamlines;
      if (!dirty && !animating) return;
      dirty = false;
      const vp = useViewportStore.getState();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);

      if (layer) {
        if (viz.field !== 'none' && solver.scalar && solver.scalarGrid) {
          if (solver.scalar !== lastScalar) {
            layer.setField(solver.scalar);
            lastScalar = solver.scalar;
          }
          if (lastLut !== viz.colormap) {
            layer.setColormap(colormapLUT(viz.colormap));
            lastLut = viz.colormap;
          }
          const info = solver.metadata?.fieldTypes.find((f) => f.id === viz.field);
          layer.render({
            vp,
            bounds: solver.scalarGrid.bounds,
            range: fieldRange(solver.scalar, viz, info?.diverging ?? false),
            opacity: viz.fieldOpacity,
            bands: viz.contourBands,
            isolines: viz.isolines,
            isoColor: parseCssColor(colors.text),
          });
        } else {
          layer.clear();
        }
      }

      const ui = useUIStore.getState();
      drawOverlay(ctx, {
        vp,
        dpr,
        scene: useSimulationStore.getState().scene,
        selectedIds: new Set(ui.selectedIds),
        hoverId: ui.hoverId,
        viz,
        streamlines: viz.streamlines.show ? solver.streamlines : null,
        vectors: viz.vectors.show ? solver.vectors : null,
        vectorBounds: solver.vectorGrid?.bounds ?? null,
        time,
        colors,
        drag: { box: interaction.view().box, pointer: interaction.pointer },
        pendingAdd: ui.pendingAdd,
        reducedMotion: reducedMotionMq.matches,
        solution: solver.solution,
        edit: { tool: ui.tool, selectedNodeIds: new Set(ui.selectedNodeIds), insertHint: interaction.view().insertHint },
      });
    };
    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      themeObserver.disconnect();
      unsubs.forEach((u) => u());
      interaction.dispose();
      layer?.dispose();
    };
  }, []);

  return (
    <div className="canvas-stack" ref={containerRef} data-cursor={cursor} role="application" aria-label="Flow canvas. Drag objects, scroll to zoom, drag the background to pan.">
      <canvas ref={glRef} aria-hidden="true" />
      <canvas ref={overlayRef} aria-hidden="true" />
      <VizToolbar />
      <StatusBadges />
      <div className="canvas-hud">
        <ProbeReadout />
        {useVisualizationStore.getState().showLegend && <Legend range={range} />}
      </div>
      {empty && (
        <div className="canvas-empty">
          <div>
            <p>Add a flow element or import a geometry to begin.</p>
            <button className="btn btn--primary" onClick={() => openDialog('examples')}>Browse examples</button>
          </div>
        </div>
      )}
      {!glSupported && (
        <div className="canvas-empty" style={{ alignItems: 'flex-end', paddingBottom: 60 }}>
          <div>WebGL2 is unavailable, so the colour field cannot be shown. Streamlines, vectors and results still work.</div>
        </div>
      )}
    </div>
  );
}
