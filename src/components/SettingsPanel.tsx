import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import {
  getBackgroundListenerEnabled,
  getTransferSettings,
  setAutoAccept,
  setAutoExtract,
  setBackgroundListenerEnabled,
  setMaxConcurrentSends,
  setMaxSendMbS,
  setQuicPort,
  setSaveDir,
} from "../ipc/commands";
import { describeError } from "../lib/errors";
import { os, pickSaveDirectory, saveLocationLabel } from "../lib/platform";
import type { AutoExtractMode } from "../types/settings";
import { useAppStore } from "../state/store";
import { Dropdown } from "./Dropdown";
import { Switch } from "./Switch";


function ExpandableText({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLSpanElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [overflows, setOverflows] = useState(false);

  useLayoutEffect(() => {
    const el = ref.current;
    if (el) setOverflows(el.scrollHeight > el.clientHeight + 1);
  }, [children]);

  return (
    <>
      <span ref={ref} className={`setx__row-desc${expanded ? " setx__row-desc--open" : ""}`}>
        {children}
      </span>
      {overflows || expanded ? (
        <button className="setx__more" onClick={() => setExpanded((e) => !e)}>
          {expanded ? "Less" : "More"}
        </button>
      ) : null}
    </>
  );
}

const EXTRACT_OPTIONS: { value: AutoExtractMode; label: string }[] = [
  { value: "off", label: "Leave as .zip" },
  { value: "extract_and_delete", label: "Extract, delete the .zip" },
  { value: "extract_and_keep", label: "Extract, keep the .zip" },
];

export function SettingsPanel() {
  const open = useAppStore((s) => s.settingsOpen);
  const setOpen = useAppStore((s) => s.setSettingsOpen);
  const trusted = useAppStore((s) => s.trusted);
  const activePeers = useAppStore((s) => s.activePeers);
  const blocked = useAppStore((s) => s.blocked);
  const refreshBlocked = useAppStore((s) => s.refreshBlocked);
  const unblock = useAppStore((s) => s.unblockDevice);
  const pushToast = useAppStore((s) => s.pushToast);
  const [backgroundListener, setBackgroundListener] = useState(false);
  const [autoAccept, setAutoAcceptState] = useState(false);
  const [saveDir, setSaveDirState] = useState<string | null>(null);
  const [defaultSaveDir, setDefaultSaveDir] = useState<string | null>(null);
  const [autoExtract, setAutoExtractState] = useState<AutoExtractMode>("off");
  const [maxSendMbS, setMaxSendMbSState] = useState(0);
  const [maxConcurrent, setMaxConcurrentState] = useState(0);


  const [quicPort, setQuicPortState] = useState(0);
  const [portDraft, setPortDraft] = useState("");
  const [portBusy, setPortBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    void getBackgroundListenerEnabled().then(setBackgroundListener);
    void getTransferSettings().then((s) => {
      setAutoAcceptState(s.auto_accept);
      setSaveDirState(s.save_dir);
      setDefaultSaveDir(s.default_save_dir);
      setAutoExtractState(s.auto_extract);
      setMaxSendMbSState(s.max_send_mb_s);
      setMaxConcurrentState(s.max_concurrent_sends);
      setQuicPortState(s.quic_port);
      setPortDraft(s.quic_port ? String(s.quic_port) : "");
    });
    void refreshBlocked();
  }, [open, refreshBlocked]);

  async function commitMaxSendMbS(value: number) {
    const clamped = Number.isFinite(value) && value > 0 ? Math.floor(value) : 0;
    setMaxSendMbSState(clamped);
    await setMaxSendMbS(clamped);
  }

  async function commitMaxConcurrent(value: number) {
    const clamped = Number.isFinite(value) && value > 0 ? Math.floor(value) : 0;
    setMaxConcurrentState(clamped);
    await setMaxConcurrentSends(clamped);
  }

  async function commitQuicPort() {
    const value = Number(portDraft);
    const clamped =
      Number.isFinite(value) && value > 0 ? Math.min(Math.floor(value), 65535) : 0;


    if (clamped === quicPort) {
      setPortDraft(clamped ? String(clamped) : "");
      return;
    }
    setPortBusy(true);
    try {


      const actual = await setQuicPort(clamped);
      setQuicPortState(clamped);
      setPortDraft(clamped ? String(clamped) : "");
      pushToast(
        "success",
        clamped === 0
          ? `Automatic port — now listening on ${actual}`
          : `Now listening on port ${actual}`,
      );
    } catch (e) {


      setPortDraft(quicPort ? String(quicPort) : "");
      pushToast("error", describeError(e));
    } finally {
      setPortBusy(false);
    }
  }

  async function toggleBackgroundListener() {
    const next = !backgroundListener;
    await setBackgroundListenerEnabled(next);
    setBackgroundListener(next);
  }

  async function toggleAutoAccept() {
    const next = !autoAccept;
    await setAutoAccept(next);
    setAutoAcceptState(next);
  }

  async function chooseSaveDir() {
    try {
      const selected = await pickSaveDirectory(true);
      if (!selected) return;
      await setSaveDir(selected);
      setSaveDirState(selected);
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function resetSaveDir() {
    await setSaveDir(null);
    setSaveDirState(null);
  }

  async function chooseAutoExtract(mode: AutoExtractMode) {
    await setAutoExtract(mode);
    setAutoExtractState(mode);
  }

  if (!open) return null;

  const connectedKeys = new Set(activePeers.map((p) => p.device_key));
  const saveLabel = saveDir ?? defaultSaveDir ?? "Choose folder…";
  const shortPath = (p: string) => {
    const parts = p.split(/[\\/]/).filter(Boolean);
    return parts.length > 2 ? `…/${parts.slice(-2).join("/")}` : p;
  };

  return (
    <div className="modal-backdrop" onClick={() => setOpen(false)}>
      <div className="modal setx" onClick={(e) => e.stopPropagation()}>
        <header className="setx__head">
          <h2 className="setx__title">Settings</h2>
          <button className="setx__close" onClick={() => setOpen(false)} title="Close" aria-label="Close">
            ✕
          </button>
        </header>

        <div className="setx__body">
          <div className="setx__group">
            <span className="setx__group-title">Transfers</span>
            <div className="setx__card">
              <div className="setx__row">
                <div className="setx__text">
                  <span className="setx__row-title">Auto-accept from paired devices</span>
                  <ExpandableText>
                    Off by default — otherwise every incoming transfer shows the files and waits for
                    you to accept.
                  </ExpandableText>
                </div>
                <div className="setx__ctl">
                  <Switch label="" checked={autoAccept} onChange={() => void toggleAutoAccept()} />
                </div>
              </div>

              <div className="setx__row">
                <div className="setx__text">
                  <span className="setx__row-title">Send speed limit</span>
                  <ExpandableText>
                    Caps outgoing speed so a transfer doesn't hog the link. Applies live, even mid-
                    transfer. 0 = unlimited.
                  </ExpandableText>
                </div>
                <div className="setx__ctl setx__ctl--cap">
                  <input
                    className="setx__cap-input"
                    type="number"
                    min={0}
                    step={1}
                    value={maxSendMbS || ""}
                    placeholder="0"
                    onChange={(e) => setMaxSendMbSState(Number(e.target.value))}
                    onBlur={(e) => void commitMaxSendMbS(Number(e.target.value))}
                  />
                  <span className="setx__cap-unit">MB/s</span>
                </div>
              </div>

              <div className="setx__row">
                <div className="setx__text">
                  <span className="setx__row-title">Max simultaneous sends</span>
                  <ExpandableText>
                    Sends beyond this wait their turn (shown as “queued”) so a big fan-out doesn't
                    thrash the disk. 0 = unlimited.
                  </ExpandableText>
                </div>
                <div className="setx__ctl setx__ctl--cap">
                  <input
                    className="setx__cap-input"
                    type="number"
                    min={0}
                    step={1}
                    value={maxConcurrent || ""}
                    placeholder="0"
                    onChange={(e) => setMaxConcurrentState(Number(e.target.value))}
                    onBlur={(e) => void commitMaxConcurrent(Number(e.target.value))}
                  />
                </div>
              </div>

              <div className="setx__row">
                <div className="setx__text">
                  <span className="setx__row-title">Save incoming files to</span>
                  <ExpandableText>
                    {saveDir
                      ? saveDir
                      : `Default · ${os === "ios" ? saveLocationLabel : (defaultSaveDir ?? saveLocationLabel)}`}
                  </ExpandableText>
                </div>
                <div className="setx__ctl">
                  {saveDir ? (
                    <button className="btn btn--outline btn--small" onClick={() => void resetSaveDir()}>
                      Reset
                    </button>
                  ) : null}
                  <button className="btn btn--outline btn--small" onClick={() => void chooseSaveDir()}>
                    {saveDir ? shortPath(saveLabel) : "Choose…"}
                  </button>
                </div>
              </div>

              <div className="setx__row">
                <div className="setx__text">
                  <span className="setx__row-title">Received folders</span>
                  <ExpandableText>
                    Only affects folder sends (they arrive as a .zip) — plain files are never
                    touched. When you're asked to accept, this is the pre-selected choice; when
                    auto-accept is on for the sender, it's applied automatically.
                  </ExpandableText>
                </div>
                <div className="setx__ctl">
                  <Dropdown
                    className="setx__extract"
                    options={EXTRACT_OPTIONS}
                    value={autoExtract}
                    onChange={(v) => void chooseAutoExtract(v as AutoExtractMode)}
                    aria-label="Received folders default"
                  />
                </div>
              </div>
            </div>
          </div>

          <div className="setx__group">
            <span className="setx__group-title">General</span>
            <div className="setx__card">
              <div className="setx__row">
                <div className="setx__text">
                  <span className="setx__row-title">Run in background</span>
                  <ExpandableText>
                    {os === "android"
                      ? "Keep receiving while ZWormHole is in the background. Android shows a persistent notification while this is on."
                      : os === "ios"
                        ? "iOS pauses apps shortly after you leave them. Transfers in progress get a short grace period to finish — keep ZWormHole open for longer transfers."
                        : "Keep listening for requests when the window is closed. A tray icon stays available to reopen or quit."}
                  </ExpandableText>
                </div>
                {os !== "ios" ? (
                  <div className="setx__ctl">
                    <Switch
                      label=""
                      checked={backgroundListener}
                      onChange={() => void toggleBackgroundListener()}
                    />
                  </div>
                ) : null}
              </div>

              <div className="setx__row">
                <div className="setx__text">
                  <span className="setx__row-title">Connection port</span>
                  <ExpandableText>
                    Most people don't need this — devices on the same network find each other
                    automatically on whatever port is in use. Set a fixed port only for firewall or
                    port-forward rules; the other side then connects to your address with that port
                    (e.g. 192.168.1.5:59000). 0 = automatic.
                  </ExpandableText>
                </div>
                <div className="setx__ctl setx__ctl--cap">
                  <input
                    className="setx__cap-input"
                    type="number"
                    min={0}
                    max={65535}
                    step={1}
                    value={portDraft}
                    placeholder="Auto"
                    disabled={portBusy}
                    onChange={(e) => setPortDraft(e.target.value)}
                    onBlur={() => void commitQuicPort()}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                    }}
                  />
                </div>
              </div>
            </div>
          </div>

          <div className="setx__group">
            <span className="setx__group-title">
              Trusted devices
              <span className="setx__group-count">{trusted.length}</span>
            </span>
            <div className="setx__card">
              {trusted.length === 0 ? (
                <p className="setx__note">No trusted devices yet — pair one from Nearby or Connect by IP.</p>
              ) : (
                <>
                  <ul className="setx__devices">
                    {trusted.map((d) => (
                      <li key={d.fingerprint} className="setx__device">
                        <span
                          className={`setx__device-dot${
                            connectedKeys.has(d.fingerprint) ? " setx__device-dot--on" : ""
                          }`}
                          aria-hidden
                        />
                        <span className="setx__device-name">{d.nickname || d.display_name}</span>
                        {d.favorite ? <span className="setx__device-star">★</span> : null}
                      </li>
                    ))}
                  </ul>
                  <p className="setx__note">Rename, favorite, auto-accept or revoke from the sidebar.</p>
                </>
              )}
            </div>
          </div>

          {blocked.length > 0 ? (
            <div className="setx__group">
              <span className="setx__group-title">
                Blocked devices
                <span className="setx__group-count">{blocked.length}</span>
              </span>
              <div className="setx__card">
                <ul className="setx__devices">
                  {blocked.map((d) => (
                    <li key={d.fingerprint} className="setx__device setx__device--blocked">
                      <span className="setx__device-name">{d.display_name}</span>
                      <button
                        className="setx__unblock"
                        onClick={() => void unblock(d.fingerprint)}
                        title="Allow this device to request again"
                      >
                        Unblock
                      </button>
                    </li>
                  ))}
                </ul>
                <p className="setx__note">
                  Blocked devices can't pair or reconnect and stay hidden from Nearby.
                </p>
              </div>
            </div>
          ) : null}

          <p className="setx__footer">
            Discovery is local (mDNS); you can also connect by IP across networks. Every connection
            is direct, peer-to-peer — no relay, no cloud, no phone-home.
          </p>
        </div>
      </div>
    </div>
  );
}
