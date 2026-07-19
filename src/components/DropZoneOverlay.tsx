import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listActivePeers, sendDropped } from "../ipc/commands";
import { describeError } from "../lib/errors";
import { useAppStore } from "../state/store";


function deviceKeyAt(physicalX: number, physicalY: number): string | null {
  const dpr = window.devicePixelRatio || 1;
  const el = document.elementFromPoint(physicalX / dpr, physicalY / dpr);
  const host = el?.closest("[data-device-key]");
  return host?.getAttribute("data-device-key") ?? null;
}

export function DropZoneOverlay() {
  const [dragging, setDragging] = useState(false);
  const pushToast = useAppStore((s) => s.pushToast);
  const setHover = useAppStore((s) => s.setDropHoverKey);
  const setBusy = useAppStore((s) => s.setBusy);
  const hoverKey = useAppStore((s) => s.dropHoverKey);
  const activePeers = useAppStore((s) => s.activePeers);

  useEffect(() => {
    const unlistenPromise = getCurrentWebview().onDragDropEvent((event) => {
      const { payload } = event;
      if (payload.type === "enter" || payload.type === "over") {
        setDragging(true);
        setHover(deviceKeyAt(payload.position.x, payload.position.y));
        return;
      }
      if (payload.type === "leave") {
        setDragging(false);
        setHover(null);
        return;
      }
      if (payload.type === "drop") {
        setDragging(false);
        const targetKey = deviceKeyAt(payload.position.x, payload.position.y);
        setHover(null);

        void (async () => {
          const active = await listActivePeers();
          if (active.length === 0) {
            pushToast("error", "Pair with a device before sending files");
            return;
          }


          let key = targetKey;
          if (!key) {
            if (active.length === 1) key = active[0].device_key;
            else {
              pushToast("error", "Drop the files onto a specific connected device");
              return;
            }
          }


          setBusy("Preparing your transfer…");
          try {
            await sendDropped(key, payload.paths);
          } catch (e) {
            pushToast("error", describeError(e));
          } finally {
            setBusy(null);
          }
        })();
      }
    });

    return () => {
      void unlistenPromise.then((fn) => fn());
    };
  }, [pushToast, setHover]);

  if (!dragging) return null;

  const hoverName = hoverKey
    ? activePeers.find((p) => p.device_key === hoverKey)?.display_name
    : null;

  return (
    <div className="drop-overlay">
      <div className="drop-overlay__inner">
        {hoverName ? `Drop to send to ${hoverName}` : "Drop onto a device to send"}
      </div>
    </div>
  );
}
