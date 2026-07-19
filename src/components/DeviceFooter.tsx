import { useEffect, useState } from "react";
import { getLocalDeviceName } from "../ipc/commands";
import { useAppStore } from "../state/store";

export function DeviceFooter() {
  const [name, setName] = useState<string | null>(null);
  const visible = useAppStore((s) => s.visible);
  const openInfo = useAppStore((s) => s.setThisDeviceOpen);

  useEffect(() => {
    void getLocalDeviceName().then(setName);
  }, []);

  return (
    <button
      className="device-footer device-footer--button"
      onClick={() => openInfo(true)}
      title="Show this device's address & connection details"
    >
      <span className={`status-dot ${visible ? "status-dot--online" : "status-dot--idle"}`} />
      <div className="device-footer__info">
        <span className="device-footer__label">This device</span>
        <span className="device-footer__name">{name ?? "…"}</span>
      </div>
      <span className="device-footer__chevron" aria-hidden>
        ›
      </span>
    </button>
  );
}
