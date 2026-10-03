import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useSimulationStore, selectCanRedo, selectCanUndo } from '@/state/simulationStore';
import { useUIStore, type Theme } from '@/state/uiStore';
import { useSolverStore } from '@/state/solverStore';
import { parseScene, serialiseScene, createScene } from '@/domain/scene';
import type { ElementKind } from '@/domain/types';
import { downloadText, safeFilename } from '@/export/download';
import { useViewportStore } from '@/state/viewportStore';
import { screenToWorld } from '@/canvas/viewport';
import { fitToScene } from '@/canvas/fit';
import {
  IconChevronDown,
  IconDownload,
  IconFolder,
  IconHelp,
  IconMonitor,
  IconMoon,
  IconPanelBottom,
  IconPlus,
  IconRedo,
  IconSave,
  IconSidebarLeft,
  IconSidebarRight,
  IconSun,
  IconUndo,
  IconLayers,
} from './icons';

/** Stable empty fallbacks: zustand selectors must not return a fresh array each call. */
const EMPTY: never[] = [];

function Menu({ label, icon, children, align = 'left', primary = false, ariaLabel }: {
  label: ReactNode;
  ariaLabel?: string;
  icon?: ReactNode;
  children: (close: () => void) => ReactNode;
  align?: 'left' | 'right';
  primary?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(false);
    document.addEventListener('mousedown', onDoc);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);
  return (
    <div className="menu-anchor" ref={ref}>
      <button className={`btn${primary ? ' btn--primary' : ''}`} onClick={() => setOpen((o) => !o)} aria-haspopup="menu" aria-expanded={open} aria-label={ariaLabel}>
        {icon}
        {label && <span className="btn__label">{label}</span>}
        <IconChevronDown size={14} />
      </button>
      {open && (
        <div className={`menu${align === 'right' ? ' menu--right' : ''}`} role="menu">
          {children(() => setOpen(false))}
        </div>
      )}
    </div>
  );
}

function StatusIndicator() {
  const status = useSolverStore((s) => s.status);
  const ms = useSolverStore((s) => s.lastSolveMs);
  const sample = useSolverStore((s) => s.lastSampleMs);
  const panels = useSolverStore((s) => s.solution?.diagnostics.panel?.panelCount ?? 0);
  const label =
    status === 'running' ? 'Solving…' : status === 'error' ? 'Error' : status === 'warning' ? 'Solved · warnings' : status === 'success' ? 'Solved' : 'Idle';
  return (
    <span className="topbar__status" title="Solver status · solve time · field sampling time" role="status" aria-live="polite">
      <span className={`status-dot status-dot--${status}`} />
      <span className="topbar__status-text">{label}</span>
      {status !== 'idle' && status !== 'running' && (
        <span className="muted">
          {panels > 0 ? `${panels} panels · ` : ''}
          {ms < 1 ? '<1' : ms.toFixed(ms < 10 ? 1 : 0)} ms · field {sample.toFixed(0)} ms
        </span>
      )}
    </span>
  );
}

