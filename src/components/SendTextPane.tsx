import { useEffect, useState } from "react";
import { sendClipboardImage, sendText } from "../ipc/commands";
import { describeError } from "../lib/errors";
import { formatDeviceName } from "../lib/format";
import { useAppStore } from "../state/store";
import { Dropdown } from "./Dropdown";


export function SendTextPane() {
  const activePeers = useAppStore((s) => s.activePeers);
  const pushToast = useAppStore((s) => s.pushToast);
  const [target, setTarget] = useState<string>("");
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);


  useEffect(() => {
    if (activePeers.length === 0) {
      setTarget("");
    } else if (!activePeers.some((p) => p.device_key === target)) {
      setTarget(activePeers[0].device_key);
    }
  }, [activePeers, target]);

  if (activePeers.length === 0) {
    return (
      <div className="empty-state">
        <p className="empty-state__title">No connected device</p>
        <p className="empty-state__detail">
          Connect a device first — then you can send it text from here.
        </p>
      </div>
    );
  }

  async function pasteClipboard() {
    try {
      const clip = await navigator.clipboard.readText();
      if (clip) setText((t) => (t ? `${t}\n${clip}` : clip));
    } catch {
      pushToast("error", "Couldn't read the clipboard");
    }
  }

  async function send() {
    const body = text.trim();
    if (!body || !target) return;
    setSending(true);
    try {
      await sendText(target, body);
      pushToast("success", "Text sent");
      setText("");
    } catch (e) {
      pushToast("error", describeError(e));
    } finally {
      setSending(false);
    }
  }

  async function sendImage() {
    if (!target) return;
    try {
      await sendClipboardImage(target);
      pushToast("success", "Sending clipboard image");
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  return (
    <div className="stext">
      <div className="stext__to">
        <span className="stext__to-label">To</span>
        <Dropdown
          className="stext__target"
          options={activePeers.map((p) => ({
            value: p.device_key,
            label: formatDeviceName(p.display_name),
          }))}
          value={target}
          onChange={setTarget}
          aria-label="Recipient device"
        />
      </div>

      <div className="stext__composer">
        <textarea
          className="stext__input"
          placeholder="Type or paste a message…"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void send();
          }}
          spellCheck={false}
        />
        <div className="stext__bar">
          <div className="stext__bar-left">
            <button className="stext__ghost" onClick={() => void pasteClipboard()}>
              Paste
            </button>
            <button
              className="stext__ghost"
              onClick={() => void sendImage()}
              title="Send an image from your clipboard"
            >
              Send image
            </button>
          </div>
          <button
            className="stext__send"
            onClick={() => void send()}
            disabled={sending || text.trim().length === 0}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
              <path d="M22 2 11 13M22 2 15 22l-4-9-9-4z" />
            </svg>
            {sending ? "Sending…" : "Send"}
          </button>
        </div>
      </div>
    </div>
  );
}
