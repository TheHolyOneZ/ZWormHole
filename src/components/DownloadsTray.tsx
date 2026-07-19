import { useEffect, useRef, useState } from "react";
import { listTransferHistory, openPath, revealPath } from "../ipc/commands";
import {
  formatBytes,
  formatDeviceName,
  formatRate,
  formatTimestamp,
  timeAgo,
} from "../lib/format";
import { useAppStore } from "../state/store";
import { Sparkline } from "./Sparkline";
import type { HistoryEntry } from "../types/history";

const DownloadIcon = () => (
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={1.8}
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M12 3v12" />
    <path d="m7 10 5 5 5-5" />
    <path d="M5 20h14" />
  </svg>
);


export function DownloadsTray() {
  const receives = useAppStore((s) => s.receives);
  const recvGraphs = useAppStore((s) => s.recvGraphs);
  const [open, setOpen] = useState(false);
  const [recent, setRecent] = useState<HistoryEntry[]>([]);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  const live = Object.values(receives).sort((a, b) => b.updated - a.updated);
  const activeCount = live.length;

  async function loadRecent() {
    try {
      const h = await listTransferHistory();
      setRecent(
        h
          .filter((e) => e.direction === "received")
          .sort((a, b) => b.at_unix - a.at_unix)
          .slice(0, 8),
      );
    } catch {

    }
  }


  useEffect(() => {
    if (open) void loadRecent();
  }, [open]);
  useEffect(() => {
    void loadRecent();
  }, [activeCount]);

  useEffect(() => {
    if (!open) return;
    function onDown(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="downloads" ref={ref}>
      <button
        className={`downloads__btn${activeCount > 0 ? " downloads__btn--active" : ""}`}
        onClick={() => setOpen((o) => !o)}
        title="Downloads"
        aria-label="Downloads"
      >
        <DownloadIcon />
        {activeCount > 0 ? <span className="downloads__badge">{activeCount}</span> : null}
      </button>

      {open ? (
        <div className="downloads__panel" role="dialog" aria-label="Downloads">
          <div className="downloads__head">Downloads</div>

          {live.length > 0 ? (
            <div className="downloads__section">
              <div className="downloads__section-title">Receiving now</div>
              {live.map((r) => {
                const pct =
                  r.bytes_total > 0 ? Math.round((r.bytes_done / r.bytes_total) * 100) : 0;
                return (
                  <div key={r.id} className="downloads__live">
                    <div className="downloads__live-top">
                      <span className="downloads__live-label" title={r.label}>
                        {r.label}
                      </span>
                      <span className="downloads__live-pct">{pct}%</span>
                    </div>
                    <div className="downloads__live-from">
                      from {formatDeviceName(r.display_name)}
                    </div>
                    {r.history.length >= 2 ? (
                      <Sparkline values={r.history} height={34} className="downloads__spark" />
                    ) : null}
                    <div className="downloads__bar">
                      <div className="downloads__bar-fill" style={{ width: `${pct}%` }} />
                    </div>
                    <div className="downloads__live-meta">
                      <span>
                        {formatBytes(r.bytes_done)} / {formatBytes(r.bytes_total)}
                      </span>
                      <span className="downloads__live-rate">{formatRate(r.rate)}</span>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : null}

          <div className="downloads__section">
            <div className="downloads__section-title">Recent</div>
            {recent.length === 0 ? (
              <div className="downloads__empty">Files you receive will show up here.</div>
            ) : (
              recent.map((e) => {
                const expanded = expandedId === e.id;


                const graph =
                  e.rate_samples && e.rate_samples.length >= 2
                    ? e.rate_samples
                    : recvGraphs[e.paths.join("|")];
                return (
                  <div
                    key={e.id}
                    className={`downloads__item${expanded ? " downloads__item--open" : ""}`}
                  >
                    <div className="downloads__item-row">
                      <button
                        className="downloads__item-main"
                        onClick={() => setExpandedId(expanded ? null : e.id)}
                        title="Click for details"
                      >
                        <span className="downloads__item-name" title={e.file_names.join(", ")}>
                          {e.file_names[0] ?? "file"}
                          {e.file_names.length > 1 ? ` +${e.file_names.length - 1}` : ""}
                        </span>
                        <span className="downloads__item-meta">
                          {formatDeviceName(e.display_name)} · {timeAgo(e.at_unix)}
                        </span>
                      </button>
                      {e.paths[0] ? (
                        <div className="downloads__item-actions">
                          <button
                            className="downloads__item-btn"
                            onClick={() => void openPath(e.paths[0])}
                          >
                            Open
                          </button>
                          <button
                            className="downloads__item-btn"
                            onClick={() => void revealPath(e.paths[0])}
                          >
                            Show
                          </button>
                        </div>
                      ) : null}
                    </div>

                    {expanded ? (
                      <div className="downloads__detail">
                        {graph && graph.length >= 2 ? (
                          <>
                            <div className="downloads__detail-label">Speed while receiving</div>
                            <Sparkline values={graph} height={34} className="downloads__spark" />
                          </>
                        ) : null}
                        <div className="downloads__detail-row">
                          <span>From</span>
                          <span>{formatDeviceName(e.display_name)}</span>
                        </div>
                        <div className="downloads__detail-row">
                          <span>Size</span>
                          <span>{formatBytes(e.total_bytes)}</span>
                        </div>
                        <div className="downloads__detail-row">
                          <span>When</span>
                          <span>{formatTimestamp(e.at_unix)}</span>
                        </div>
                        {e.file_names.length > 1 ? (
                          <ul className="downloads__files">
                            {e.file_names.map((n) => (
                              <li key={n} title={n}>
                                {n}
                              </li>
                            ))}
                          </ul>
                        ) : null}
                      </div>
                    ) : null}
                  </div>
                );
              })
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
