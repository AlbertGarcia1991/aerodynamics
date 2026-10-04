/** UIState (PRD §49): selection, tools, panels, theme, dialogs, toasts. */
import { create } from 'zustand';
import type { ElementKind } from '@/domain/types';

export type Theme = 'light' | 'dark' | 'system';
/** `node` edits Bézier nodes/handles, `pen` draws new ones (PRD2 §17). */
export type Tool = 'select' | 'pan' | 'seed' | 'node' | 'pen';
export type BottomTab = 'surface' | 'polar' | 'diagnostics' | 'data';
export type DialogKind = 'welcome' | 'examples' | 'import' | 'geometry' | 'about' | 'shortcuts' | 'export' | 'sweep';

export interface Toast {
  id: number;
  kind: 'info' | 'success' | 'warning' | 'error';
  message: string;
}

export type ResizablePanel = 'left' | 'right' | 'bottom';

/** Default, minimum and maximum size of each resizable panel, in CSS px. */
export const PANEL_LIMITS: Record<ResizablePanel, { initial: number; min: number; max: number }> = {
  left: { initial: 260, min: 180, max: 520 },
  right: { initial: 320, min: 260, max: 640 },
  bottom: { initial: 280, min: 140, max: 800 },
};

export interface UIStore {
  /** Current panel sizes in px (left/right widths, bottom height). */
  panelSizes: Record<ResizablePanel, number>;
  setPanelSize(panel: ResizablePanel, px: number): void;
  resetPanelSize(panel: ResizablePanel): void;
  selectedIds: string[];
  /** Selected Bézier nodes of the (single) selected body. */
  selectedNodeIds: string[];
  hoverId: string | null;
  tool: Tool;
  /** Element kind to place at the next canvas click. */
  pendingAdd: ElementKind | null;
  theme: Theme;
  leftPanelOpen: boolean;
  rightPanelOpen: boolean;
  bottomPanelOpen: boolean;
  bottomTab: BottomTab;
  helpTopic: string | null;
  dialog: DialogKind | null;
  toasts: Toast[];
  /** Compact layout (tablet/phone) detected from viewport width. */
  compact: boolean;

  select(ids: string[], additive?: boolean): void;
  selectNodes(ids: string[], additive?: boolean): void;
  toggleSelect(id: string): void;
  clearSelection(): void;
  setHover(id: string | null): void;
  setTool(tool: Tool): void;
  setPendingAdd(kind: ElementKind | null): void;
  setTheme(theme: Theme): void;
  setPanel(panel: 'left' | 'right' | 'bottom', open: boolean): void;
  togglePanel(panel: 'left' | 'right' | 'bottom'): void;
  setBottomTab(tab: BottomTab): void;
  openHelp(topicId: string): void;
  closeHelp(): void;
  openDialog(kind: DialogKind): void;
  closeDialog(): void;
  toast(message: string, kind?: Toast['kind']): void;
  dismissToast(id: number): void;
  setCompact(compact: boolean): void;
}

const THEME_KEY = 'aeroflow.theme';
const SIZES_KEY = 'aeroflow.panel-sizes';

function clampSize(panel: ResizablePanel, px: number): number {
  const { min, max } = PANEL_LIMITS[panel];
  // The bottom panel may never take more than 70 % of the window.
  const cap = panel === 'bottom' && typeof window !== 'undefined' ? Math.min(max, window.innerHeight * 0.7) : max;
  return Math.round(Math.min(cap, Math.max(min, px)));
}

function loadSizes(): Record<ResizablePanel, number> {
  const sizes = { left: PANEL_LIMITS.left.initial, right: PANEL_LIMITS.right.initial, bottom: PANEL_LIMITS.bottom.initial };
  try {
    const raw = JSON.parse(localStorage.getItem(SIZES_KEY) ?? '{}') as Partial<Record<ResizablePanel, number>>;
    for (const k of ['left', 'right', 'bottom'] as const) {
      if (typeof raw[k] === 'number' && Number.isFinite(raw[k])) sizes[k] = clampSize(k, raw[k]!);
    }
  } catch {
    /* storage unavailable or corrupt: use defaults */
  }
  return sizes;
}

