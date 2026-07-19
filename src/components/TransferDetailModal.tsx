import { formatBytes, formatDeviceName, formatDuration, formatRate, formatTimestamp, timeAgo } from "../lib/format";
import { useAppStore } from "../state/store";
import type { TransferJob } from "../types/transfer";


const MIN_TIMING_MICROS = 50_000;

type Verdict = {
  title: string;
  body: string;
  tone: "local" | "remote" | "network" | "unknown";
};


function micros(us: number): string {
  if (us <= 0) return "0ms";
  if (us >= 1_000_000) return `${(us / 1_000_000).toFixed(1)}s`;
  return `${Math.round(us / 1000)}ms`;
}


function diagnose(job: TransferJob): Verdict {
  const senderTotal = job.local_micros + job.encrypt_micros + job.socket_micros;
  if (senderTotal < MIN_TIMING_MICROS) {
    return {
      tone: "unknown",
      title: "Measuring…",
      body: "Not enough has been sent yet to tell where the time is going. Give it a moment.",
    };
  }

  const socketShare = job.socket_micros / senderTotal;
  if (socketShare < 0.5) {

    if (job.encrypt_micros >= job.local_micros) {
      return {
        tone: "local",
        title: "This device's CPU (encryption) is the bottleneck",
        body: "Most of the time is going into Noise-encrypting each chunk on this machine — not the disk, not the network. If you're running a development/debug build this is expected: the crypto runs unoptimized. A release build (or the optimized dev profile) encrypts far faster.",
      };
    }
    return {
      tone: "local",
      title: "This device's disk is the bottleneck",
      body: "Most of the time is going into reading the file off this machine's disk and hashing it. A faster source drive would help; many tiny files are also slower than one big one.",
    };
  }


  const peerTotal = job.peer_recv_wait_micros + job.peer_disk_write_micros;
  if (peerTotal < MIN_TIMING_MICROS) {
    return {
      tone: "unknown",
      title: "The connection or the other device",
      body: "Chunks are encrypted and ready but the send is stalling on the socket — so it's the network or the receiving device. Waiting for the other device to report its own timing to say which.",
    };
  }

  const peerWriteShare = job.peer_disk_write_micros / peerTotal;
  if (peerWriteShare >= 0.5) {
    return {
      tone: "remote",
      title: "The other device is the bottleneck",
      body: "The receiving device is spending its time writing to its own disk, which backpressures the link and stalls this send. It's their disk (or a busy CPU) holding things up — not your machine and not the network.",
    };
  }
  return {
    tone: "network",
    title: "The connection is the bottleneck",
    body: "This machine's disk and CPU are keeping up and the socket is stalling — data isn't crossing the connection any faster than this. Note the receiver's decryption time also counts here, so a slow debug build on their end can look the same.",
  };
}


