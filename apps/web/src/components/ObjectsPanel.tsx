/** Scene tree (PRD §31): freestream, elements, bodies; select / show / lock / delete / duplicate. */
import { useState } from 'react';
import { useSimulationStore } from '@/state/simulationStore';
import { useUIStore } from '@/state/uiStore';
import { useSolverStore } from '@/state/solverStore';
import type { SceneBody, SceneElement } from '@/domain/types';
import { IconCopy, IconEye, IconEyeOff, IconLock, IconTrash, IconUnlock } from './icons';

/** Stable empty fallbacks: zustand selectors must not return a fresh array each call. */
const EMPTY: never[] = [];

const ELEMENT_GLYPH: Record<string, string> = {
  uniformFlow: '→',
  source: '◉',
  sink: '◎',
  vortex: '↻',
  doublet: '⬭',
};

function Row({
  id,
  name,
  glyph,
  visible,
  locked,
  subtitle,
  warning,
}: {
  id: string;
  name: string;
  glyph: string;
  visible: boolean;
  locked: boolean;
  subtitle?: string;
  warning?: boolean;
}) {
  const selected = useUIStore((s) => s.selectedIds.includes(id));
  const hovered = useUIStore((s) => s.hoverId === id);
  const { select, toggleSelect, setHover } = useUIStore.getState();
  const { setVisible, setLocked, removeObjects, duplicateObject, renameObject } = useSimulationStore.getState();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(name);

  return (
    <li
      className={`tree-row${selected ? ' is-selected' : ''}${hovered ? ' is-hovered' : ''}${visible ? '' : ' is-hidden'}`}
      onMouseEnter={() => setHover(id)}
      onMouseLeave={() => setHover(null)}
    >
      <button
        className="tree-row__main"
        onClick={(e) => (e.shiftKey || e.metaKey || e.ctrlKey ? toggleSelect(id) : select([id]))}
        onDoubleClick={() => {
          setDraft(name);
          setEditing(true);
        }}
        aria-pressed={selected}
        aria-label={`${name}${warning ? ', has warnings' : ''}`}
      >
        <span className="tree-row__glyph" aria-hidden="true">
          {glyph}
        </span>
        {editing ? (
          <input
            className="tree-row__rename"
            autoFocus
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={() => {
              setEditing(false);
              if (draft.trim() && draft !== name) renameObject(id, draft.trim());
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
              if (e.key === 'Escape') setEditing(false);
              e.stopPropagation();
            }}
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <span className="tree-row__label">
            <span className="tree-row__name">{name}</span>
            {subtitle && <span className="tree-row__sub">{subtitle}</span>}
          </span>
        )}
        {warning && <span className="tree-row__warn" title="This object has warnings" />}
      </button>
      <span className="tree-row__actions">
        <button className="icon-btn" onClick={() => setVisible(id, !visible)} aria-label={visible ? 'Hide' : 'Show'} title={visible ? 'Hide' : 'Show'}>
          {visible ? <IconEye size={14} /> : <IconEyeOff size={14} />}
        </button>
        <button className="icon-btn" onClick={() => setLocked(id, !locked)} aria-label={locked ? 'Unlock' : 'Lock'} title={locked ? 'Unlock position' : 'Lock position'}>
          {locked ? <IconLock size={14} /> : <IconUnlock size={14} />}
        </button>
        <button className="icon-btn" onClick={() => duplicateObject(id)} aria-label="Duplicate" title="Duplicate">
          <IconCopy size={14} />
        </button>
        <button className="icon-btn icon-btn--danger" onClick={() => removeObjects([id])} aria-label="Delete" title="Delete">
          <IconTrash size={14} />
        </button>
      </span>
    </li>
  );
}

function elementSubtitle(e: SceneElement): string {
  const el = e.element;
  switch (el.type) {
    case 'uniformFlow':
      return `${el.velocity.toFixed(2)} m/s`;
    case 'vortex':
      return `Γ = ${el.circulation.toFixed(2)} m²/s`;
    case 'doublet':
      return `κ = ${el.strength.toFixed(2)} m³/s`;
    default:
      return `Λ = ${el.strength.toFixed(2)} m²/s`;
  }
}

function bodySubtitle(b: SceneBody): string {
  switch (b.geometry.kind) {
    case 'naca4':
      return `NACA ${b.geometry.code} · ${b.panels.count} panels`;
    case 'circle':
      return `r = ${b.geometry.radius} m · ${b.panels.count} panels`;
    case 'ellipse':
      return `${b.geometry.semiAxisX}×${b.geometry.semiAxisY} m · ${b.panels.count} panels`;
    case 'joukowski':
      return `Joukowski · ${b.panels.count} panels`;
    case 'points':
      return `${b.geometry.points.length} points${b.panels.distribution === 'asImported' ? '' : ` → ${b.panels.count} panels`}`;
  }
}

export function ObjectsPanel() {
  const scene = useSimulationStore((s) => s.scene);
  const selected = useUIStore((s) => s.selectedIds);
  const warnings = useSolverStore((s) => s.solution?.warnings ?? EMPTY);
  const select = useUIStore((s) => s.select);
  const clearSelection = useUIStore((s) => s.clearSelection);
  const warnIds = new Set(warnings.map((w) => w.objectId).filter(Boolean));
  const freestreamSelected = selected.length === 0;

  return (
    <aside className="panel panel--left" aria-label="Scene objects">
      <header className="panel__header">
        <h2>Scene</h2>
        <span className="panel__count">{scene.elements.length + scene.bodies.length}</span>
      </header>
      <div className="panel__body">
        <ul className="tree" aria-label="Objects">
          <li className={`tree-row tree-row--freestream${freestreamSelected ? ' is-selected' : ''}`}>
            <button className="tree-row__main" onClick={() => clearSelection()} aria-pressed={freestreamSelected}>
              <span className="tree-row__glyph" aria-hidden="true">☼</span>
              <span className="tree-row__label">
                <span className="tree-row__name">Freestream</span>
                <span className="tree-row__sub">
                  U∞ = {scene.conditions.velocity.toFixed(2)} m/s · α = {((scene.conditions.angle * 180) / Math.PI).toFixed(1)}°
                </span>
              </span>
            </button>
          </li>
          {scene.elements.length > 0 && <li className="tree__group" aria-hidden="true">Elements</li>}
          {scene.elements.map((e) => (
            <Row
              key={e.id}
              id={e.id}
              name={e.name}
              glyph={ELEMENT_GLYPH[e.element.type] ?? '•'}
              visible={e.visible}
              locked={e.locked}
              subtitle={elementSubtitle(e)}
              warning={warnIds.has(e.id)}
            />
          ))}
          {scene.bodies.length > 0 && <li className="tree__group" aria-hidden="true">Bodies</li>}
          {scene.bodies.map((b) => (
            <Row key={b.id} id={b.id} name={b.name} glyph="◇" visible={b.visible} locked={b.locked} subtitle={bodySubtitle(b)} warning={warnIds.has(b.id)} />
          ))}
        </ul>
        {scene.elements.length === 0 && scene.bodies.length === 0 && (
          <p className="panel__empty">
            Add a flow element or import a geometry to begin. Use <kbd>+ Add</kbd> in the toolbar.
          </p>
        )}
        {selected.length > 1 && (
          <div className="panel__footer">
            <span>{selected.length} selected</span>
            <button className="btn btn--ghost btn--sm" onClick={() => select([])}>
              Clear
            </button>
          </div>
        )}
      </div>
    </aside>
  );
}
