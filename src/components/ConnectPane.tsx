import { useEffect, useState } from "react";
import { getLocalConnectionInfo, sendManualConnectRequest } from "../ipc/commands";
import { describeError } from "../lib/errors";
import { useAppStore } from "../state/store";
import type { LocalConnectionInfo } from "../types/localinfo";
import { ExpirySelect } from "./ExpirySelect";


export function ConnectPane() {
  const setOutgoingPairing = useAppStore((s) => s.setOutgoingPairing);
  const pushToast = useAppStore((s) => s.pushToast);
  const [address, setAddress] = useState("");
  const [expirySecs, setExpirySecs] = useState<number | null>(4 * 60 * 60);
  const [connecting, setConnecting] = useState(false);
  const [mine, setMine] = useState<LocalConnectionInfo | null>(null);

  useEffect(() => {
    getLocalConnectionInfo()
      .then(setMine)
      .catch(() => setMine(null));
  }, []);

  const myAddress =
    mine && mine.addresses.length > 0 ? `${mine.addresses[0]}:${mine.port}` : null;

  async function copyMine() {
    if (!myAddress) return;
    try {
      await navigator.clipboard.writeText(myAddress);
      pushToast("success", "Your address copied");
    } catch {
      pushToast("error", "Couldn't copy");
    }
  }

  async function connect() {
    const target = address.trim();
    if (!target) return;
    setConnecting(true);
    try {
      const code = await sendManualConnectRequest(target, expirySecs);
      if (code === null) {


        pushToast("success", "Connected");
      } else {
        setOutgoingPairing({ fullname: target, displayName: target, code });
      }
    } catch (e) {
      pushToast("error", describeError(e));
    } finally {
      setConnecting(false);
    }
  }

  return (
    <div className="conn">
      <p className="conn__intro">
        Reach a device that isn't under <strong>Nearby</strong> — for networks that block mDNS
        discovery, or a device across the internet. Both need the app open.
      </p>

      <div className="conn__card">
        <label className="conn__field">
          <span className="conn__label">Device address</span>
          <div className="conn__input-wrap">
            <svg className="conn__input-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
              <path d="M9 12h6M9 8H7a4 4 0 0 0 0 8h2M15 8h2a4 4 0 0 1 0 8h-2" />
            </svg>
            <input
              className="conn__input"
              type="text"
              inputMode="decimal"
              placeholder="192.168.1.5 : 58821"
              value={address}
              spellCheck={false}
              onChange={(e) => setAddress(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void connect();
              }}
            />
          </div>
        </label>

        <label className="conn__field">
          <span className="conn__label">Trust this device for</span>
          <ExpirySelect value={expirySecs} onChange={setExpirySecs} />
        </label>

        <button
          className="btn btn--accent conn__go"
          onClick={() => void connect()}
          disabled={connecting || address.trim().length === 0}
        >
          {connecting ? "Connecting…" : "Connect"}
        </button>

        <p className="conn__hint">
          You'll both see a confirm code — check it matches on the other screen before accepting.
        </p>
      </div>

      {myAddress ? (
        <div className="conn__mine">
          <div className="conn__mine-head">
            <span className="conn__mine-title">Your address</span>
            <span className="conn__mine-sub">share this so others can reach you</span>
          </div>
          <div className="conn__mine-row">
            <code className="conn__mine-value">{myAddress}</code>
            <button className="btn btn--outline btn--small" onClick={() => void copyMine()}>
              Copy
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
