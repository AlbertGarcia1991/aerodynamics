/** Contextual help (PRD §55–§56): what it means / why it matters / definition. */
import { useSolverStore } from '@/state/solverStore';
import { useUIStore } from '@/state/uiStore';
import { IconClose } from './icons';

/** Stable empty fallbacks: zustand selectors must not return a fresh array each call. */
const EMPTY: never[] = [];

export function HelpDrawer() {
  const topicId = useUIStore((s) => s.helpTopic);
  const close = useUIStore((s) => s.closeHelp);
  const open = useUIStore((s) => s.openHelp);
  const topics = useSolverStore((s) => s.metadata?.helpTopics ?? EMPTY);
  if (!topicId) return null;
  const topic = topics.find((t) => t.id === topicId);
  return (
    <aside className="help-drawer" role="dialog" aria-label="Help" aria-modal="false">
      <header className="panel__header">
        <h2>Help</h2>
        <button className="icon-btn" onClick={close} aria-label="Close help" style={{ marginLeft: 'auto' }}>
          <IconClose />
        </button>
      </header>
      <div className="help-drawer__body">
        {topic ? (
          <>
            <h2 style={{ marginTop: 0 }}>{topic.title}</h2>
            <h3>What it means</h3>
            <p>{topic.meaning}</p>
            <h3>Why it matters</h3>
            <p>{topic.importance}</p>
            <h3>Definition</h3>
            <div className="definition">{topic.definition}</div>
          </>
        ) : (
          <p className="muted">No help is available for “{topicId}”.</p>
        )}
        <h3>All topics</h3>
        <div className="help-index">
          {topics.map((t) => (
            <button key={t.id} onClick={() => open(t.id)} aria-current={t.id === topicId}>
              {t.title}
            </button>
          ))}
        </div>
      </div>
    </aside>
  );
}
