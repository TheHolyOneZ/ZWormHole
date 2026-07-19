import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  readFolderTree,
  runSpeedTest,
  sendClipboardImage,
  sendFiles,
  sendText,
} from "../ipc/commands";
import { describeError } from "../lib/errors";
import { deviceLabel } from "../lib/format";
import { useAppStore } from "../state/store";
import { Dropdown } from "./Dropdown";
import { TransferPane } from "./TransferPane";

const SPEED_TEST_SIZE_MB = 1024;


const svg = { viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 1.8, strokeLinecap: "round", strokeLinejoin: "round" } as const;
const FilesIcon = () => (
  <svg {...svg}>
    <path d="M9 3h6l4 4v10a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
    <path d="M15 3v4h4" />
  </svg>
);
const FolderIcon = () => (
  <svg {...svg}>
    <path d="M3 7a2 2 0 0 1 2-2h3.5l2 2H19a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
  </svg>
);
const ImageIcon = () => (
  <svg {...svg}>
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <circle cx="8.5" cy="9.5" r="1.5" />
    <path d="M21 15l-5-5L5 21" />
  </svg>
);
const SendIcon = () => (
  <svg {...svg}>
    <path d="M22 2 11 13" />
    <path d="M22 2 15 22l-4-9-9-4z" />
  </svg>
);
const ChartIcon = () => (
  <svg {...svg}>
    <path d="M5 20V10M12 20V4M19 20v-7" />
  </svg>
);
const BoltIcon = () => (
  <svg {...svg}>
    <path d="M13 2 4 14h7l-1 8 9-12h-7z" />
  </svg>
);
const LockIcon = () => (
  <svg {...svg}>
    <rect x="5" y="11" width="14" height="10" rx="2" />
    <path d="M8 11V7a4 4 0 0 1 8 0v4" />
  </svg>
);


