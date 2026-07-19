import { useAppStore } from "../state/store";


export function BusyOverlay() {
  const busy = useAppStore((s) => s.busy);
  if (!busy) return null;
  return (
    <div className="busy-overlay">
      <div className="busy-card">
        <span className="busy-spinner" aria-hidden />
        <span className="busy-text">{busy}</span>
      </div>
    </div>
  );
}
