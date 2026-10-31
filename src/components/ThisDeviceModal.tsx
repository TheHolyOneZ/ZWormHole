import { useEffect, useState } from "react";
import {
  clearMyAvatar,
  disableInternetReachable,
  enableInternetReachable,
  getLocalConnectionInfo,
  getMyProfile,
  getWanStatus,
  refreshAdvertisement,
  setMyAvatar,
  setMyDisplayName,
} from "../ipc/commands";
import { describeError } from "../lib/errors";
import { pickImage } from "../lib/platform";
import { deviceInitials } from "../lib/format";
import { useAppStore } from "../state/store";
import type { LocalConnectionInfo } from "../types/localinfo";
import type { MyProfilePayload } from "../types/profile";
import type { WanStatus } from "../types/wan";
import { Switch } from "./Switch";

function CopyRow({ label, value }: { label: string; value: string }) {
  const pushToast = useAppStore((s) => s.pushToast);
  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      pushToast("success", "Copied");
    } catch {
      pushToast("error", "Couldn't copy");
    }
  }
  return (
    <div className="thisdev__row">
      <span className="thisdev__label">{label}</span>
      <span className="thisdev__value">{value}</span>
      <button className="btn btn--outline btn--small" onClick={() => void copy()}>
        Copy
      </button>
    </div>
  );
}


