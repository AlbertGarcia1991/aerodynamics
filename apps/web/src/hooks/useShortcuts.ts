/** Keyboard shortcuts (PRD §34). */
import { useEffect } from 'react';
import { useSimulationStore } from '@/state/simulationStore';
import { useUIStore } from '@/state/uiStore';
import { useVisualizationStore } from '@/state/visualizationStore';
import { fitToScene } from '@/canvas/fit';
import { useViewportStore } from '@/state/viewportStore';
import { niceStep } from '@/canvas/viewport';
import { elementPosition, vadd } from '@/domain/scene';
import { deleteNodes, moveNodes, worldToBody } from '@/domain/bezier';
import { editTarget } from '@/canvas/bezierEdit';

function inEditable(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable;
}

export const SHORTCUTS: { keys: string; action: string }[] = [
  { keys: 'Delete / Backspace', action: 'Delete selected objects (or selected Bézier nodes while editing them)' },
  { keys: 'N', action: 'Node tool: select and drag Bézier nodes and handles; click the curve to insert a node' },
  { keys: 'P', action: 'Pen tool: click = node, click-drag = smooth node, click the first node to close' },
  { keys: 'K', action: 'Show / hide the panel mesh' },
  { keys: 'Double-click a node', action: 'Toggle sharp corner ↔ smooth (node tool)' },
  { keys: 'Esc', action: 'Clear selection, cancel placement, close dialogs' },
  { keys: 'Enter (while placing)', action: 'Drop the armed element at the view centre' },
  { keys: 'Space (hold)', action: 'Pan with the pointer' },
  { keys: 'Ctrl/⌘ + Z', action: 'Undo' },
  { keys: 'Ctrl/⌘ + Shift + Z, Ctrl + Y', action: 'Redo' },
  { keys: 'Ctrl/⌘ + D', action: 'Duplicate selection' },
  { keys: 'Ctrl/⌘ + S', action: 'Save simulation' },
  { keys: 'Arrow keys', action: 'Nudge the selection (or selected Bézier nodes) by one grid step (Shift: ×10, Alt: ×0.1)' },
  { keys: 'F', action: 'Fit scene to view' },
  { keys: 'G', action: 'Toggle grid' },
  { keys: 'L', action: 'Toggle streamlines' },
  { keys: 'V', action: 'Toggle velocity vectors' },
  { keys: 'O', action: 'Toggle force arrows' },
  { keys: '1 – 7', action: 'Velocity, no field, pressure, Cp, vorticity, potential, stream function' },
  { keys: 'Scroll / pinch', action: 'Zoom about the cursor' },
  { keys: 'Drag background', action: 'Pan' },
  { keys: 'Shift + drag', action: 'Box select' },
  { keys: '?', action: 'This list' },
];

