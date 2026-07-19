import { useCallback, useEffect, useState } from "react";
import {
  cancelTransfer,
  dismissResumable,
  listResumableTransfers,
  pauseTransfer,
  resumeDeviceTransfers,
  resumeTransfer,
} from "../ipc/commands";
import { describeError } from "../lib/errors";
import { deviceLabel, formatBytes, formatDuration, formatRate, timeAgo } from "../lib/format";
import { useAppStore } from "../state/store";
import { ContextMenu } from "./ContextMenu";
import { WormholeMark } from "./WormholeMark";
import type { TransferJob } from "../types/transfer";
import type { PersistedJob } from "../types/resumable";

function TransferRow({
  job,
  deviceName,
  rate,
}: {
  job: TransferJob;
  deviceName: string;
  rate: number | undefined;
}) {
  const refreshTransfers = useAppStore((s) => s.refreshTransfers);
  const openDetail = useAppStore((s) => s.setSelectedTransfer);
  const [menuPt, setMenuPt] = useState<{ x: number; y: number } | null>(null);
  const pct = job.bytes_total > 0 ? Math.round((job.bytes_done / job.bytes_total) * 100) : 0;
  const paused = job.status === "paused";
  const inFlight =
    job.status === "queued" || job.status === "in_progress" || job.status === "archiving" || paused;
  const active = job.status === "in_progress";


  const remaining = job.bytes_total - job.bytes_done;
  const etaSecs = active && rate && rate > 0 ? remaining / rate : NaN;

  async function cancel() {
    await cancelTransfer(job.id);


    void refreshTransfers();
  }


  async function togglePause() {
    if (paused) await resumeTransfer(job.id);
    else await pauseTransfer(job.id);
    void refreshTransfers();
  }

  return (
    <li
      className={`transfer-row transfer-row--${job.status}`}
      onClick={() => openDetail(job.id)}
      onContextMenu={(e) => {
        e.preventDefault();
        setMenuPt({ x: e.clientX, y: e.clientY });
      }}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") openDetail(job.id);
      }}
      title="Click for details · right-click for actions"
    >
      <div className="transfer-row__top">
        <span className="transfer-row__label" title={job.label}>
          {job.label || deviceName}
        </span>
        <span className="transfer-row__status">{job.status.replace("_", " ")}</span>
      </div>
      <div className="transfer-row__meta">
        <span className="transfer-row__to">to {deviceName}</span>
        {job.created_at ? (
          <span className="transfer-row__when">· {timeAgo(job.created_at)}</span>
        ) : null}
      </div>
      <div className="transfer-row__bar">
        <div className="transfer-row__bar-fill" style={{ width: `${pct}%` }} />
      </div>
      <div className="transfer-row__bottom">
        <span>
          {formatBytes(job.bytes_done)} / {formatBytes(job.bytes_total)}
          {inFlight ? ` · ${pct}%` : null}
        </span>
        {active ? (
          <span className="transfer-row__rate">
            {formatRate(rate ?? 0)}
            {Number.isFinite(etaSecs) ? ` · ${formatDuration(etaSecs)} left` : ""}
          </span>
        ) : null}
        {active || paused ? (
          <button
            className="btn btn--outline btn--small"
            onClick={(e) => {
              e.stopPropagation();
              void togglePause();
            }}
          >
            {paused ? "Resume" : "Pause"}
          </button>
        ) : null}
        {inFlight ? (
          <button
            className="btn btn--outline btn--small"
            onClick={(e) => {
              e.stopPropagation();
              void cancel();
            }}
          >
            Cancel
          </button>
        ) : null}
      </div>

      {menuPt ? (
        <ContextMenu point={menuPt} onClose={() => setMenuPt(null)}>
          <button className="side-menu__item" onClick={() => openDetail(job.id)}>
            View details & speed
          </button>
          {active || paused ? (
            <button className="side-menu__item" onClick={() => void togglePause()}>
              {paused ? "Resume" : "Pause"}
            </button>
          ) : null}
          {inFlight ? (
            <button
              className="side-menu__item side-menu__item--danger"
              onClick={() => void cancel()}
            >
              Cancel transfer
            </button>
          ) : null}
        </ContextMenu>
      ) : null}
    </li>
  );
}


