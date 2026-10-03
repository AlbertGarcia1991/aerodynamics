/**
 * Drag handle on a panel edge. Pointer drag resizes; arrow keys resize in
 * 16 px steps (Shift: 64 px); double-click or Home restores the default.
 * Exposed as a WAI-ARIA window splitter (`role="separator"`).
 */
import { useRef } from 'react';
import { PANEL_LIMITS, useUIStore, type ResizablePanel } from '@/state/uiStore';

interface Props {
  panel: ResizablePanel;
  label: string;
}

export function ResizeHandle({ panel, label }: Props) {
  const size = useUIStore((s) => s.panelSizes[panel]);
  const setPanelSize = useUIStore((s) => s.setPanelSize);
  const resetPanelSize = useUIStore((s) => s.resetPanelSize);
  const drag = useRef<{ start: number; size: number } | null>(null);
  const vertical = panel !== 'bottom'; // the handle line is vertical for side panels
  // Every panel grows when its edge is dragged towards the canvas: the left
  // panel to the right, the right panel to the left, the bottom panel upwards.
  const sign = panel === 'left' ? 1 : -1;

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { start: vertical ? e.clientX : e.clientY, size };
    document.body.classList.add(vertical ? 'is-resizing-x' : 'is-resizing-y');
  };
  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    const delta = (vertical ? e.clientX : e.clientY) - d.start;
    setPanelSize(panel, d.size + sign * delta);
  };
  const end = () => {
    drag.current = null;
    document.body.classList.remove('is-resizing-x', 'is-resizing-y');
  };
  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? 64 : 16;
    const grow = vertical ? (panel === 'left' ? 'ArrowRight' : 'ArrowLeft') : 'ArrowUp';
    const shrink = vertical ? (panel === 'left' ? 'ArrowLeft' : 'ArrowRight') : 'ArrowDown';
    if (e.key === grow) setPanelSize(panel, size + step);
    else if (e.key === shrink) setPanelSize(panel, size - step);
    else if (e.key === 'Home') resetPanelSize(panel);
    else return;
    e.preventDefault();
    e.stopPropagation();
  };

  const { min, max } = PANEL_LIMITS[panel];
  return (
    <div
      className={`resize-handle resize-handle--${panel}`}
      role="separator"
      aria-orientation={vertical ? 'vertical' : 'horizontal'}
      aria-label={label}
      aria-valuenow={size}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      title="Drag to resize · double-click to reset"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={end}
      onPointerCancel={end}
      onDoubleClick={() => resetPanelSize(panel)}
      onKeyDown={onKeyDown}
    />
  );
}
