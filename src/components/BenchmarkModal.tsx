import { useEffect, useState } from "react";
import {
  exportBenchmark,
  listBenchmarkHistory,
  recordBenchmark,
  runBenchmark,
} from "../ipc/commands";
import { describeError } from "../lib/errors";
import { exportTextFile } from "../lib/platform";
import { formatBytes, formatDuration, formatMbps, formatTimestamp } from "../lib/format";
import { useAppStore } from "../state/store";
import type {
  BenchHistoryEntry,
  BenchLeg,
  BenchmarkOutcome,
  BenchStats,
} from "../types/benchmark";
import { Dropdown } from "./Dropdown";

const SIZE_OPTIONS = [
  { value: "256", label: "256 MB" },
  { value: "1024", label: "1 GB" },
  { value: "5120", label: "5 GB" },
  { value: "10240", label: "10 GB" },
];


function Sparkline({ stats }: { stats: BenchStats }) {
  const w = 360;
  const h = 36;
  if (stats.samples.length < 2) {
    return <div className="benchmark__spark benchmark__spark--empty">too short to chart</div>;
  }
  const maxMbps = Math.max(...stats.samples.map((s) => s.mbps), 1);
  const maxAt = Math.max(...stats.samples.map((s) => s.at_ms), 1);
  const pts = stats.samples
    .map((s) => {
      const x = (s.at_ms / maxAt) * w;
      const y = h - (s.mbps / maxMbps) * h;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <svg className="benchmark__spark" viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none">
      <polyline points={pts} fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

function LegView({ title, leg }: { title: string; leg: BenchLeg }) {


  const r = leg.receiver;
  return (
    <div className="benchmark__leg">
      <div className="benchmark__leg-head">
        <span className="benchmark__leg-title">{title}</span>
        <span className="benchmark__leg-avg">{formatMbps(r.avg_mbps)}</span>
      </div>
      <Sparkline stats={r} />
      <div className="benchmark__leg-stats">
        <span>
          avg <strong>{formatMbps(r.avg_mbps)}</strong>
        </span>
        <span>
          peak <strong>{formatMbps(r.max_mbps)}</strong>
        </span>
        <span>
          low <strong>{formatMbps(r.min_mbps)}</strong>
        </span>
        <span>{formatDuration(r.elapsed_ms / 1000)}</span>
      </div>
    </div>
  );
}

export function BenchmarkModal() {
  const target = useAppStore((s) => s.benchmarkTarget);
  const close = useAppStore((s) => s.setBenchmarkTarget);
  const pushToast = useAppStore((s) => s.pushToast);

  const [sizeMb, setSizeMb] = useState("1024");
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<BenchmarkOutcome | null>(null);
  const [history, setHistory] = useState<BenchHistoryEntry[]>([]);

  const fingerprint = target?.fingerprint ?? null;
  useEffect(() => {
    if (!fingerprint) return;
    setResult(null);
    listBenchmarkHistory()
      .then((all) => setHistory(all.filter((h) => h.fingerprint === fingerprint)))
      .catch(() => setHistory([]));
  }, [fingerprint]);

  if (!target) return null;

  async function run() {
    setRunning(true);
    setResult(null);
    try {
      const outcome = await runBenchmark(target!.fingerprint, Number(sizeMb));
      setResult(outcome);

      await recordBenchmark(
        target!.fingerprint,
        target!.displayName,
        outcome.upload.receiver.avg_mbps,
        outcome.download.receiver.avg_mbps,
      );
      const all = await listBenchmarkHistory();
      setHistory(all.filter((h) => h.fingerprint === target!.fingerprint));
    } catch (e) {
      pushToast("error", describeError(e));
    } finally {
      setRunning(false);
    }
  }

  async function exportJson() {
    if (!result) return;
    const stamp = new Date().toISOString().replace(/[:.]/g, "-");
    const report = {
      generated_at: new Date().toISOString(),
      peer: target!.displayName,
      peer_fingerprint: target!.fingerprint,
      note: "Synthetic bidirectional throughput benchmark over the encrypted connection. Mbps values are megabits/second. 'receiver' is the authoritative delivered-throughput dataset; 'sender' is the push-side view.",
      ...result,
    };
    try {
      const saved = await exportTextFile(
        `zwormhole-benchmark-${target!.displayName}-${stamp}.json`,
        JSON.stringify(report, null, 2),
        [{ name: "JSON", extensions: ["json"] }],
        "application/json",
        exportBenchmark,
      );
      if (saved) pushToast("success", "Benchmark exported");
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  function reset() {
    if (running) return;
    close(null);
  }

  return (
    <div className="modal-backdrop" onClick={reset}>
      <div className="modal benchmark" onClick={(e) => e.stopPropagation()}>
        <p className="pairing-modal__eyebrow">Connection benchmark</p>
        <h2 className="benchmark__title">{target.displayName}</h2>
        <p className="benchmark__subtitle">
          Sends throwaway data each way over the encrypted connection to measure real
          throughput — nothing touches either disk.
        </p>

        <div className="benchmark__controls">
          <span className="benchmark__controls-label">Size each way</span>
          <Dropdown
            options={SIZE_OPTIONS}
            value={sizeMb}
            onChange={setSizeMb}
            className="benchmark__size"
            aria-label="Benchmark size per direction"
          />
          <button className="btn btn--accent" onClick={() => void run()} disabled={running}>
            {running ? "Running…" : result ? "Run again" : "Run benchmark"}
          </button>
        </div>

        {running ? (
          <p className="benchmark__hint">
            Measuring both directions… {formatBytes(Number(sizeMb) * 1024 * 1024)} up, then down.
          </p>
        ) : null}

        {result ? (
          <div className="benchmark__results">
            <LegView title={`This device → ${target.displayName}`} leg={result.upload} />
            <LegView title={`${target.displayName} → this device`} leg={result.download} />
          </div>
        ) : null}

        {history.length > 0 ? (
          <div className="benchmark__history">
            <div className="benchmark__history-head">
              <span>Recent runs</span>
              <span>up / down (Mbps)</span>
            </div>
            <ul className="benchmark__history-list">
              {history.slice(0, 6).map((h) => (
                <li key={h.id} className="benchmark__history-row">
                  <span className="benchmark__history-time">{formatTimestamp(h.at_unix)}</span>
                  <span className="benchmark__history-nums">
                    <span className="benchmark__history-up">{h.up_avg_mbps.toFixed(0)}↑</span>
                    <span className="benchmark__history-down">{h.down_avg_mbps.toFixed(0)}↓</span>
                  </span>
                </li>
              ))}
            </ul>
          </div>
        ) : null}

        <div className="pairing-modal__actions">
          <button className="btn btn--outline" onClick={reset} disabled={running}>
            Close
          </button>
          {result ? (
            <button className="btn btn--accent" onClick={() => void exportJson()}>
              Export JSON
            </button>
          ) : null}
        </div>
      </div>
    </div>
  );
}
