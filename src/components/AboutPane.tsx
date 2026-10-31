import { useEffect, useState, type ReactNode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openExternal } from "../ipc/commands";
import { describeError } from "../lib/errors";
import { useAppStore } from "../state/store";
import { WormholeMark } from "./WormholeMark";

const FALLBACK_VERSION = "0.2.0";


const svg = {
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.7,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

const CheckIcon = () => (
  <svg {...svg}>
    <path d="M20 6 9 17l-5-5" />
  </svg>
);
const GithubIcon = () => (
  <svg {...svg}>
    <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22" />
  </svg>
);
const CodeIcon = () => (
  <svg {...svg}>
    <path d="m16 18 6-6-6-6M8 6l-6 6 6 6" />
  </svg>
);
const DownloadIcon = () => (
  <svg {...svg}>
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3" />
  </svg>
);
const GlobeIcon = () => (
  <svg {...svg}>
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3a15 15 0 0 1 4 9 15 15 0 0 1-4 9 15 15 0 0 1-4-9 15 15 0 0 1 4-9z" />
  </svg>
);
const LayersIcon = () => (
  <svg {...svg}>
    <path d="m12 2 9 5-9 5-9-5 9-5zM3 12l9 5 9-5M3 17l9 5 9-5" />
  </svg>
);
const SwapIcon = () => (
  <svg {...svg}>
    <path d="M7 17V7m0 0-3 3m3-3 3 3M17 7v10m0 0 3-3m-3 3-3-3" />
  </svg>
);
const LockIcon = () => (
  <svg {...svg}>
    <rect x="5" y="11" width="14" height="10" rx="2" />
    <path d="M8 11V7a4 4 0 0 1 8 0v4" />
  </svg>
);
const ShieldIcon = () => (
  <svg {...svg}>
    <path d="M12 3l7 3v5c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V6l7-3z" />
    <path d="m9 12 2 2 4-4" />
  </svg>
);
const KeyIcon = () => (
  <svg {...svg}>
    <circle cx="7.5" cy="15.5" r="3.5" />
    <path d="m10 13 8-8M15 5l3 3M18 8l2-2" />
  </svg>
);
const BroadcastIcon = () => (
  <svg {...svg}>
    <circle cx="12" cy="12" r="2" />
    <path d="M4.9 4.9a10 10 0 0 0 0 14.2M19.1 4.9a10 10 0 0 1 0 14.2M7.8 7.8a6 6 0 0 0 0 8.4M16.2 7.8a6 6 0 0 1 0 8.4" />
  </svg>
);
const ArrowUpRight = () => (
  <svg {...svg}>
    <path d="M7 17 17 7M9 7h8v8" />
  </svg>
);

const LINKS: { label: string; sub: string; url: string; icon: ReactNode }[] = [
  {
    label: "Author on GitHub",
    sub: "github.com/TheHolyOneZ",
    url: "https://github.com/TheHolyOneZ",
    icon: <GithubIcon />,
  },
  {
    label: "Source code",
    sub: "TheHolyOneZ/ZWormHole",
    url: "https://github.com/TheHolyOneZ/ZWormHole",
    icon: <CodeIcon />,
  },
  {
    label: "Download builds",
    sub: "zsync.eu/zwormhole",
    url: "https://zsync.eu/zwormhole/",
    icon: <DownloadIcon />,
  },
  {
    label: "More projects",
    sub: "zsync.eu",
    url: "https://zsync.eu/",
    icon: <GlobeIcon />,
  },
];

const CAN_DO = [
  "Send files, whole folders, clipboard images, or text — device to device",
  "Send the same thing to several devices at once, and resume interrupted transfers",
  "Pair with a short confirm code, then reconnect automatically — with device profiles & avatars",
  "Find devices on the LAN automatically, connect by IP, or reach across the internet with no relay",
  "Choose exactly which files to send from a folder; cap bandwidth; auto-accept or block a device",
  "Live speed graphs, ETA, a 3-way bottleneck breakdown, and a built-in benchmark",
];

const STACK: { icon: ReactNode; label: string; value: string }[] = [
  { icon: <LayersIcon />, label: "Runtime", value: "Tauri 2 · Rust · React + TypeScript" },
  { icon: <SwapIcon />, label: "Transport", value: "QUIC (quinn) — UDP, multiplexed streams" },
  {
    icon: <LockIcon />,
    label: "Encryption",
    value: "Noise over QUIC (snow) · X25519 · ChaCha20-Poly1305 · BLAKE2s",
  },
  { icon: <ShieldIcon />, label: "Integrity", value: "BLAKE3 — verified per chunk & per file" },
  { icon: <KeyIcon />, label: "Pairing", value: "SPAKE2 confirm code + Noise IK fast-reconnect" },
  { icon: <BroadcastIcon />, label: "Discovery", value: "mDNS on the LAN + manual IP connect" },
  { icon: <GlobeIcon />, label: "Internet (beta)", value: "UPnP / NAT-PMP router port-mapping — serverless, no relay" },
];


export function AboutPane() {
  const pushToast = useAppStore((s) => s.pushToast);
  const [version, setVersion] = useState(FALLBACK_VERSION);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  async function open(url: string) {
    try {
      await openExternal(url);
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  return (
    <div className="about">
      <div className="about__hero">
        <div className="about__glow" aria-hidden />
        <WormholeMark size={74} spin />
        <div className="about__hero-text">
          <div className="about__name-row">
            <h2 className="about__name">ZWormHole</h2>
            <span className="about__ver">v{version}</span>
          </div>
          <p className="about__tagline">
            LAN-first, end-to-end encrypted peer-to-peer file transfer. No relay, no cloud — your
            files go straight from one device to the other.
          </p>
          <div className="about__badges">
            <span className="about__badge">GPL-3.0</span>
            <span className="about__badge">Serverless</span>
            <span className="about__badge">No phone-home</span>
          </div>
        </div>
      </div>

      <section className="about__group">
        <span className="about__group-title">What it does</span>
        <div className="about__card">
          {CAN_DO.map((line) => (
            <div key={line} className="about__feature">
              <span className="about__feature-icon" aria-hidden>
                <CheckIcon />
              </span>
              <span className="about__feature-text">{line}</span>
            </div>
          ))}
        </div>
      </section>

      <section className="about__group">
        <span className="about__group-title">Tech stack</span>
        <div className="about__card">
          {STACK.map((s) => (
            <div key={s.label} className="about__stack-row">
              <span className="about__stack-icon" aria-hidden>
                {s.icon}
              </span>
              <span className="about__stack-label">{s.label}</span>
              <span className="about__stack-value">{s.value}</span>
            </div>
          ))}
        </div>
        <p className="about__note">Deeper docs live on the website and in the project README.</p>
      </section>

      <section className="about__group">
        <span className="about__group-title">Links</span>
        <div className="about__links">
          {LINKS.map((l) => (
            <button key={l.url} className="about__link" onClick={() => void open(l.url)}>
              <span className="about__link-icon" aria-hidden>
                {l.icon}
              </span>
              <span className="about__link-text">
                <span className="about__link-label">{l.label}</span>
                <span className="about__link-sub">{l.sub}</span>
              </span>
              <span className="about__link-arrow" aria-hidden>
                <ArrowUpRight />
              </span>
            </button>
          ))}
        </div>
      </section>

      <footer className="about__footer">Licensed under GPL-3.0 · © 2026 TheHolyOneZ</footer>
    </div>
  );
}
