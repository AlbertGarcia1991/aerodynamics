import { useUIStore } from '@/state/uiStore';
import { IconCheck, IconClose, IconInfo, IconWarning } from './icons';

export function Toasts() {
  const toasts = useUIStore((s) => s.toasts);
  const dismiss = useUIStore((s) => s.dismissToast);
  if (toasts.length === 0) return null;
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast--${t.kind}`}>
          <span className="toast__icon">
            {t.kind === 'success' ? <IconCheck /> : t.kind === 'info' ? <IconInfo /> : <IconWarning />}
          </span>
          <span className="toast__text">{t.message}</span>
          <button className="icon-btn" onClick={() => dismiss(t.id)} aria-label="Dismiss notification">
            <IconClose size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
