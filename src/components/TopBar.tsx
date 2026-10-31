import { getCurrentWindow } from "@tauri-apps/api/window";
import { isMobile } from "../lib/platform";
import { useAppStore } from "../state/store";
import { DownloadsTray } from "./DownloadsTray";
import { WormholeMark } from "./WormholeMark";

const win = getCurrentWindow();

function WindowControls() {
  return (
    <div className="window-controls">
      <button
        className="window-controls__btn"
        onClick={() => void win.minimize()}
        aria-label="Minimize"
      >
        <svg width="10" height="10" viewBox="0 0 10 10">
          <line x1="1.5" y1="5" x2="8.5" y2="5" stroke="currentColor" strokeWidth="1.2" />
        </svg>
      </button>
      <button
        className="window-controls__btn"
        onClick={() => void win.toggleMaximize()}
        aria-label="Maximize"
      >
        <svg width="10" height="10" viewBox="0 0 10 10">
          <rect
            x="1.5"
            y="1.5"
            width="7"
            height="7"
            rx="1"
            stroke="currentColor"
            strokeWidth="1.2"
            fill="none"
          />
        </svg>
      </button>
      <button
        className="window-controls__btn window-controls__btn--close"
        onClick={() => void win.close()}
        aria-label="Close"
      >
        <svg width="10" height="10" viewBox="0 0 10 10">
          <line x1="1.5" y1="1.5" x2="8.5" y2="8.5" stroke="currentColor" strokeWidth="1.2" />
          <line x1="8.5" y1="1.5" x2="1.5" y2="8.5" stroke="currentColor" strokeWidth="1.2" />
        </svg>
      </button>
    </div>
  );
}

export function TopBar() {
  const viewMode = useAppStore((s) => s.viewMode);
  const setViewMode = useAppStore((s) => s.setViewMode);

  return (
    <header className="top-bar" data-tauri-drag-region>
      <div className="top-bar__brand" data-tauri-drag-region>
        <WormholeMark size={19} spin />
        <span className="top-bar__name" data-tauri-drag-region>
          ZWormHole
        </span>
      </div>

      <div className="mode-toggle" role="tablist" aria-label="View mode">
        <button
          className={`mode-toggle__btn${viewMode === "dashboard" ? " mode-toggle__btn--active" : ""}`}
          role="tab"
          aria-selected={viewMode === "dashboard"}
          onClick={() => setViewMode("dashboard")}
        >
          Dashboard
        </button>
        <button
          className={`mode-toggle__btn${viewMode === "focus" ? " mode-toggle__btn--active" : ""}`}
          role="tab"
          aria-selected={viewMode === "focus"}
          onClick={() => setViewMode("focus")}
        >
          Focus
        </button>
      </div>

      <div className="top-bar__right">
        <DownloadsTray />
        {!isMobile && <WindowControls />}
      </div>
    </header>
  );
}
