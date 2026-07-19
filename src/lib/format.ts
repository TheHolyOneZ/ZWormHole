export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = n / 1024;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(1)} ${units[unitIndex]}`;
}


export function formatTimestamp(unixSecs: number): string {
  if (!unixSecs) return "";
  const d = new Date(unixSecs * 1000);
  const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  if (d.toDateString() === new Date().toDateString()) return time;
  return `${d.toLocaleDateString([], { month: "short", day: "numeric" })}, ${time}`;
}


export function timeAgo(unixSecs: number): string {
  if (!unixSecs) return "";
  const s = Math.max(0, Math.floor(Date.now() / 1000 - unixSecs));
  if (s < 45) return "just now";
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} minute${m === 1 ? "" : "s"} ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h} hour${h === 1 ? "" : "s"} ago`;
  const d = Math.floor(h / 24);
  return `${d} day${d === 1 ? "" : "s"} ago`;
}


export function formatMbps(mbps: number): string {
  if (!Number.isFinite(mbps) || mbps <= 0) return "—";
  if (mbps >= 1000) return `${(mbps / 1000).toFixed(2)} Gbps`;
  return `${mbps.toFixed(0)} Mbps`;
}


export function formatRate(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec <= 0) return "—";
  return `${formatBytes(bytesPerSec)}/s`;
}


export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "—";
  const s = Math.round(seconds);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}


export function formatDeviceName(raw: string): string {
  if (/^[0-9a-f]{64}$/i.test(raw)) {
    return `${raw.slice(0, 6)}…${raw.slice(-6)}`;
  }
  return raw;
}


export function deviceInitials(label: string): string {
  return (
    label
      .replace(/[^\p{L}\p{N}]+/gu, " ")
      .trim()
      .split(/\s+/)
      .slice(0, 2)
      .map((w) => w[0])
      .join("")
      .toUpperCase() || "?"
  );
}


export function deviceLabel(
  raw: string,
  trusted: { fingerprint: string; display_name: string; nickname: string | null }[],
): string {
  const t = trusted.find((d) => d.fingerprint === raw);
  return t?.nickname || t?.display_name || formatDeviceName(raw);
}


export function formatExpiry(expiresAt: number | null): string {
  if (expiresAt === null) return "Trusted indefinitely";

  const remainingSecs = expiresAt - Math.floor(Date.now() / 1000);
  if (remainingSecs <= 0) return "Session expired";

  const hours = Math.floor(remainingSecs / 3600);
  const minutes = Math.floor((remainingSecs % 3600) / 60);
  if (hours >= 1) return `Expires in ${hours}h ${minutes}m`;
  return `Expires in ${minutes}m`;
}
