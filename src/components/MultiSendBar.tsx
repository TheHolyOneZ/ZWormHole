import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { readFolderTree, sendFilesMulti } from "../ipc/commands";
import { describeError } from "../lib/errors";
import { formatDeviceName } from "../lib/format";
import { useAppStore } from "../state/store";


export function MultiSendBar() {
  const activePeers = useAppStore((s) => s.activePeers);
  const pushToast = useAppStore((s) => s.pushToast);
  const setFolderPick = useAppStore((s) => s.setFolderPick);

  const [selected, setSelected] = useState<Set<string>>(new Set());

  if (activePeers.length < 2) return null;

  const keys = activePeers.map((p) => p.device_key);
  const effective =
    selected.size === 0
      ? new Set(keys)
      : new Set([...selected].filter((k) => keys.includes(k)));
  const targetKeys = [...effective];

  function toggle(key: string) {
    setSelected((prev) => {
      const base = prev.size === 0 ? new Set(keys) : new Set(prev);
      if (base.has(key)) base.delete(key);
      else base.add(key);


      if (base.size === 0 || base.size === keys.length) return new Set();
      return base;
    });
  }

  async function sendFilesToTargets() {
    const sel = await open({ multiple: true, directory: false });
    if (!sel) return;
    const paths = Array.isArray(sel) ? sel : [sel];
    if (paths.length === 0) return;
    try {
      const r = await sendFilesMulti(targetKeys, paths);
      const skip = r.skipped.length ? `, ${r.skipped.length} skipped` : "";
      pushToast(
        "success",
        `Sending ${paths.length} file${paths.length > 1 ? "s" : ""} to ${r.jobs.length} device${
          r.jobs.length === 1 ? "" : "s"
        }${skip}`,
      );
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function sendFolderToTargets() {
    const sel = await open({ directory: true });
    if (typeof sel !== "string") return;
    try {
      const tree = await readFolderTree(sel);
      setFolderPick({
        deviceKey: targetKeys[0],
        deviceKeys: targetKeys,
        displayName: `${targetKeys.length} devices`,
        folderPath: sel,
        tree,
      });
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  return (
    <div className="multisend">
      <div className="multisend__head">
        <span className="multisend__title">Send to multiple</span>
        <span className="multisend__count">
          {targetKeys.length} of {keys.length}
        </span>
      </div>
      <div className="multisend__chips">
        {activePeers.map((p) => {
          const on = effective.has(p.device_key);
          return (
            <button
              key={p.device_key}
              className={`multisend__chip${on ? " multisend__chip--on" : ""}`}
              onClick={() => toggle(p.device_key)}
              title={on ? "Sending to this device" : "Excluded"}
            >
              {formatDeviceName(p.display_name)}
            </button>
          );
        })}
      </div>
      <div className="multisend__actions">
        <button className="btn btn--accent btn--small" onClick={() => void sendFilesToTargets()}>
          Send files
        </button>
        <button className="btn btn--accent btn--small" onClick={() => void sendFolderToTargets()}>
          Send folder
        </button>
      </div>
    </div>
  );
}
