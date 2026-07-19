import { useState } from "react";
import { useAppStore } from "../state/store";
import { DeviceRow } from "./DeviceRow";
import { EmptyState } from "./EmptyState";
import { MultiSendBar } from "./MultiSendBar";
import { SidebarSection } from "./SidebarSection";

type Filter = "all" | "connected" | "favorites";
const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "connected", label: "Connected" },
  { id: "favorites", label: "Favorites" },
];

export function TrustedPane() {
  const trusted = useAppStore((s) => s.trusted);
  const activePeers = useAppStore((s) => s.activePeers);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");

  const connectedKeys = new Set(activePeers.map((p) => p.device_key));
  const q = query.trim().toLowerCase();

  const list = trusted
    .filter((d) => {
      if (filter === "connected" && !connectedKeys.has(d.fingerprint)) return false;
      if (filter === "favorites" && !d.favorite) return false;
      if (q && !(d.nickname || d.display_name).toLowerCase().includes(q)) return false;
      return true;
    })


    .slice()
    .sort(
      (a, b) =>
        Number(connectedKeys.has(b.fingerprint)) - Number(connectedKeys.has(a.fingerprint)),
    );


  const showFilters = trusted.length > 4;

  return (
    <SidebarSection title="Trusted" count={trusted.length}>
      <MultiSendBar />

      {showFilters ? (
        <div className="side-filter">
          <input
            className="side-filter__search"
            type="text"
            placeholder="Search devices…"
            value={query}
            spellCheck={false}
            onChange={(e) => setQuery(e.target.value)}
          />
          <div className="side-filter__chips">
            {FILTERS.map((f) => (
              <button
                key={f.id}
                className={`side-filter__chip${filter === f.id ? " side-filter__chip--on" : ""}`}
                onClick={() => setFilter(f.id)}
              >
                {f.label}
              </button>
            ))}
          </div>
        </div>
      ) : null}

      {trusted.length === 0 ? (
        <EmptyState title="No trusted devices yet" detail="Devices you pair with appear here." />
      ) : list.length === 0 ? (
        <p className="side-empty">No devices match.</p>
      ) : (
        <div className="side-dev-list">
          {list.map((d) => (
            <DeviceRow
              key={d.fingerprint}
              device={d}
              connected={connectedKeys.has(d.fingerprint)}
            />
          ))}
        </div>
      )}
    </SidebarSection>
  );
}
