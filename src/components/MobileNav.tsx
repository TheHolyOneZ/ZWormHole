import { useAppStore } from "../state/store";

function DevicesIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <rect x="2.5" y="4" width="10" height="7.5" rx="1.4" stroke="currentColor" strokeWidth="1.4" />
      <path d="M5.5 14.5h4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      <rect x="13.5" y="7" width="4" height="9" rx="1.2" stroke="currentColor" strokeWidth="1.4" />
    </svg>
  );
}

function DashboardIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <rect x="3" y="3" width="6" height="6" rx="1.4" stroke="currentColor" strokeWidth="1.4" />
      <rect x="11" y="3" width="6" height="6" rx="1.4" stroke="currentColor" strokeWidth="1.4" />
      <rect x="3" y="11" width="6" height="6" rx="1.4" stroke="currentColor" strokeWidth="1.4" />
      <rect x="11" y="11" width="6" height="6" rx="1.4" stroke="currentColor" strokeWidth="1.4" />
    </svg>
  );
}

function FocusIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <circle cx="10" cy="10" r="6.5" stroke="currentColor" strokeWidth="1.4" />
      <circle cx="10" cy="10" r="2.5" stroke="currentColor" strokeWidth="1.4" />
    </svg>
  );
}

function SettingsIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <circle cx="10" cy="10" r="2.6" stroke="currentColor" strokeWidth="1.4" />
      <path
        d="M10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function MobileNav() {
  const viewMode = useAppStore((s) => s.viewMode);
  const mobilePane = useAppStore((s) => s.mobilePane);
  const setViewMode = useAppStore((s) => s.setViewMode);
  const setMobilePane = useAppStore((s) => s.setMobilePane);
  const setSettingsOpen = useAppStore((s) => s.setSettingsOpen);
  const connected = useAppStore((s) => s.activePeers.length);

  const items = [
    {
      key: "devices",
      label: "Devices",
      icon: <DevicesIcon />,
      active: mobilePane === "devices",
      badge: connected > 0 ? connected : null,
      onClick: () => setMobilePane("devices"),
    },
    {
      key: "dashboard",
      label: "Dashboard",
      icon: <DashboardIcon />,
      active: mobilePane === "main" && viewMode === "dashboard",
      badge: null,
      onClick: () => setViewMode("dashboard"),
    },
    {
      key: "focus",
      label: "Focus",
      icon: <FocusIcon />,
      active: mobilePane === "main" && viewMode === "focus",
      badge: null,
      onClick: () => setViewMode("focus"),
    },
    {
      key: "settings",
      label: "Settings",
      icon: <SettingsIcon />,
      active: false,
      badge: null,
      onClick: () => setSettingsOpen(true),
    },
  ];

  return (
    <nav className="mobile-nav" aria-label="Sections">
      {items.map((item) => (
        <button
          key={item.key}
          className={`mobile-nav__btn${item.active ? " mobile-nav__btn--active" : ""}`}
          aria-current={item.active ? "page" : undefined}
          onClick={item.onClick}
        >
          <span className="mobile-nav__icon">
            {item.icon}
            {item.badge !== null && <span className="mobile-nav__badge">{item.badge}</span>}
          </span>
          <span className="mobile-nav__label">{item.label}</span>
        </button>
      ))}
    </nav>
  );
}