function ResumableSection({
  deviceKey,
  nameFor,
  onEmptyChange,
}: {
  deviceKey?: string;
  nameFor: (raw: string) => string;
  onEmptyChange?: (empty: boolean) => void;
}) {
  const transfers = useAppStore((s) => s.transfers);
  const activePeers = useAppStore((s) => s.activePeers);
  const pushToast = useAppStore((s) => s.pushToast);
  const [jobs, setJobs] = useState<PersistedJob[]>([]);

  const refresh = useCallback(async () => {
    try {
      const all = await listResumableTransfers();
      setJobs(deviceKey ? all.filter((j) => j.device_key === deviceKey) : all);
    } catch {

    }
  }, [deviceKey]);

  useEffect(() => {
    void refresh();
  }, [refresh, transfers, activePeers]);


  const activeIds = new Set(
    Object.values(transfers)
      .filter((j) => ["queued", "archiving", "in_progress", "paused"].includes(j.status))
      .map((j) => j.id),
  );
  const shown = jobs.filter((j) => !activeIds.has(j.id));

  useEffect(() => {
    onEmptyChange?.(shown.length === 0);
  }, [shown.length, onEmptyChange]);

  if (shown.length === 0) return null;

  const connected = new Set(activePeers.map((p) => p.device_key));
  const baseName = (p: string) => p.split(/[\\/]/).pop() || p;
  const sourceLabel = (j: PersistedJob) =>
    j.source.kind === "folder"
      ? `${baseName(j.source.folder_path)}/ (folder)`
      : j.source.paths.map(baseName).join(", ");

  async function act(fn: () => Promise<void>) {
    try {
      await fn();
      await refresh();
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  return (
    <div className="resumable">
      <p className="resumable__title">Interrupted — resumes on reconnect</p>
      <ul className="resumable__list">
        {shown.map((j) => {
          const online = connected.has(j.device_key);
          return (
            <li key={j.id} className="resumable__row">
              <div className="resumable__body">
                <span className="resumable__files">{sourceLabel(j)}</span>
                <span className="resumable__meta">
                  to {nameFor(j.device_key)} ·{" "}
                  {online ? "reconnected — resuming…" : "waiting for device to reconnect"}
                </span>
              </div>
              {online ? (
                <button
                  className="btn btn--accent btn--small"
                  onClick={() => void act(() => resumeDeviceTransfers(j.device_key))}
                >
                  Resume now
                </button>
              ) : null}
              <button
                className="btn btn--outline btn--small"
                onClick={() => void act(() => dismissResumable(j.id))}
              >
                Dismiss
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

export function TransferPane({ deviceKey }: { deviceKey?: string } = {}) {
  const transfers = useAppStore((s) => s.transfers);
  const trusted = useAppStore((s) => s.trusted);
  const rates = useAppStore((s) => s.transferRates);
  const [noResumable, setNoResumable] = useState(true);
  const jobs = Object.values(transfers)
    .filter((j) => !deviceKey || j.device_fullname === deviceKey)
    .sort((a, b) => (a.id < b.id ? 1 : -1));


  const nameFor = (raw: string): string => deviceLabel(raw, trusted);


  const showEmpty = jobs.length === 0 && noResumable;

  return (
    <>
      <ResumableSection deviceKey={deviceKey} nameFor={nameFor} onEmptyChange={setNoResumable} />
      {showEmpty && deviceKey ? (
        <div className="empty-state">
          <p className="empty-state__title">No transfers with this device yet</p>
          <p className="empty-state__detail">Send files, a folder, or text using the actions above.</p>
        </div>
      ) : showEmpty ? (
        <div className="hero">
          <WormholeMark size={56} />
          <h3 className="hero__title">Send a file through the wormhole</h3>
          <ol className="hero__steps">
            <li>
              <span className="hero__step-index">1</span>
              <span className="hero__step-text">
                Turn on <strong>Visible to others</strong> so nearby devices can find you, or wait
                for one to appear under Nearby.
              </span>
            </li>
            <li>
              <span className="hero__step-index">2</span>
              <span className="hero__step-text">
                Click <strong>Connect</strong> and confirm the code matches on both screens.
              </span>
            </li>
            <li>
              <span className="hero__step-index">3</span>
              <span className="hero__step-text">
                Hit <strong>Send files</strong> on the paired device, or drop files anywhere on
                this window.
              </span>
            </li>
          </ol>
        </div>
      ) : (
        <ul className="transfer-list">
          {jobs.map((job) => (
            <TransferRow
              key={job.id}
              job={job}
              deviceName={nameFor(job.device_fullname)}
              rate={rates[job.id]}
            />
          ))}
        </ul>
      )}
    </>
  );
}
