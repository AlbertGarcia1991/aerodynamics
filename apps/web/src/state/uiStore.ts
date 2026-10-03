/** UIState (PRD §49): selection, tools, panels, theme, dialogs, toasts. */
import { create } from 'zustand';
import type { ElementKind } from '@/domain/types';

export type Theme = 'light' | 'dark' | 'system';
export type Tool = 'select' | 'pan' | 'seed';
export type BottomTab = 'surface' | 'polar' | 'diagnostics' | 'data';
export type DialogKind = 'welcome' | 'examples' | 'import' | 'geometry' | 'about' | 'shortcuts' | 'export' | 'sweep';

export interface Toast {
  id: number;
  kind: 'info' | 'success' | 'warning' | 'error';
  message: string;
}

export interface UIStore {
  selectedIds: string[];
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
  selectedIds: [],
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
    set((s) => ({ selectedIds: additive ? Array.from(new Set([...s.selectedIds, ...ids])) : ids }));
  },
  toggleSelect(id) {
    set((s) => ({
      selectedIds: s.selectedIds.includes(id) ? s.selectedIds.filter((x) => x !== id) : [...s.selectedIds, id],
    }));
  },
  clearSelection() {
    set({ selectedIds: [] });
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
