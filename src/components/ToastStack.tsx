import { useAppStore } from "../state/store";

export function ToastStack() {
  const toasts = useAppStore((s) => s.toasts);
  const dismiss = useAppStore((s) => s.dismissToast);

  if (toasts.length === 0) return null;

  return (
    <div className="toast-stack">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast--${t.kind}`}>
          <span className="toast__msg" onClick={() => dismiss(t.id)}>
            {t.message}
          </span>
          {t.action ? (
            <button
              className="toast__action"
              onClick={() => {
                t.action?.run();
                dismiss(t.id);
              }}
            >
              {t.action.label}
            </button>
          ) : null}
        </div>
      ))}
    </div>
  );
}
