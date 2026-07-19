import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { NearbyDevicePayload } from "../types/discovery";

interface DeviceInfoMenuProps {
  device: NearbyDevicePayload;
  x: number;
  y: number;
  onClose: () => void;
}

const OS_LABEL: Record<string, string> = {
  windows: "Windows",
  macos: "macOS",
  linux: "Linux",
};

const EDGE_PADDING = 8;


export function DeviceInfoMenu({ device, x, y, onClose }: DeviceInfoMenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y, visible: false });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;


    const width = el.offsetWidth;
    const height = el.offsetHeight;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const left = Math.min(
      Math.max(EDGE_PADDING, x),
      Math.max(EDGE_PADDING, vw - width - EDGE_PADDING),
    );
    const top = Math.min(
      Math.max(EDGE_PADDING, y),
      Math.max(EDGE_PADDING, vh - height - EDGE_PADDING),
    );
    setPos({ left, top, visible: true });
  }, [x, y]);

  useEffect(() => {
    function handlePointerDown(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    }
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKey);
    };
  }, [onClose]);

  const style: React.CSSProperties = {
    left: pos.left,
    top: pos.top,


    visibility: pos.visible ? "visible" : "hidden",
  };

  return createPortal(
    <div className="context-menu" style={style} ref={ref}>
      <div className="context-menu__header">{device.hostname}</div>
      <dl className="context-menu__list">
        <dt>OS</dt>
        <dd>{OS_LABEL[device.os] ?? device.os}</dd>

        <dt>Fingerprint</dt>
        <dd className="context-menu__mono">{device.fingerprint}</dd>

        <dt>Addresses</dt>
        <dd>
          {device.addresses.map((a) => (
            <span key={a} className="context-menu__mono context-menu__address">
              {a}
            </span>
          ))}
        </dd>

        <dt>Port</dt>
        <dd className="context-menu__mono">{device.port}</dd>

        <dt>Protocol</dt>
        <dd className="context-menu__mono">{device.protocol_version}</dd>

        <dt>Session</dt>
        <dd className="context-menu__mono">{device.session_id}</dd>
      </dl>
    </div>,
    document.body,
  );
}