export function FocusView() {
  const activePeers = useAppStore((s) => s.activePeers);
  const trusted = useAppStore((s) => s.trusted);
  const focus = useAppStore((s) => s.focusDevice);
  const setFocus = useAppStore((s) => s.setFocusDevice);
  const profiles = useAppStore((s) => s.profiles);
  const openBenchmark = useAppStore((s) => s.setBenchmarkTarget);
  const setFolderPick = useAppStore((s) => s.setFolderPick);
  const pushToast = useAppStore((s) => s.pushToast);
  const setBusy = useAppStore((s) => s.setBusy);

  const [text, setText] = useState("");
  const [sendingText, setSendingText] = useState(false);
  const [testing, setTesting] = useState(false);
  const [lastTest, setLastTest] = useState<string | null>(null);
  const taRef = useRef<HTMLTextAreaElement | null>(null);


  useEffect(() => {
    const el = taRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [text]);


  useEffect(() => {
    if (activePeers.length === 0) {
      if (focus !== null) setFocus(null);
    } else if (!activePeers.some((p) => p.device_key === focus)) {
      setFocus(activePeers[0].device_key);
    }
  }, [activePeers, focus, setFocus]);

  const peer = activePeers.find((p) => p.device_key === focus) ?? null;

  if (activePeers.length === 0 || !peer) {
    return (
      <section className="pane pane--main">
        <div className="empty-state">
          <p className="empty-state__title">No connected device to focus on</p>
          <p className="empty-state__detail">
            Connect a device from Nearby or Connect by IP, then it'll appear here to work with.
          </p>
        </div>
      </section>
    );
  }

  const key = peer.device_key;
  const profile = profiles[key];
  const nickname = trusted.find((d) => d.fingerprint === key)?.nickname;
  const label = nickname || profile?.display_name || deviceLabel(key, trusted);
  const avatarUrl = profile?.avatar_data_url ?? null;
  const initials =
    label
      .replace(/[^\p{L}\p{N}]+/gu, " ")
      .trim()
      .split(/\s+/)
      .slice(0, 2)
      .map((w) => w[0])
      .join("")
      .toUpperCase() || "?";

  async function pickAndSendFiles() {
    const selected = await open({ multiple: true, directory: false });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    if (paths.length === 0) return;
    try {
      await sendFiles(key, paths);
      pushToast("success", `Sending ${paths.length} file${paths.length > 1 ? "s" : ""}`);
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function pickAndSendFolder() {
    const selected = await open({ directory: true });
    if (typeof selected !== "string") return;
    // Reading a big folder's tree can take a moment on a slow disk — show a

    setBusy("Reading folder…");
    try {
      const tree = await readFolderTree(selected);
      setFolderPick({ deviceKey: key, displayName: label, folderPath: selected, tree });
    } catch (e) {
      pushToast("error", describeError(e));
    } finally {
      setBusy(null);
    }
  }

  async function sendImage() {
    try {
      await sendClipboardImage(key);
      pushToast("success", "Sending clipboard image");
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function test() {
    setTesting(true);
    try {
      const r = await runSpeedTest(key, SPEED_TEST_SIZE_MB);
      const summary = `${r.send_mbps.toFixed(0)}↑ / ${r.receive_mbps.toFixed(0)}↓ Mbps`;
      setLastTest(summary);
      pushToast("success", summary);
    } catch (e) {
      pushToast("error", describeError(e));
    } finally {
      setTesting(false);
    }
  }

  async function sendTheText() {
    const body = text.trim();
    if (!body) return;
    setSendingText(true);
    try {
      await sendText(key, body);
      pushToast("success", "Message sent");
      setText("");
    } catch (e) {
      pushToast("error", describeError(e));
    } finally {
      setSendingText(false);
    }
  }

  return (
    <section className="pane pane--main">

      <div className="focusv" data-device-key={key}>
        <header className="focusv__id">
          <div className="focusv__avatar">
            {avatarUrl ? (
              <img className="focusv__avatar-img" src={avatarUrl} alt="" />
            ) : (
              <span>{initials}</span>
            )}
          </div>
          <div className="focusv__meta">
            <div className="focusv__name" title={label}>
              {label}
            </div>
            <div className="focusv__status">
              <span className="focusv__dot" aria-hidden />
              Connected
              <span className="focusv__status-sep" aria-hidden />
              <span className="focusv__crypto" tabIndex={0} role="button" aria-label="Encryption details">
                <LockIcon />
                Encrypted
                <div className="focusv__crypto-pop" role="tooltip">
                  <div className="focusv__crypto-pop-head">End-to-end encrypted</div>
                  <dl className="focusv__crypto-list">
                    <div>
                      <dt>Channel</dt>
                      <dd>Noise over QUIC</dd>
                    </div>
                    <div>
                      <dt>Handshake</dt>
                      <dd>Noise NNpsk0 · X25519</dd>
                    </div>
                    <div>
                      <dt>Cipher</dt>
                      <dd>ChaCha20-Poly1305</dd>
                    </div>
                    <div>
                      <dt>Hash</dt>
                      <dd>BLAKE2s</dd>
                    </div>
                    <div>
                      <dt>Integrity</dt>
                      <dd>BLAKE3 · per chunk &amp; file</dd>
                    </div>
                  </dl>
                  <p className="focusv__crypto-foot">
                    Reconnects use Noise IK keyed to each device's identity.
                  </p>
                </div>
              </span>
              {lastTest ? (
                <>
                  <span className="focusv__status-sep" aria-hidden />
                  <BoltIcon />
                  {lastTest}
                </>
              ) : null}
            </div>
            {peer.remote_addr ? (
              <button
                className="focusv__addr"
                title="The address you're connected to them on — click to copy"
                onClick={() => {
                  void navigator.clipboard
                    .writeText(peer.remote_addr)
                    .then(() => pushToast("success", "Copied"))
                    .catch(() => pushToast("error", "Couldn't copy"));
                }}
              >
                {peer.remote_addr}
              </button>
            ) : null}
          </div>
          <div className="focusv__id-right">
            <button
              className="focusv__icon-btn"
              title="Benchmark this connection"
              aria-label="Benchmark this connection"
              onClick={() => openBenchmark({ fingerprint: key, displayName: label })}
            >
              <ChartIcon />
            </button>
            <button
              className="focusv__icon-btn"
              title="Test connection speed"
              aria-label="Test connection speed"
              onClick={() => void test()}
              disabled={testing}
            >
              <BoltIcon />
            </button>
            {activePeers.length > 1 ? (
              <Dropdown
                className="focusv__switch"
                options={activePeers.map((p) => ({
                  value: p.device_key,
                  label: deviceLabel(p.device_key, trusted),
                }))}
                value={key}
                onChange={setFocus}
                aria-label="Focused device"
              />
            ) : null}
          </div>
        </header>

        <section className="focusv__block">
          <div className="focusv__block-head">
            <span className="focusv__block-title">Send</span>
            <span className="focusv__block-hint">
              Drag &amp; drop files or folders anywhere on this page to send
            </span>
          </div>
          <div className="focusv__actions">
            <button className="focusv__action" onClick={() => void pickAndSendFiles()}>
              <span className="focusv__action-icon">
                <FilesIcon />
              </span>
              <span className="focusv__action-label">Files</span>
              <span className="focusv__action-sub">One or more</span>
            </button>
            <button className="focusv__action" onClick={() => void pickAndSendFolder()}>
              <span className="focusv__action-icon">
                <FolderIcon />
              </span>
              <span className="focusv__action-label">Folder</span>
              <span className="focusv__action-sub">Zipped &amp; sent</span>
            </button>
            <button className="focusv__action" onClick={() => void sendImage()}>
              <span className="focusv__action-icon">
                <ImageIcon />
              </span>
              <span className="focusv__action-label">Image</span>
              <span className="focusv__action-sub">From clipboard</span>
            </button>
          </div>
        </section>

        <section className="focusv__block">
          <div className="focusv__block-head">
            <span className="focusv__block-title">Message</span>
          </div>
          <div className="focusv__composer">
            <textarea
              ref={taRef}
              className="focusv__composer-input"
              placeholder={`Write to ${label}…`}
              value={text}
              rows={1}
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void sendTheText();
              }}
              spellCheck={false}
            />
            <div className="focusv__composer-bar">
              <span className="focusv__composer-hint">
                {text.trim().length > 0 ? (
                  <>
                    <kbd>⌘</kbd>
                    <kbd>↵</kbd> to send
                  </>
                ) : (
                  "Text goes over the same encrypted channel"
                )}
              </span>
              <button
                className="focusv__composer-send"
                onClick={() => void sendTheText()}
                disabled={sendingText || text.trim().length === 0}
              >
                <SendIcon />
                {sendingText ? "Sending…" : "Send"}
              </button>
            </div>
          </div>
        </section>

        <section className="focusv__feed">
          <div className="focusv__block-head">
            <span className="focusv__block-title">Transfers</span>
          </div>
          <div className="focusv__feed-body">
            <TransferPane deviceKey={key} />
          </div>
        </section>
      </div>
    </section>
  );
}
