import { useEffect, useRef } from 'react';
import { SolverClient } from '@/solver/client';
import { createProbeThrottle, startSolverBridge } from '@/solver/bridge';
import { useSolverStore } from '@/state/solverStore';
import { useUIStore } from '@/state/uiStore';
import { useViewportStore } from '@/state/viewportStore';
import { useSimulationStore } from '@/state/simulationStore';
import { TopBar } from '@/components/TopBar';
import { ObjectsPanel } from '@/components/ObjectsPanel';
import { PropertiesPanel } from '@/components/PropertiesPanel';
import { AnalysisPanel } from '@/components/AnalysisPanel';
import { CanvasView } from '@/canvas/CanvasView';
import { Toasts } from '@/components/Toasts';
import { Dialogs } from '@/components/Dialogs';
import { HelpDrawer } from '@/components/HelpDrawer';
import { useShortcuts } from '@/hooks/useShortcuts';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { EXAMPLES } from '@/domain/examples';
import { fitToScene } from '@/canvas/fit';

/** Module-level handle so non-React code (canvas interaction) can reach the solver. */
export const solverRef: { client: SolverClient | null; probe: ((x: number, y: number) => void) | null } = {
  client: null,
  probe: null,
};

const FIRST_RUN_KEY = 'aeroflow.seen-welcome';

// Development-only handle for end-to-end tests and console debugging.
if (import.meta.env.DEV) {
  (window as unknown as { __aeroflow?: unknown }).__aeroflow = { useSimulationStore, useSolverStore, useUIStore, useViewportStore };
}

export function App() {
  const theme = useUIStore((s) => s.theme);
  const leftOpen = useUIStore((s) => s.leftPanelOpen);
  const rightOpen = useUIStore((s) => s.rightPanelOpen);
  const bottomOpen = useUIStore((s) => s.bottomPanelOpen);
  const panelSizes = useUIStore((s) => s.panelSizes);
  const initError = useSolverStore((s) => s.initError);
  const ready = useSolverStore((s) => s.ready);
  const started = useRef(false);

  useShortcuts();

  // Boot the solver worker once.
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    let stopBridge: (() => void) | null = null;
    let client: SolverClient | null = null;
    SolverClient.create()
      .then((c) => {
        client = c;
        solverRef.client = c;
        solverRef.probe = createProbeThrottle(c);
        useSolverStore.getState().setReady(c.metadata);
        // First run: open a showcase instead of an empty canvas (PRD §57).
        let seen = false;
        try {
          seen = localStorage.getItem(FIRST_RUN_KEY) === '1';
        } catch {
          /* ignore */
        }
        if (!seen) {
          useSimulationStore.getState().loadScene(EXAMPLES[0].build());
          useUIStore.getState().openDialog('welcome');
        } else if (useSimulationStore.getState().scene.elements.length === 0 && useSimulationStore.getState().scene.bodies.length === 0) {
          useSimulationStore.getState().loadScene(EXAMPLES[0].build());
        }
        stopBridge = startSolverBridge(c);
        // Fit the first solved scene to whatever canvas size this device has;
        // the fixed default view only suits a desktop-sized canvas.
        const unsubscribeFit = useSolverStore.subscribe((st) => {
          if (st.solution && st.solution.status !== 'error') {
            unsubscribeFit();
            fitToScene();
          }
        });
      })
      .catch((e: Error) => {
        useSolverStore.getState().setInitError(e.message);
      });
    return () => {
      stopBridge?.();
      client?.dispose();
      solverRef.client = null;
    };
  }, []);

  // Theme: explicit or follow the OS (PRD §39).
  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      const resolved = theme === 'system' ? (mq.matches ? 'dark' : 'light') : theme;
      document.documentElement.dataset.theme = resolved;
    };
    apply();
    mq.addEventListener('change', apply);
    return () => mq.removeEventListener('change', apply);
  }, [theme]);

  // Compact layout on narrow screens (PRD §38).
  useEffect(() => {
    const mq = window.matchMedia('(max-width: 960px)');
    const apply = () => useUIStore.getState().setCompact(mq.matches);
    apply();
    mq.addEventListener('change', apply);
    return () => mq.removeEventListener('change', apply);
  }, []);

  // Warn before leaving with unsaved changes.
  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (useSimulationStore.getState().dirty) e.preventDefault();
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, []);

  return (
    <div
      className="app"
      style={{ '--left-w': `${panelSizes.left}px`, '--right-w': `${panelSizes.right}px`, '--bottom-h': `${panelSizes.bottom}px` } as React.CSSProperties}
      data-left={leftOpen ? 'open' : 'closed'}
      data-right={rightOpen ? 'open' : 'closed'}
      data-bottom={bottomOpen ? 'open' : 'closed'}
    >
      <ErrorBoundary label="toolbar"><TopBar /></ErrorBoundary>
      <div className="app__main">
        <ErrorBoundary label="scene panel"><ObjectsPanel /></ErrorBoundary>
        <main className="app__canvas" aria-label="Simulation canvas">
          <ErrorBoundary label="canvas"><CanvasView /></ErrorBoundary>
          {!ready && !initError && (
            <div className="boot-overlay" role="status">
              <div className="spinner" />
              <p>Loading the WebAssembly solver…</p>
            </div>
          )}
          {initError && (
            <div className="boot-overlay boot-overlay--error" role="alert">
              <h2>The solver could not start</h2>
              <p>{initError}</p>
              <p className="muted">
                AeroFlow needs WebAssembly and Web Workers. Try a current version of Chrome, Firefox, Safari or Edge.
              </p>
            </div>
          )}
        </main>
        <ErrorBoundary label="properties panel"><PropertiesPanel /></ErrorBoundary>
      </div>
      <ErrorBoundary label="analysis panel"><AnalysisPanel /></ErrorBoundary>
      <ErrorBoundary label="dialog"><Dialogs /></ErrorBoundary>
      <HelpDrawer />
      <Toasts />
    </div>
  );
}