export function TopBar() {
  const name = useSimulationStore((s) => s.scene.name);
  const dirty = useSimulationStore((s) => s.dirty);
  const canUndo = useSimulationStore(selectCanUndo);
  const canRedo = useSimulationStore(selectCanRedo);
  const theme = useUIStore((s) => s.theme);
  const descriptors = useSolverStore((s) => s.metadata?.descriptors ?? EMPTY);
  const ready = useSolverStore((s) => s.ready);
  const fileRef = useRef<HTMLInputElement>(null);

  const save = () => {
    const scene = useSimulationStore.getState().scene;
    downloadText(`${safeFilename(scene.name)}.aeroflow.json`, serialiseScene(scene), 'application/json');
    useSimulationStore.getState().markSaved();
    useUIStore.getState().toast('Simulation saved.', 'success');
  };

  useEffect(() => {
    const h = () => save();
    window.addEventListener('aeroflow:save', h);
    return () => window.removeEventListener('aeroflow:save', h);
  }, []);

  const openFile = (file: File) => {
    file
      .text()
      .then((text) => {
        const scene = parseScene(text);
        useSimulationStore.getState().loadScene(scene);
        useUIStore.getState().clearSelection();
        useUIStore.getState().toast(`Loaded “${scene.name}”.`, 'success');
        setTimeout(fitToScene, 300);
      })
      .catch((e: Error) => useUIStore.getState().toast(e.message, 'error'));
  };

  const addElement = (kind: ElementKind) => {
    const ui = useUIStore.getState();
    if (kind === 'uniformFlow') {
      const id = useSimulationStore.getState().addElement('uniformFlow', { x: 0, y: 0 });
      ui.select([id]);
      return;
    }
    // Arm placement: the next canvas click places the element. Also offer an
    // immediate drop at the view centre for keyboard users.
    ui.setPendingAdd(kind);
    ui.toast(`Click on the canvas to place the ${kind} — or press Enter to drop it at the centre. Esc cancels.`, 'info');
  };

  const addAtCentre = (kind: ElementKind) => {
    const vp = useViewportStore.getState();
    const p = screenToWorld({ x: vp.width / 2, y: vp.height / 2 }, vp);
    const id = useSimulationStore.getState().addElement(kind, p);
    useUIStore.getState().select([id]);
  };

  const cycleTheme = () => {
    const next: Record<Theme, Theme> = { system: 'light', light: 'dark', dark: 'system' };
    useUIStore.getState().setTheme(next[theme]);
  };

  return (
    <header className="topbar" role="banner">
      <div className="topbar__brand" aria-label="AeroFlow">
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <rect width="32" height="32" rx="7" fill="var(--text)" />
          <path d="M4 11c6-4 12-4 24 0M4 16c6-4 12-4 24 0M4 21c6-4 12-4 24 0" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" />
          <ellipse cx="17" cy="16" rx="7" ry="2.6" fill="var(--surface)" />
        </svg>
        <span className="topbar__brand-text">AeroFlow</span>
      </div>
      <input
        className="topbar__name"
        value={name}
        onChange={(e) => useSimulationStore.getState().update((s) => void (s.name = e.target.value), { transient: true })}
        aria-label="Simulation name"
      />
      {dirty && <span className="muted" title="Unsaved changes" aria-label="Unsaved changes">●</span>}

      <Menu label="Add" ariaLabel="Add" icon={<IconPlus size={15} />} primary>
        {(close) => (
          <>
            <div className="menu__group">Elementary</div>
            {descriptors.map((d) => (
              <button
                key={d.kind}
                className="menu__item"
                role="menuitem"
                aria-label={d.name}
                onClick={(e) => {
                  close();
                  if (e.altKey) addAtCentre(d.kind);
                  else addElement(d.kind);
                }}
                title={`${d.summary} (Alt+click to drop at the view centre)`}
              >
                <span className="glyph">{d.glyph}</span>
                {d.name}
                <small>{d.summary.split('.')[0].slice(0, 28)}</small>
              </button>
            ))}
            {descriptors.length === 0 && <div className="menu__item muted">Loading solver…</div>}
            <div className="menu__sep" />
            <div className="menu__group">Geometry</div>
            <button className="menu__item" role="menuitem" aria-label="Create geometry" onClick={() => { close(); useUIStore.getState().openDialog('geometry'); }}>
              <span className="glyph">◇</span>
              Airfoil / cylinder / ellipse…
            </button>
            <button className="menu__item" role="menuitem" aria-label="Import coordinates" onClick={() => { close(); useUIStore.getState().openDialog('import'); }}>
              <span className="glyph">⤓</span>
              Import coordinates (.csv, .dat)…
            </button>
          </>
        )}
      </Menu>

      <button className="btn" onClick={() => useUIStore.getState().openDialog('examples')} aria-label="Examples">
        <IconLayers size={15} />
        <span className="btn__label">Examples</span>
      </button>

      <Menu label="File" ariaLabel="File" icon={<IconFolder size={15} />}>
        {(close) => (
          <>
            <button className="menu__item" role="menuitem" onClick={() => { close(); useSimulationStore.getState().loadScene(createScene()); useUIStore.getState().clearSelection(); }}>
              <span className="glyph"><IconPlus size={14} /></span>New simulation
            </button>
            <button className="menu__item" role="menuitem" onClick={() => { close(); fileRef.current?.click(); }}>
              <span className="glyph"><IconFolder size={14} /></span>Open .aeroflow.json…
            </button>
            <button className="menu__item" role="menuitem" onClick={() => { close(); save(); }}>
              <span className="glyph"><IconSave size={14} /></span>Save<small>Ctrl+S</small>
            </button>
            <div className="menu__sep" />
            <button className="menu__item" role="menuitem" onClick={() => { close(); useUIStore.getState().openDialog('export'); }}>
              <span className="glyph"><IconDownload size={14} /></span>Export results…
            </button>
          </>
        )}
      </Menu>
      <input
        ref={fileRef}
        type="file"
        accept=".json,application/json"
        className="visually-hidden"
        aria-label="Open simulation file"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) openFile(f);
          e.target.value = '';
        }}
      />

      <div className="topbar__group">
        <button className="icon-btn" onClick={() => useSimulationStore.getState().undo()} disabled={!canUndo} aria-label="Undo" title="Undo (Ctrl+Z)">
          <IconUndo />
        </button>
        <button className="icon-btn" onClick={() => useSimulationStore.getState().redo()} disabled={!canRedo} aria-label="Redo" title="Redo (Ctrl+Y)">
          <IconRedo />
        </button>
      </div>

      <div className="topbar__spacer" />
      {ready && <StatusIndicator />}

      <div className="topbar__group">
        <button className="icon-btn" onClick={() => useUIStore.getState().togglePanel('left')} aria-label="Toggle scene panel" title="Scene panel">
          <IconSidebarLeft />
        </button>
        <button className="icon-btn" onClick={() => useUIStore.getState().togglePanel('bottom')} aria-label="Toggle analysis panel" title="Analysis panel">
          <IconPanelBottom />
        </button>
        <button className="icon-btn" onClick={() => useUIStore.getState().togglePanel('right')} aria-label="Toggle properties panel" title="Properties panel">
          <IconSidebarRight />
        </button>
      </div>
      <div className="topbar__group">
        <button className="icon-btn" onClick={cycleTheme} aria-label={`Theme: ${theme}. Click to change.`} title={`Theme: ${theme}`}>
          {theme === 'light' ? <IconSun /> : theme === 'dark' ? <IconMoon /> : <IconMonitor />}
        </button>
        <Menu label="" ariaLabel="Help" icon={<IconHelp size={16} />} align="right">
          {(close) => (
            <>
              <button className="menu__item" role="menuitem" onClick={() => { close(); useUIStore.getState().openHelp('assumptions'); }}>Model assumptions</button>
              <button className="menu__item" role="menuitem" onClick={() => { close(); useUIStore.getState().openHelp('panelMethod'); }}>How the solver works</button>
              <button className="menu__item" role="menuitem" onClick={() => { close(); useUIStore.getState().openDialog('shortcuts'); }}>Keyboard shortcuts<small>?</small></button>
              <div className="menu__sep" />
              <button className="menu__item" role="menuitem" onClick={() => { close(); useUIStore.getState().openDialog('welcome'); }}>Welcome tour</button>
              <button className="menu__item" role="menuitem" onClick={() => { close(); useUIStore.getState().openDialog('about'); }}>About AeroFlow</button>
            </>
          )}
        </Menu>
      </div>
    </header>
  );
}
