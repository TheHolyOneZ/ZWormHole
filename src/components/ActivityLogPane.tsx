import { formatTimestamp } from "../lib/format";
import { useAppStore } from "../state/store";


export function ActivityLogPane() {
  const log = useAppStore((s) => s.activityLog);
  const clear = useAppStore((s) => s.clearActivityLog);

  if (log.length === 0) {
    return (
      <div className="empty-state">
        <p className="empty-state__title">Nothing yet</p>
        <p className="empty-state__detail">
          Connections, pairings, and transfers this session will show up here.
        </p>
      </div>
    );
  }

  return (
    <div className="act">
      <div className="act__toolbar">
        <span className="act__count">
          {log.length} event{log.length === 1 ? "" : "s"} · this session
        </span>
        <button className="btn btn--outline btn--small" onClick={() => clear()}>
          Clear
        </button>
      </div>
      <ul className="act__list">
        {log.map((e) => (
          <li key={e.id} className="act__row">
            <span className={`act__dot act__dot--${e.kind}`} aria-hidden />
            <span className="act__msg">{e.message}</span>
            <span className="act__time">{formatTimestamp(Math.floor(e.at / 1000))}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
