import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { open } from "@tauri-apps/plugin-dialog";
import {
  readFolderTree,
  revokeTrustedDevice,
  runSpeedTest,
  sendClipboardImage,
  sendFiles,
  setDeviceAutoAccept,
  setDeviceFavorite,
  setDeviceNickname,
} from "../ipc/commands";
import { describeError } from "../lib/errors";
import { deviceInitials, deviceLabel, formatExpiry } from "../lib/format";
import { useAppStore } from "../state/store";
import type { TrustedDevicePayload } from "../types/trusted";

const SPEED_TEST_SIZE_MB = 1024;


export function DeviceRow({
  device,
  connected,
}: {
  device: TrustedDevicePayload;
  connected: boolean;
}) {
  const trusted = useAppStore((s) => s.trusted);
  const pushToast = useAppStore((s) => s.pushToast);
  const setFolderPick = useAppStore((s) => s.setFolderPick);
  const openBenchmark = useAppStore((s) => s.setBenchmarkTarget);
  const refreshTrusted = useAppStore((s) => s.refreshTrusted);
  const blockDevice = useAppStore((s) => s.blockDevice);
  const setFocusDevice = useAppStore((s) => s.setFocusDevice);
  const setViewMode = useAppStore((s) => s.setViewMode);
  const dropHoverKey = useAppStore((s) => s.dropHoverKey);

  const [menuOpen, setMenuOpen] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [testing, setTesting] = useState(false);
  const moreRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [menuPos, setMenuPos] = useState({ left: 0, top: 0, visible: false });


  const [menuPoint, setMenuPoint] = useState<{ x: number; y: number } | null>(null);

  const fp = device.fingerprint;
  const profile = useAppStore((s) => s.profiles[fp]);


  const label = device.nickname || profile?.display_name || deviceLabel(fp, trusted);
  const avatarUrl = profile?.avatar_data_url ?? null;
  const dropTarget = connected && dropHoverKey === fp;


  useLayoutEffect(() => {
    if (!menuOpen) {
      setMenuPos((p) => (p.visible ? { ...p, visible: false } : p));
      return;
    }
    const menu = menuRef.current;
    if (!menu) return;
    const w = menu.offsetWidth;
    const h = menu.offsetHeight;
    const pad = 8;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let px: number;
    let py: number;
    let alignRight: boolean;
    if (menuPoint) {
      px = menuPoint.x;
      py = menuPoint.y;
      alignRight = false;
    } else if (moreRef.current) {
      const a = moreRef.current.getBoundingClientRect();
      px = a.right;
      py = a.bottom + 4;
      alignRight = true;
    } else {
      return;
    }
    let left = alignRight ? px - w : px;
    left = Math.max(pad, Math.min(left, vw - w - pad));
    let top = py;
    if (top + h > vh - pad) top = Math.max(pad, py - h - (menuPoint ? 0 : 8));
    top = Math.max(pad, Math.min(top, vh - h - pad));
    setMenuPos({ left, top, visible: true });
  }, [menuOpen, menuPoint]);

  function openMenuAt(e: React.MouseEvent) {
    e.preventDefault();
    setMenuPoint({ x: e.clientX, y: e.clientY });
    setMenuOpen(true);
  }

  function focusThis() {
    if (!connected) return;
    setFocusDevice(fp);
    setViewMode("focus");
  }


  async function run(fn: () => Promise<void>) {
    setMenuOpen(false);
    try {
      await fn();
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function pickFiles() {
    const sel = await open({ multiple: true, directory: false });
    if (!sel) return;
    const paths = Array.isArray(sel) ? sel : [sel];
    if (paths.length === 0) return;
    await sendFiles(fp, paths);
    pushToast("success", `Sending ${paths.length} file${paths.length > 1 ? "s" : ""} to ${label}`);
  }

  async function pickFolder() {
    const sel = await open({ directory: true });
    if (typeof sel !== "string") return;
    const tree = await readFolderTree(sel);
    setFolderPick({ deviceKey: fp, displayName: label, folderPath: sel, tree });
  }

  async function sendImage() {
    await sendClipboardImage(fp);
    pushToast("success", `Sending clipboard image to ${label}`);
  }

  async function test() {
    setTesting(true);
    try {
      const r = await runSpeedTest(fp, SPEED_TEST_SIZE_MB);
      pushToast("success", `${label}: ${r.send_mbps.toFixed(0)}↑ / ${r.receive_mbps.toFixed(0)}↓ Mbps`);
    } finally {
      setTesting(false);
    }
  }

  function startRename() {
    setDraft(device.nickname || "");
    setEditing(true);
    setMenuOpen(false);
  }

  async function saveRename() {
    try {
      await setDeviceNickname(fp, draft.trim());
      setEditing(false);
      await refreshTrusted();
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function toggleFavorite() {
    setMenuOpen(false);
    try {
      await setDeviceFavorite(fp, !device.favorite);
      await refreshTrusted();
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function toggleAutoAccept() {
    setMenuOpen(false);
    try {
      await setDeviceAutoAccept(fp, !device.auto_accept);
      await refreshTrusted();
      pushToast(
        "success",
        !device.auto_accept
          ? `Auto-accepting transfers from ${label}`
          : `Auto-accept off for ${label}`,
      );
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function revoke() {
    setMenuOpen(false);
    try {
      await revokeTrustedDevice(fp);
      await refreshTrusted();
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function block() {
    setMenuOpen(false);
    try {
      await blockDevice(fp, device.display_name);
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  if (editing) {
    return (
      <div className="side-dev side-dev--editing">
        <input
          className="side-dev__rename-input"
          value={draft}
          autoFocus
          spellCheck={false}
          placeholder={device.display_name}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void saveRename();
            if (e.key === "Escape") setEditing(false);
          }}
        />
        <button className="side-dev__rename-btn" onClick={() => void saveRename()} title="Save">
          ✓
        </button>
        <button className="side-dev__rename-btn" onClick={() => setEditing(false)} title="Cancel">
          ✕
        </button>
      </div>
    );
  }

  return (
    <div
      className={`side-dev${connected ? " side-dev--connected" : ""}${
        dropTarget ? " side-dev--drop" : ""
      }`}
      data-device-key={connected ? fp : undefined}
      onContextMenu={openMenuAt}
    >
      <button
        className="side-dev__main"
        onClick={focusThis}
        disabled={!connected}
        title={connected ? "Open in Focus" : undefined}
      >
        <span className={`side-dev__avatar${connected ? " side-dev__avatar--online" : ""}`}>
          {avatarUrl ? (
            <img className="side-dev__avatar-img" src={avatarUrl} alt="" />
          ) : (
            deviceInitials(label)
          )}
        </span>
        <span className="side-dev__meta">
          <span className="side-dev__name">
            {device.favorite ? <span className="side-dev__star">★</span> : null}
            {label}
          </span>
          <span className="side-dev__sub">
            {connected ? "Connected" : `Offline · ${formatExpiry(device.expires_at)}`}
          </span>
        </span>
      </button>

      <button
        ref={moreRef}
        className="side-dev__more"
        onClick={() => {
          setMenuPoint(null);
          setMenuOpen((o) => !o);
        }}
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        title="Actions (or right-click the device)"
      >
        ⋯
      </button>

      {menuOpen
        ? createPortal(
            <>
              <div className="side-menu-backdrop" onClick={() => setMenuOpen(false)} />
              <div
                ref={menuRef}
                className="side-menu"
                role="menu"
                style={{
                  left: menuPos.left,
                  top: menuPos.top,
                  visibility: menuPos.visible ? "visible" : "hidden",
                }}
              >
                {connected ? (
              <>
                <button className="side-menu__item" onClick={() => void run(pickFiles)}>
                  Send files
                </button>
                <button className="side-menu__item" onClick={() => void run(pickFolder)}>
                  Send folder
                </button>
                <button className="side-menu__item" onClick={() => void run(sendImage)}>
                  Send clipboard image
                </button>
                <div className="side-menu__sep" />
                <button
                  className="side-menu__item"
                  onClick={() => void run(test)}
                  disabled={testing}
                >
                  {testing ? "Testing…" : "Test connection"}
                </button>
                <button
                  className="side-menu__item"
                  onClick={() => {
                    setMenuOpen(false);
                    openBenchmark({ fingerprint: fp, displayName: label });
                  }}
                >
                  Benchmark
                </button>
                <button className="side-menu__item" onClick={() => void toggleAutoAccept()}>
                  Auto-accept {device.auto_accept ? "✓" : ""}
                </button>
                <div className="side-menu__sep" />
              </>
            ) : null}
            <button className="side-menu__item" onClick={startRename}>
              Rename
            </button>
            <button className="side-menu__item" onClick={() => void toggleFavorite()}>
              {device.favorite ? "Remove favorite" : "Add to favorites"}
            </button>
            <button
              className="side-menu__item side-menu__item--danger"
              onClick={() => void revoke()}
            >
              Revoke
            </button>
            <button
              className="side-menu__item side-menu__item--danger"
              onClick={() => void block()}
              title="Revoke and refuse all future connections from this device"
            >
              Block
            </button>
              </div>
            </>,
            document.body,
          )
        : null}
    </div>
  );
}
