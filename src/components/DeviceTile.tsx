import type { ReactNode } from "react";

interface DeviceTileProps {
  name: string;
  subtitle: string;
  os?: string;
  statusDot?: "online" | "idle";
  action?: ReactNode;
  onContextMenu?: (e: React.MouseEvent) => void;
}

const OS_LABEL: Record<string, string> = {
  windows: "Windows",
  macos: "macOS",
  linux: "Linux",
};

export function DeviceTile({
  name,
  subtitle,
  os,
  statusDot = "online",
  action,
  onContextMenu,
}: DeviceTileProps) {
  return (
    <div className="device-tile" onContextMenu={onContextMenu}>
      <span className={`status-dot status-dot--${statusDot}`} aria-hidden />
      <div className="device-tile__info">
        <span className="device-tile__name">{name}</span>
        <span className="device-tile__subtitle">
          {os ? OS_LABEL[os] ?? os : null}
          {os && subtitle ? " · " : null}
          {subtitle}
        </span>
      </div>
      {action ? <div className="device-tile__action">{action}</div> : null}
    </div>
  );
}
