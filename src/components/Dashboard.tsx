import { useEffect, useState, type ReactNode } from "react";
import { listTransferHistory } from "../ipc/commands";
import type { HistoryEntry } from "../types/history";
import { useAppStore } from "../state/store";
import { AboutPane } from "./AboutPane";
import { ActivityLogPane } from "./ActivityLogPane";
import { ConnectPane } from "./ConnectPane";
import { HistoryPane } from "./HistoryPane";
import { SendTextPane } from "./SendTextPane";
import { TransferPane } from "./TransferPane";


const svg = {
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.7,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;
const ICONS: Record<string, ReactNode> = {
  transfers: (
    <svg {...svg}>
      <path d="M7 17V7m0 0-3 3m3-3 3 3" />
      <path d="M17 7v10m0 0 3-3m-3 3-3-3" />
    </svg>
  ),
  history: (
    <svg {...svg}>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7v5l3 2" />
    </svg>
  ),
  connect: (
    <svg {...svg}>
      <path d="M9 12h6" />
      <path d="M9 8H7a4 4 0 0 0 0 8h2" />
      <path d="M15 8h2a4 4 0 0 1 0 8h-2" />
    </svg>
  ),
  text: (
    <svg {...svg}>
      <path d="M21 15a2 2 0 0 1-2 2H8l-4 4V5a2 2 0 0 1 2-2h13a2 2 0 0 1 2 2z" />
    </svg>
  ),
  logs: (
    <svg {...svg}>
      <path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" />
    </svg>
  ),
  about: (
    <svg {...svg}>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 11v5M12 8h.01" />
    </svg>
  ),
};

interface SectionDef {
  id: string;
  label: string;
  desc: string;
  soon?: boolean;


  feature?: boolean;
}

const SECTIONS: SectionDef[] = [
  { id: "transfers", label: "Transfers", desc: "Active & recent transfers" },
  { id: "history", label: "History", desc: "Log of completed transfers" },
  { id: "connect", label: "Connect by IP", desc: "Reach a device manually" },
  { id: "text", label: "Send text", desc: "Send a snippet or clipboard" },
  { id: "logs", label: "Activity log", desc: "Connection & pairing events" },
  { id: "about", label: "About", desc: "The app, tech & links", feature: true },
];


function timeAgo(unixSecs: number): string {
  const s = Math.max(0, Math.floor(Date.now() / 1000 - unixSecs));
  if (s < 45) return "just now";
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} minute${m === 1 ? "" : "s"} ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h} hour${h === 1 ? "" : "s"} ago`;
  const d = Math.floor(h / 24);
  return `${d} day${d === 1 ? "" : "s"} ago`;
}


type TransferState = { label: string; cls: string };

function DashboardHome() {
  const go = useAppStore((s) => s.setDashboardSection);
  const setViewMode = useAppStore((s) => s.setViewMode);
  const transfers = useAppStore((s) => s.transfers);
  const activePeers = useAppStore((s) => s.activePeers);
  const logCount = useAppStore((s) => s.activityLog.length);
  const [historyCount, setHistoryCount] = useState<number | null>(null);
  const [lastTransfer, setLastTransfer] = useState<HistoryEntry | null>(null);

  useEffect(() => {
    listTransferHistory()
      .then((h) => {
        setHistoryCount(h.length);

        const newest = h.reduce<HistoryEntry | null>(
          (best, e) => (best === null || e.at_unix > best.at_unix ? e : best),
          null,
        );
        setLastTransfer(newest);
      })
      .catch(() => {
        setHistoryCount(null);
        setLastTransfer(null);
      });
  }, []);

  const jobs = Object.values(transfers);
  const active = jobs.filter(
    (j) => j.status === "in_progress" || j.status === "queued" || j.status === "archiving",
  ).length;


  const transferState: TransferState =
    active > 0
      ? { label: "Active", cls: "active" }
      : jobs.some((j) => j.status === "failed")
        ? { label: "Error", cls: "error" }
        : jobs.some((j) => j.status === "paused" || j.status === "queued")
          ? { label: "Waiting", cls: "waiting" }
          : { label: "Ready", cls: "ready" };

  const statFor = (id: string): string | null => {
    if (id === "transfers") return active > 0 ? `${active} active` : `${jobs.length} recent`;
    if (id === "history") return historyCount === null ? null : `${historyCount} logged`;
    if (id === "text") return `${activePeers.length} connected`;
    if (id === "logs") return logCount > 0 ? `${logCount} events` : null;
    return null;
  };

  return (
    <div className="dash">
      <header className="dash__hero">
        <div className="dash__hero-text">
          <span className="dash__eyebrow">Overview</span>
          <h2 className="dash__title">Dashboard</h2>
          <p className="dash__subtitle">Everything ZWormHole can do, in one place.</p>
        </div>
        <div className="dash__stats">
          <button
            className="dash__stat dash__stat--btn"
            onClick={() => activePeers.length > 0 && setViewMode("focus")}
            disabled={activePeers.length === 0}
            title={activePeers.length > 0 ? "Open the connected device" : "No connected devices"}
          >
            <span
              className={`dash__stat-num${activePeers.length > 0 ? " dash__stat-num--ok" : ""}`}
            >
              {activePeers.length}
            </span>
            <span className="dash__stat-label">Connected</span>
          </button>
          <button className="dash__stat dash__stat--btn" onClick={() => go("transfers")} title="Open Transfers">
            <span className={`dash__stat-num${active > 0 ? " dash__stat-num--accent" : ""}`}>
              {active}
            </span>
            <span className="dash__stat-label">Active</span>
          </button>
          <button className="dash__stat dash__stat--btn" onClick={() => go("history")} title="Open History">
            <span className="dash__stat-num">{historyCount ?? "—"}</span>
            <span className="dash__stat-label">Completed</span>
          </button>
        </div>
      </header>

      <div className="dash__grid">
        {SECTIONS.map((s) => (
          <button
            key={s.id}
            className={`dash__tile${s.feature ? " dash__tile--feature" : ""}`}
            onClick={() => !s.soon && go(s.id)}
            disabled={s.soon}
          >
            <span className="dash__tile-icon">{ICONS[s.id]}</span>
            <span className="dash__tile-label">
              {s.label}
              {s.id === "transfers" ? (
                <span className={`dash__tile-status dash__tile-status--${transferState.cls}`}>
                  <span className="dash__tile-status-dot" aria-hidden />
                  {transferState.label}
                </span>
              ) : null}
            </span>
            <span className="dash__tile-desc">{s.desc}</span>
            {s.id === "transfers" && lastTransfer ? (
              <span className="dash__tile-last" title={lastTransfer.file_names.join(", ")}>
                Last: {lastTransfer.file_names[0] ?? "transfer"}
                {lastTransfer.file_names.length > 1
                  ? ` +${lastTransfer.file_names.length - 1}`
                  : ""}{" "}
                · {timeAgo(lastTransfer.at_unix)}
              </span>
            ) : null}
            <span className="dash__tile-foot">
              {statFor(s.id) ? <span className="dash__tile-stat">{statFor(s.id)}</span> : <span />}
              <span className="dash__tile-arrow" aria-hidden>
                →
              </span>
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

export function Dashboard() {
  const section = useAppStore((s) => s.dashboardSection);
  const go = useAppStore((s) => s.setDashboardSection);

  const def = SECTIONS.find((s) => s.id === section);

  return (
    <section className="pane pane--main">
      {section === "home" || !def ? (
        <DashboardHome />
      ) : (
        <div className="dash__section" key={section}>
          <header className="dash__section-head">
            <button className="dash__back" onClick={() => go("home")}>
              <span aria-hidden>‹</span> Dashboard
            </button>
            <span className="dash__section-icon">{ICONS[section]}</span>
            <h2 className="dash__section-title">{def.label}</h2>
          </header>
          <div className="dash__section-body">
            {section === "transfers" ? <TransferPane /> : null}
            {section === "history" ? <HistoryPane /> : null}
            {section === "connect" ? <ConnectPane /> : null}
            {section === "text" ? <SendTextPane /> : null}
            {section === "logs" ? <ActivityLogPane /> : null}
            {section === "about" ? <AboutPane /> : null}
          </div>
        </div>
      )}
    </section>
  );
}