export function useShortcuts(): void {
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      const ui = useUIStore.getState();
      const sim = useSimulationStore.getState();
      const viz = useVisualizationStore.getState();
      const mod = e.ctrlKey || e.metaKey;

      if (e.key === 'Escape') {
        if (ui.dialog) ui.closeDialog();
        else if (ui.helpTopic) ui.closeHelp();
        else if (ui.pendingAdd) ui.setPendingAdd(null);
        else if (ui.tool !== 'select') ui.setTool('select');
        else ui.clearSelection();
        return;
      }
      if (inEditable(e.target)) return;

      // Keyboard placement (PRD §54): with an element armed, Enter drops it at
      // the view centre so placement never requires a pointer.
      if (e.key === 'Enter' && ui.pendingAdd && !(e.target instanceof HTMLButtonElement)) {
        e.preventDefault();
        const vp = useViewportStore.getState();
        const id = sim.addElement(ui.pendingAdd, { x: vp.center.x, y: vp.center.y });
        ui.setPendingAdd(null);
        ui.select([id]);
        return;
      }

      if (mod && e.key.toLowerCase() === 'z') {
        e.preventDefault();
        if (e.shiftKey) sim.redo();
        else sim.undo();
        return;
      }
      if (mod && e.key.toLowerCase() === 'y') {
        e.preventDefault();
        sim.redo();
        return;
      }
      if (mod && e.key.toLowerCase() === 'd') {
        e.preventDefault();
        const ids = ui.selectedIds.map((id) => sim.duplicateObject(id)).filter((x): x is string => !!x);
        if (ids.length) ui.select(ids);
        return;
      }
      if (mod && e.key.toLowerCase() === 's') {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('aeroflow:save'));
        return;
      }
      if (mod) return;

      const bezier = editTarget(sim.scene, ui.selectedIds);
      const nodesSelected = !!bezier && bezier.geometry.kind === 'bezier' && ui.selectedNodeIds.length > 0;

      if (e.key.startsWith('Arrow') && nodesSelected && bezier && bezier.geometry.kind === 'bezier') {
        // Keyboard alternative to dragging nodes (PRD2 §76); the step is applied in world space, then mapped into the body.
        e.preventDefault();
        const step = (niceStep(useViewportStore.getState().scale, 90) / 5) * (e.shiftKey ? 10 : e.altKey ? 0.1 : 1);
        const dx = e.key === 'ArrowRight' ? step : e.key === 'ArrowLeft' ? -step : 0;
        const dy = e.key === 'ArrowUp' ? step : e.key === 'ArrowDown' ? -step : 0;
        const g = bezier.geometry;
        const delta = worldToBody(bezier, vadd(bezier.position, { x: dx, y: dy }));
        sim.updateBody(bezier.id, (b) => ({ ...b, geometry: moveNodes(g, ui.selectedNodeIds, delta) }));
        return;
      }
      if ((e.key === 'Delete' || e.key === 'Backspace') && nodesSelected && bezier && bezier.geometry.kind === 'bezier') {
        e.preventDefault();
        const next = deleteNodes(bezier.geometry, ui.selectedNodeIds);
        if (!next) ui.toast(`A ${bezier.geometry.closed ? 'closed shape needs at least 3 nodes' : 'path needs at least 2 nodes'}; delete the body instead.`, 'warning');
        else {
          sim.updateBody(bezier.id, (b) => ({ ...b, geometry: next }));
          ui.selectNodes([]);
        }
        return;
      }

      if (e.key.startsWith('Arrow') && ui.selectedIds.length > 0) {
        // Keyboard alternative to dragging (PRD §54): one minor grid step per press.
        e.preventDefault();
        const step = (niceStep(useViewportStore.getState().scale, 90) / 5) * (e.shiftKey ? 10 : 1);
        const dx = e.key === 'ArrowRight' ? step : e.key === 'ArrowLeft' ? -step : 0;
        const dy = e.key === 'ArrowUp' ? step : e.key === 'ArrowDown' ? -step : 0;
        sim.update((scene) => {
          for (const id of ui.selectedIds) {
            scene.elements = scene.elements.map((el) => {
              const p = elementPosition(el.element);
              return el.id === id && p && !el.locked && el.element.type !== 'uniformFlow'
                ? { ...el, element: { ...el.element, position: { x: p.x + dx, y: p.y + dy } } }
                : el;
            });
            scene.bodies = scene.bodies.map((b) => (b.id === id && !b.locked ? { ...b, position: { x: b.position.x + dx, y: b.position.y + dy } } : b));
          }
        });
        return;
      }

      switch (e.key) {
        case 'Delete':
        case 'Backspace':
          if (ui.selectedIds.length) {
            e.preventDefault();
            sim.removeObjects(ui.selectedIds);
            ui.clearSelection();
          }
          break;
        case ' ':
          if (ui.tool !== 'pan') {
            e.preventDefault();
            ui.setTool('pan');
          }
          break;
        case 'f':
        case 'F':
          fitToScene();
          break;
        case 'g':
        case 'G':
          viz.toggle('showGrid');
          break;
        case 'n':
        case 'N':
          ui.setTool(ui.tool === 'node' ? 'select' : 'node');
          break;
        case 'p':
        case 'P':
          ui.setTool(ui.tool === 'pen' ? 'select' : 'pen');
          break;
        case 'k':
        case 'K':
          viz.toggle('showPanels');
          break;
        case 'l':
        case 'L':
          viz.updateStreamlines({ show: !viz.streamlines.show });
          break;
        case 'v':
        case 'V':
          viz.updateVectors({ show: !viz.vectors.show });
          break;
        case 'o':
        case 'O':
          viz.toggle('showForces');
          break;
        case '?':
          ui.openDialog('shortcuts');
          break;
        case '1':
          viz.setField('velocityMagnitude');
          break;
        case '2':
          viz.setField('none');
          break;
        case '3':
          viz.setField('pressure');
          break;
        case '4':
          viz.setField('cp');
          break;
        case '5':
          viz.setField('vorticity');
          break;
        case '6':
          viz.setField('potential');
          break;
        case '7':
          viz.setField('streamFunction');
          break;
      }
    };
    const up = (e: KeyboardEvent) => {
      if (e.key === ' ' && useUIStore.getState().tool === 'pan') useUIStore.getState().setTool('select');
    };
    window.addEventListener('keydown', down);
    window.addEventListener('keyup', up);
    return () => {
      window.removeEventListener('keydown', down);
      window.removeEventListener('keyup', up);
    };
  }, []);
}