export function ThisDeviceModal() {
  const open = useAppStore((s) => s.thisDeviceOpen);
  const close = useAppStore((s) => s.setThisDeviceOpen);
  const visible = useAppStore((s) => s.visible);
  const pushToast = useAppStore((s) => s.pushToast);
  const [info, setInfo] = useState<LocalConnectionInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [wan, setWan] = useState<WanStatus | null>(null);
  const [wanBusy, setWanBusy] = useState(false);
  const [wanError, setWanError] = useState<string | null>(null);
  const [profile, setProfile] = useState<MyProfilePayload | null>(null);
  const [nameDraft, setNameDraft] = useState("");

  useEffect(() => {
    if (!open) return;
    setInfo(null);
    setError(null);
    setWanError(null);
    getLocalConnectionInfo()
      .then(setInfo)
      .catch((e) => setError(describeError(e)));
    getWanStatus()
      .then(setWan)
      .catch(() => setWan(null));
    getMyProfile()
      .then((p) => {
        setProfile(p);
        setNameDraft(p.display_name ?? "");
      })
      .catch(() => setProfile(null));
  }, [open]);

  async function reloadProfile() {
    const p = await getMyProfile();
    setProfile(p);
    setNameDraft(p.display_name ?? "");

    void refreshAdvertisement().catch(() => {});
  }

  async function commitName() {
    const next = nameDraft.trim();
    if (next === (profile?.display_name ?? "")) return;
    try {
      await setMyDisplayName(next || null);
      await reloadProfile();
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function pickAvatar() {
    try {
      const selected = await pickImage([
        { name: "Image", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] },
      ]);
      if (!selected) return;
      await setMyAvatar(selected);
      await reloadProfile();
      pushToast("success", "Avatar updated");
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function removeAvatar() {
    try {
      await clearMyAvatar();
      await reloadProfile();
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function toggleWan() {
    setWanBusy(true);
    setWanError(null);
    try {
      if (wan) {
        await disableInternetReachable();
        setWan(null);
      } else {
        setWan(await enableInternetReachable());
      }
    } catch (e) {
      setWanError(describeError(e));
    } finally {
      setWanBusy(false);
    }
  }

  if (!open) return null;

  const primary = info && info.addresses.length > 0 ? info.addresses[0] : null;
  const shareAddress = primary && info ? `${primary}:${info.port}` : null;

  return (
    <div className="modal-backdrop" onClick={() => close(false)}>
      <div className="modal thisdev" onClick={(e) => e.stopPropagation()}>
        <p className="pairing-modal__eyebrow">This device</p>
        <h2 className="thisdev__name">
          {profile?.effective_name ?? info?.device_name ?? "…"}
        </h2>

        <div className="thisdev__profile">
          <div className="thisdev__avatar">
            {profile?.avatar_data_url ? (
              <img className="thisdev__avatar-img" src={profile.avatar_data_url} alt="" />
            ) : (
              <span>{deviceInitials(profile?.effective_name ?? info?.device_name ?? "?")}</span>
            )}
          </div>
          <div className="thisdev__profile-fields">
            <label className="thisdev__profile-label">Display name</label>
            <input
              className="thisdev__name-input"
              value={nameDraft}
              placeholder={info?.device_name ?? "This device"}
              spellCheck={false}
              onChange={(e) => setNameDraft(e.target.value)}
              onBlur={() => void commitName()}
              onKeyDown={(e) => {
                if (e.key === "Enter") (e.target as HTMLInputElement).blur();
              }}
            />
            <div className="thisdev__avatar-actions">
              <button className="btn btn--outline btn--small" onClick={() => void pickAvatar()}>
                {profile?.avatar_data_url ? "Change photo" : "Add photo"}
              </button>
              {profile?.avatar_data_url ? (
                <button className="thisdev__avatar-remove" onClick={() => void removeAvatar()}>
                  Remove
                </button>
              ) : null}
            </div>
            <p className="thisdev__profile-hint">
              Your name &amp; photo are shared with devices you pair with and update on the next
              connection. No photo? They'll see your initials.
            </p>
          </div>
        </div>

        {error ? (
          <p className="thisdev__error">{error}</p>
        ) : !info ? (
          <p className="thisdev__loading">Reading connection details…</p>
        ) : (
          <>
            <p className="thisdev__intro">
              Another device reaches you by pasting your address into{" "}
              <strong>Connect by IP</strong>. Use the <strong>local</strong> address if they're on
              the same network; for a different network, turn on{" "}
              <strong>Reachable over the internet</strong> below and share that address instead. You
              need <strong>Visible to others</strong>{" "}
              {visible ? "(it's on)" : "on, or an accepted request"}.
            </p>

            {shareAddress ? (
              <CopyRow label="Local address" value={shareAddress} />
            ) : (
              <p className="thisdev__error">
                Couldn't determine a LAN address — check your network connection.
              </p>
            )}
            <CopyRow label="Fingerprint" value={info.fingerprint} />

            <div className="thisdev__wan">
              <div className="thisdev__wan-head">
                <div className="thisdev__wan-title">
                  <Switch
                    label="Reachable over the internet"
                    checked={!!wan}
                    onChange={() => void toggleWan()}
                  />
                  <span className="badge-beta" title="Beta — router port-mapping isn't verified on every network yet">
                    BETA
                  </span>
                </div>
                {wanBusy ? <span className="thisdev__wan-busy">Asking your router…</span> : null}
              </div>

              {wanError ? <p className="thisdev__error">{wanError}</p> : null}

              {wan ? (
                <>
                  <CopyRow
                    label="Internet address"
                    value={`${wan.external_ip}:${wan.external_port}`}
                  />
                  {wan.public ? (
                    <p className="thisdev__wan-note">
                      This is the address to share with someone on <strong>another network</strong>
                      {" "}— they paste it into <strong>Connect by IP</strong>. (Your local address
                      above only works on the same network.) Mapped via {wan.protocol}; stays live
                      while the app is open.
                    </p>
                  ) : (
                    <p className="thisdev__error">
                      Your router's address is private (carrier-grade NAT), so this won't be
                      reachable from the internet. You'd need a public IP or a shared VPN.
                    </p>
                  )}
                </>
              ) : (
                <p className="thisdev__wan-note">
                  Off by default. This asks your router (UPnP / NAT-PMP) to forward the port — no
                  relay or cloud involved. Needs UPnP enabled on the router; won't work behind
                  carrier-grade NAT.
                </p>
              )}
            </div>
          </>
        )}

        <div className="pairing-modal__actions">
          <button className="btn btn--accent" onClick={() => close(false)}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