function Sparkline({ values }: { values: number[] }) {
  const w = 260;
  const h = 46;
  const pad = 3;
  const max = Math.max(...values, 1);
  const n = values.length;
  const line = values
    .map((v, i) => {
      const x = n === 1 ? w - pad : (i / (n - 1)) * (w - pad * 2) + pad;
      const y = h - pad - (v / max) * (h - pad * 2);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  const area = `${pad},${h - pad} ${line} ${w - pad},${h - pad}`;
  return (
    <svg
      className="transfer-detail__spark"
      viewBox={`0 0 ${w} ${h}`}
      preserveAspectRatio="none"
      aria-hidden
    >
      <polygon className="transfer-detail__spark-fill" points={area} />
      <polyline className="transfer-detail__spark-line" points={line} />
    </svg>
  );
}

function Bar({ segments }: { segments: { micros: number; className: string }[] }) {
  const total = segments.reduce((s, x) => s + x.micros, 0) || 1;
  return (
    <div className="transfer-detail__split">
      {segments.map((s, i) => (
        <div
          key={i}
          className={s.className}
          style={{ width: `${Math.round((s.micros / total) * 100)}%` }}
        />
      ))}
    </div>
  );
}

function LegendRow({
  dotClass,
  label,
  value,
  total,
}: {
  dotClass: string;
  label: string;
  value: number;
  total: number;
}) {
  const pct = total > 0 ? Math.round((value / total) * 100) : 0;
  return (
    <span>
      <span className={`transfer-detail__dot ${dotClass}`} />
      {label} {pct}% · {micros(value)}
    </span>
  );
}

export function TransferDetailModal() {
  const selectedId = useAppStore((s) => s.selectedTransferId);
  const job = useAppStore((s) => (selectedId ? s.transfers[selectedId] : undefined));
  const rate = useAppStore((s) => (selectedId ? s.transferRates[selectedId] : undefined));
  const rateHistory = useAppStore((s) => (selectedId ? s.transferRateHistory[selectedId] : undefined));
  const trusted = useAppStore((s) => s.trusted);
  const close = useAppStore((s) => s.setSelectedTransfer);


  if (!selectedId || !job) return null;

  const deviceName =
    trusted.find((t) => t.fingerprint === job.device_fullname)?.display_name ??
    formatDeviceName(job.device_fullname);
  const pct = job.bytes_total > 0 ? Math.round((job.bytes_done / job.bytes_total) * 100) : 0;
  const active = job.status === "in_progress";
  const remaining = job.bytes_total - job.bytes_done;
  const etaSecs = active && rate && rate > 0 ? remaining / rate : NaN;

  const senderTotal = job.local_micros + job.encrypt_micros + job.socket_micros;
  const peerTotal = job.peer_recv_wait_micros + job.peer_disk_write_micros;
  const hasSender = senderTotal >= MIN_TIMING_MICROS;
  const hasPeer = peerTotal >= MIN_TIMING_MICROS;
  const verdict = diagnose(job);

  return (
    <div className="modal-backdrop" onClick={() => close(null)}>
      <div className="modal transfer-detail" onClick={(e) => e.stopPropagation()}>
        <p className="pairing-modal__eyebrow">Transfer</p>
        <h2 className="transfer-detail__title">{job.label || deviceName}</h2>
        <p className="transfer-detail__subtitle">
          to {deviceName}
          {job.created_at ? ` · ${timeAgo(job.created_at)}` : ""}
        </p>

        <dl className="transfer-detail__stats">
          <dt>Status</dt>
          <dd className={`transfer-detail__status transfer-detail__status--${job.status}`}>
            {job.status.replace("_", " ")}
          </dd>
          {job.created_at ? (
            <>
              <dt>Sent</dt>
              <dd>{formatTimestamp(job.created_at)}</dd>
            </>
          ) : null}
          <dt>Progress</dt>
          <dd>
            {formatBytes(job.bytes_done)} / {formatBytes(job.bytes_total)} · {pct}%
          </dd>
          {active ? (
            <>
              <dt>Speed</dt>
              <dd>{formatRate(rate ?? 0)}</dd>
              <dt>Time left</dt>
              <dd>{Number.isFinite(etaSecs) ? formatDuration(etaSecs) : "—"}</dd>
            </>
          ) : null}
        </dl>

        {rateHistory && rateHistory.length >= 2 ? (
          <div className="transfer-detail__spark-wrap">
            <div className="transfer-detail__spark-head">
              <span>Speed over time</span>
              <span>peak {formatRate(Math.max(...rateHistory))}</span>
            </div>
            <Sparkline values={rateHistory} />
          </div>
        ) : null}

        <div className="transfer-detail__diag">
          <p className={`transfer-detail__verdict transfer-detail__verdict--${verdict.tone}`}>
            {verdict.title}
          </p>
          <p className="transfer-detail__verdict-body">{verdict.body}</p>

          {hasSender ? (
            <div className="transfer-detail__side">
              <p className="transfer-detail__side-label">This device (sender)</p>
              <Bar
                segments={[
                  { micros: job.local_micros, className: "transfer-detail__split-a" },
                  { micros: job.encrypt_micros, className: "transfer-detail__split-c" },
                  { micros: job.socket_micros, className: "transfer-detail__split-b" },
                ]}
              />
              <div className="transfer-detail__split-legend transfer-detail__split-legend--stack">
                <LegendRow
                  dotClass="transfer-detail__dot--a"
                  label="Reading + hashing"
                  value={job.local_micros}
                  total={senderTotal}
                />
                <LegendRow
                  dotClass="transfer-detail__dot--c"
                  label="Encrypting (CPU)"
                  value={job.encrypt_micros}
                  total={senderTotal}
                />
                <LegendRow
                  dotClass="transfer-detail__dot--b"
                  label="Sending (socket)"
                  value={job.socket_micros}
                  total={senderTotal}
                />
              </div>
            </div>
          ) : null}

          {hasPeer ? (
            <div className="transfer-detail__side">
              <p className="transfer-detail__side-label">{deviceName} (receiver)</p>
              <Bar
                segments={[
                  { micros: job.peer_disk_write_micros, className: "transfer-detail__split-a" },
                  { micros: job.peer_recv_wait_micros, className: "transfer-detail__split-b" },
                ]}
              />
              <div className="transfer-detail__split-legend">
                <LegendRow
                  dotClass="transfer-detail__dot--a"
                  label="Writing to disk"
                  value={job.peer_disk_write_micros}
                  total={peerTotal}
                />
                <LegendRow
                  dotClass="transfer-detail__dot--b"
                  label="Receiving + decrypting"
                  value={job.peer_recv_wait_micros}
                  total={peerTotal}
                />
              </div>
            </div>
          ) : (
            <p className="transfer-detail__pending">
              Waiting for the other device to report its timing…
            </p>
          )}
        </div>

        <div className="pairing-modal__actions">
          <button className="btn btn--accent" onClick={() => close(null)}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