function saveSizes(sizes: Record<ResizablePanel, number>): void {
  try {
    localStorage.setItem(SIZES_KEY, JSON.stringify(sizes));
  } catch {
    /* ignore */
  }
}

function loadTheme(): Theme {
  try {
    const v = localStorage.getItem(THEME_KEY);
    if (v === 'light' || v === 'dark' || v === 'system') return v;
  } catch {
    /* storage unavailable */
  }
  return 'system';
}

let toastCounter = 0;

export const useUIStore = create<UIStore>((set, get) => ({
  panelSizes: loadSizes(),
  setPanelSize(panel, px) {
    const sizes = { ...get().panelSizes, [panel]: clampSize(panel, px) };
    if (sizes[panel] === get().panelSizes[panel]) return;
    saveSizes(sizes);
    set({ panelSizes: sizes });
  },
  resetPanelSize(panel) {
    const sizes = { ...get().panelSizes, [panel]: PANEL_LIMITS[panel].initial };
    saveSizes(sizes);
    set({ panelSizes: sizes });
  },
  selectedIds: [],
  selectedNodeIds: [],
  hoverId: null,
  tool: 'select',
  pendingAdd: null,
  theme: loadTheme(),
  leftPanelOpen: true,
  rightPanelOpen: true,
  bottomPanelOpen: true,
  bottomTab: 'surface',
  helpTopic: null,
  dialog: null,
  toasts: [],
  compact: false,

  select(ids, additive = false) {
    set((s) => ({ selectedIds: additive ? Array.from(new Set([...s.selectedIds, ...ids])) : ids, selectedNodeIds: [] }));
  },
  selectNodes(ids, additive = false) {
    set((s) => ({ selectedNodeIds: additive ? Array.from(new Set([...s.selectedNodeIds, ...ids])) : ids }));
  },
  toggleSelect(id) {
    set((s) => ({
      selectedIds: s.selectedIds.includes(id) ? s.selectedIds.filter((x) => x !== id) : [...s.selectedIds, id],
    }));
  },
  clearSelection() {
    set({ selectedIds: [], selectedNodeIds: [] });
  },
  setHover(id) {
    if (get().hoverId !== id) set({ hoverId: id });
  },
  setTool(tool) {
    set({ tool, pendingAdd: null });
  },
  setPendingAdd(kind) {
    set({ pendingAdd: kind, tool: 'select' });
  },
  setTheme(theme) {
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      /* ignore */
    }
    set({ theme });
  },
  setPanel(panel, open) {
    set(panel === 'left' ? { leftPanelOpen: open } : panel === 'right' ? { rightPanelOpen: open } : { bottomPanelOpen: open });
  },
  togglePanel(panel) {
    const s = get();
    s.setPanel(panel, !(panel === 'left' ? s.leftPanelOpen : panel === 'right' ? s.rightPanelOpen : s.bottomPanelOpen));
  },
  setBottomTab(tab) {
    set({ bottomTab: tab, bottomPanelOpen: true });
  },
  openHelp(topicId) {
    set({ helpTopic: topicId });
  },
  closeHelp() {
    set({ helpTopic: null });
  },
  openDialog(kind) {
    set({ dialog: kind });
  },
  closeDialog() {
    set({ dialog: null });
  },
  toast(message, kind = 'info') {
    const id = ++toastCounter;
    set((s) => ({ toasts: [...s.toasts.slice(-4), { id, kind, message }] }));
    setTimeout(() => get().dismissToast(id), kind === 'error' ? 9000 : 5000);
  },
  dismissToast(id) {
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
  },
  setCompact(compact) {
    if (get().compact !== compact) {
      set({ compact, leftPanelOpen: !compact, rightPanelOpen: !compact });
    }
  },
}));
