/**
 * Properties for a Bézier body (PRD2 §40): path state, mirror, validation, and
 * a numeric node editor. The numeric fields are the precision and keyboard
 * alternative to dragging on the canvas (PRD2 §76).
 */
import { useMemo } from 'react';
import { useUIStore } from '@/state/uiStore';
import type { BezierGeometry, BezierNode, NodeType, SceneBody } from '@/domain/types';
import { deleteNodes, mirrorGeometry, segmentCount, setClosed, setNodeHandle, setNodePosition, setNodeType, validateCached } from '@/domain/bezier';
import { NumberField } from './NumberField';
import { IconInfo, IconWarning } from './icons';

type Update = (fn: (b: SceneBody) => SceneBody, transient?: boolean) => void;

const TYPES: { id: NodeType; label: string; hint: string }[] = [
  { id: 'smooth', label: 'Smooth', hint: 'Handles stay collinear: a continuous tangent.' },
  { id: 'corner', label: 'Corner', hint: 'Handles are independent: a sharp edge, e.g. a trailing edge.' },
  { id: 'symmetric', label: 'Symmetric', hint: 'Handles stay opposite and equal in length.' },
];

function NodeForm({ g, nodes, update }: { g: BezierGeometry; nodes: BezierNode[]; update: Update }) {
  const selectNodes = useUIStore((s) => s.selectNodes);
  const toast = useUIStore((s) => s.toast);
  const set = (fn: (g: BezierGeometry) => BezierGeometry, transient = false) => update((b) => (b.geometry.kind === 'bezier' ? { ...b, geometry: fn(b.geometry) } : b), transient);
  const single = nodes.length === 1 ? nodes[0] : null;

  const remove = () => {
    const next = deleteNodes(g, nodes.map((n) => n.id));
    if (!next) {
      toast(`A ${g.closed ? 'closed shape needs at least 3 nodes' : 'path needs at least 2 nodes'}.`, 'warning');
      return;
    }
    set(() => next);
    selectNodes([]);
  };

  return (
    <div className="form__section">
      <h3>{single ? 'Node' : `${nodes.length} nodes selected`}</h3>
      <div className="segmented" role="radiogroup" aria-label="Node type">
        {TYPES.map((t) => (
          <button
            key={t.id}
            role="radio"
            aria-checked={nodes.every((n) => n.nodeType === t.id)}
            className={nodes.every((n) => n.nodeType === t.id) ? 'is-active' : ''}
            title={t.hint}
            onClick={() => set((x) => nodes.reduce((acc, n) => setNodeType(acc, n.id, t.id), x))}
          >
            {t.label}
          </button>
        ))}
      </div>
      {single && (
        <>
          <NumberField label="Position X" symbol="x" unit="m" value={single.position.x} step={0.01} digits={4} onChange={(v, t) => set((x) => setNodePosition(x, single.id, { ...single.position, x: v }), t)} />
          <NumberField label="Position Y" symbol="y" unit="m" value={single.position.y} step={0.01} digits={4} onChange={(v, t) => set((x) => setNodePosition(x, single.id, { ...single.position, y: v }), t)} />
          {(['in', 'out'] as const).map((side) => {
            const h = side === 'in' ? single.inHandle : single.outHandle;
            const label = side === 'in' ? 'Incoming' : 'Outgoing';
            return (
              <div key={side}>
                <NumberField label={`${label} handle X`} unit="m" value={h.x} step={0.01} digits={4} onChange={(v, t) => set((x) => setNodeHandle(x, single.id, side, { ...h, x: v }), t)} />
                <NumberField label={`${label} handle Y`} unit="m" value={h.y} step={0.01} digits={4} onChange={(v, t) => set((x) => setNodeHandle(x, single.id, side, { ...h, y: v }), t)} />
              </div>
            );
          })}
          <p className="form__hint">Handles are offsets from the node; (0, 0) means a straight neighbouring segment.</p>
        </>
      )}
      <button className="btn btn--sm btn--danger" onClick={remove}>Delete {single ? 'node' : 'nodes'}</button>
    </div>
  );
}

export function BezierFields({ body, update }: { body: SceneBody; update: Update }) {
  const g = body.geometry as BezierGeometry;
  const selectedNodeIds = useUIStore((s) => s.selectedNodeIds);
  const tool = useUIStore((s) => s.tool);
  const setTool = useUIStore((s) => s.setTool);
  const validation = useMemo(() => validateCached(g), [g]);
  const selected = useMemo(() => g.nodes.filter((n) => selectedNodeIds.includes(n.id)), [g, selectedNodeIds]);
  const set = (fn: (g: BezierGeometry) => BezierGeometry) => update((b) => (b.geometry.kind === 'bezier' ? { ...b, geometry: fn(b.geometry) } : b));

  return (
    <>
      <p className="form__hint">
        Bézier body: {g.nodes.length} nodes, {segmentCount(g)} cubic segments. The curve is the source of truth; panels are re-derived from it after every edit.
      </p>
      {validation.status !== 'ok' && (
        <div className={`callout callout--${validation.status === 'invalid' ? 'danger' : 'warning'}`}>
          <IconWarning size={14} />
          <span>{validation.messages.join(' ')}</span>
        </div>
      )}
      {validation.status === 'ok' && validation.messages.length > 0 && (
        <div className="callout"><IconInfo size={14} /><span>{validation.messages.join(' ')}</span></div>
      )}
      {g.fitError != null && (
        <p className="form__hint">Imported coordinates were approximated by Bézier curves; the largest deviation from the source points is {g.fitError.toPrecision(2)} m.</p>
      )}
      <label className="checkbox">
        <input type="checkbox" checked={g.closed} disabled={body.locked} onChange={() => set((x) => setClosed(x, !x.closed))} />
        Closed path (required to solve)
      </label>
      <div className="row" style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
        <button className={`btn btn--sm${tool === 'node' ? ' is-active' : ''}`} onClick={() => setTool(tool === 'node' ? 'select' : 'node')} aria-pressed={tool === 'node'}>Edit nodes (N)</button>
        <button className={`btn btn--sm${tool === 'pen' ? ' is-active' : ''}`} onClick={() => setTool(tool === 'pen' ? 'select' : 'pen')} aria-pressed={tool === 'pen'} disabled={g.closed}>Pen (P)</button>
        <button className="btn btn--sm" disabled={body.locked} onClick={() => set((x) => mirrorGeometry(x, 'vertical'))} title="Flip left ↔ right about the shape's centre">Mirror ↔</button>
        <button className="btn btn--sm" disabled={body.locked} onClick={() => set((x) => mirrorGeometry(x, 'horizontal'))} title="Flip top ↔ bottom about the shape's centre">Mirror ↕</button>
      </div>
      {body.locked && <p className="form__hint">Locked: unlock the body to edit its nodes. It still takes part in the simulation.</p>}
      {selected.length > 0 && !body.locked && <NodeForm g={g} nodes={selected} update={update} />}
    </>
  );
}
