import { useState } from "react";
import { sendConnectRequest } from "../ipc/commands";
import { describeError } from "../lib/errors";
import { useAppStore } from "../state/store";
import type { NearbyDevicePayload } from "../types/discovery";
import { DeviceInfoMenu } from "./DeviceInfoMenu";
import { DeviceTile } from "./DeviceTile";
import { EmptyState } from "./EmptyState";
import { ExpirySelect } from "./ExpirySelect";
import { SidebarSection } from "./SidebarSection";
import { Switch } from "./Switch";

export function NearbyPane() {
  const nearby = useAppStore((s) => s.nearby);
  const activePeers = useAppStore((s) => s.activePeers);
  const visible = useAppStore((s) => s.visible);
  const browsing = useAppStore((s) => s.browsing);
  const toggleVisible = useAppStore((s) => s.toggleVisible);
  const setOutgoingPairing = useAppStore((s) => s.setOutgoingPairing);
  const pushToast = useAppStore((s) => s.pushToast);
  const refreshTrusted = useAppStore((s) => s.refreshTrusted);
  const refreshActivePeers = useAppStore((s) => s.refreshActivePeers);
  const [expirySecs, setExpirySecs] = useState<number | null>(4 * 60 * 60);
  const [connecting, setConnecting] = useState<Set<string>>(new Set());
  const [infoMenu, setInfoMenu] = useState<{ device: NearbyDevicePayload; x: number; y: number } | null>(
    null,
  );


  const connectedFps = new Set(activePeers.map((p) => p.device_key));
  const byFingerprint = new Map<string, NearbyDevicePayload>();
  for (const d of Object.values(nearby)) {
    if (connectedFps.has(d.fingerprint)) continue;
    byFingerprint.set(d.fingerprint, d);
  }
  const devices = [...byFingerprint.values()];


  function preferredAddress(addresses: string[]): string {
    return addresses.find((a) => !a.includes(":")) ?? addresses[0] ?? "";
  }

  async function connect(fullname: string, displayName: string) {


    if (connecting.has(fullname)) return;
    setConnecting((s) => new Set(s).add(fullname));
    try {
      const code = await sendConnectRequest(fullname, expirySecs);
      if (code === null) {


        void refreshTrusted();
        void refreshActivePeers();
        return;
      }
      setOutgoingPairing({ fullname, displayName, code });
    } catch (e) {
      pushToast("error", describeError(e));
    } finally {
      setConnecting((s) => {
        const next = new Set(s);
        next.delete(fullname);
        return next;
      });
    }
  }

  return (
    <SidebarSection
      title="Nearby"
      count={devices.length}
      action={
        <Switch
          label="Visible"
          checked={visible}
          onChange={() => void toggleVisible()}
          title={
            visible
              ? "Other devices on this network can see you"
              : "You're hidden — other devices can't find you to send a request"
          }
        />
      }
    >
      {browsing ? <p className="scan-status">Scanning for devices…</p> : null}

      {devices.length > 0 ? (
        <ExpirySelect value={expirySecs} onChange={setExpirySecs} />
      ) : null}

      {devices.length === 0 ? (
        <EmptyState
          title="No devices found yet"
          detail="Discovery needs the same subnet with multicast allowed — some corporate or guest networks block it."
        />
      ) : (
        <ul className="device-list">
          {devices.map((d) => (
            <li key={d.fullname}>
              <DeviceTile
                name={d.hostname}
                subtitle={preferredAddress(d.addresses)}
                os={d.os}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setInfoMenu({ device: d, x: e.clientX, y: e.clientY });
                }}
                action={
                  <button
                    className="btn btn--accent btn--small"
                    onClick={() => void connect(d.fullname, d.hostname)}
                    disabled={connecting.has(d.fullname)}
                  >
                    {connecting.has(d.fullname) ? "Connecting…" : "Connect"}
                  </button>
                }
              />
            </li>
          ))}
        </ul>
      )}

      {infoMenu ? (
        <DeviceInfoMenu
          device={infoMenu.device}
          x={infoMenu.x}
          y={infoMenu.y}
          onClose={() => setInfoMenu(null)}
        />
      ) : null}
    </SidebarSection>
  );
}
