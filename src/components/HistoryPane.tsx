import { useCallback, useEffect, useState } from "react";
import {
  clearTransferHistory,
  listTransferHistory,
  openPath,
  revealPath,
  saveTextFile,
  sendFiles,
} from "../ipc/commands";
import { describeError } from "../lib/errors";
import { formatBytes, formatDeviceName, formatTimestamp } from "../lib/format";
import { exportTextFile } from "../lib/platform";
import { useAppStore } from "../state/store";
import type { HistoryEntry } from "../types/history";

type DirFilter = "all" | "sent" | "received";
type TimeFilter = "all" | "today" | "week";


export function HistoryPane() {
  const pushToast = useAppStore((s) => s.pushToast);
  const activePeers = useAppStore((s) => s.activePeers);
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [query, setQuery] = useState("");
  const [dir, setDir] = useState<DirFilter>("all");
  const [time, setTime] = useState<TimeFilter>("all");

  async function resend(deviceKey: string, paths: string[]) {
    try {
      await sendFiles(deviceKey, paths);
      pushToast("success", `Re-sending ${paths.length} file${paths.length === 1 ? "" : "s"}`);
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  const refresh = useCallback(async () => {
    try {
      setEntries(await listTransferHistory());
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }, [pushToast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function clear() {
    try {
      await clearTransferHistory();
      setEntries([]);
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function exportCsv(rows: HistoryEntry[]) {
    try {
      const esc = (v: string) => `"${v.replace(/"/g, '""')}"`;
      const header = "date,direction,device,files,bytes";
      const lines = rows.map((e) =>
        [
          esc(new Date(e.at_unix * 1000).toISOString()),
          e.direction,
          esc(e.display_name),
          esc(e.file_names.join("; ")),
          String(e.total_bytes),
        ].join(","),
      );
      const saved = await exportTextFile(
        "zwormhole-history.csv",
        [header, ...lines].join("\n"),
        [{ name: "CSV", extensions: ["csv"] }],
        "text/csv",
        saveTextFile,
      );
      if (saved) pushToast("success", "History exported");
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  if (entries.length === 0) {
    return (
      <div className="empty-state">
        <p className="empty-state__title">No transfers yet</p>
        <p className="empty-state__detail">
          Completed sends and receives will be logged here.
        </p>
      </div>
    );
  }

  const q = query.trim().toLowerCase();
  const now = Date.now() / 1000;
  const startOfToday = new Date().setHours(0, 0, 0, 0) / 1000;
  const timeFloor = time === "today" ? startOfToday : time === "week" ? now - 7 * 86400 : 0;
  const shown = entries.filter((e) => {
    if (dir !== "all" && e.direction !== dir) return false;
    if (e.at_unix < timeFloor) return false;
    if (
      q &&
      !e.display_name.toLowerCase().includes(q) &&
      !e.file_names.some((n) => n.toLowerCase().includes(q))
    )
      return false;
    return true;
  });
  const filtered = dir !== "all" || time !== "all" || q.length > 0;

  return (
    <div className="hist">
      <div className="hist__toolbar">
        <div className="hist__search-wrap">
          <svg className="hist__search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
            <circle cx="11" cy="11" r="7" />
            <path d="m21 21-4.3-4.3" />
          </svg>
          <input
            className="hist__search"
            type="text"
            placeholder="Search files or devices…"
            value={query}
            spellCheck={false}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <span className="hist__count">
          {filtered ? `${shown.length} / ${entries.length}` : entries.length}
        </span>
        <button
          className="btn btn--outline btn--small"
          onClick={() => void exportCsv(shown)}
          disabled={shown.length === 0}
        >
          Export
        </button>
        <button className="btn btn--outline btn--small" onClick={() => void clear()}>
          Clear
        </button>
      </div>

      <div className="hist__filters">
        {(["all", "sent", "received"] as DirFilter[]).map((d) => (
          <button
            key={d}
            className={`hist__chip${dir === d ? " hist__chip--on" : ""}`}
            onClick={() => setDir(d)}
          >
            {d === "all" ? "All" : d === "sent" ? "Sent" : "Received"}
          </button>
        ))}
        <span className="hist__filters-sep" aria-hidden />
        {(["all", "today", "week"] as TimeFilter[]).map((t) => (
          <button
            key={t}
            className={`hist__chip${time === t ? " hist__chip--on" : ""}`}
            onClick={() => setTime(t)}
          >
            {t === "all" ? "Any time" : t === "today" ? "Today" : "7 days"}
          </button>
        ))}
      </div>

      {shown.length === 0 ? (
        <p className="hist__noresults">No transfers match the filters.</p>
      ) : (
        <ul className="hist__list">
          {shown.map((e) => {
            const sent = e.direction === "sent";
            const canOpen = !sent && e.paths && e.paths.length > 0;
            const canResend =
              sent &&
              e.paths &&
              e.paths.length > 0 &&
              activePeers.some((p) => p.device_key === e.device_key);
            return (
              <li key={e.id} className="hist__row">
                <span
                  className={`hist__dir hist__dir--${sent ? "sent" : "recv"}`}
                  title={sent ? "Sent" : "Received"}
                  aria-hidden
                >
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
                    {sent ? <path d="M7 17 17 7M9 7h8v8" /> : <path d="M17 7 7 17M15 17H7V9" />}
                  </svg>
                </span>
                <div className="hist__body">
                  <span className="hist__files">{e.file_names.join(", ")}</span>
                  <span className="hist__meta">
                    {sent ? "to" : "from"} {formatDeviceName(e.display_name)} ·{" "}
                    {formatBytes(e.total_bytes)}
                    {canOpen || canResend ? " · " : null}
                    {canOpen && e.paths.length === 1 ? (
                      <button
                        className="hist__link"
                        onClick={() =>
                          openPath(e.paths[0]).catch((err) => pushToast("error", String(err)))
                        }
                      >
                        Open
                      </button>
                    ) : null}
                    {canOpen ? (
                      <button
                        className="hist__link"
                        onClick={() =>
                          revealPath(e.paths[0]).catch((err) => pushToast("error", String(err)))
                        }
                      >
                        Show in folder
                      </button>
                    ) : null}
                    {canResend ? (
                      <button
                        className="hist__link"
                        onClick={() => void resend(e.device_key, e.paths)}
                      >
                        Re-send
                      </button>
                    ) : null}
                  </span>
                </div>
                <span className="hist__time">{formatTimestamp(e.at_unix)}</span>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
