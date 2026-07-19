import { deviceInitials, formatDeviceName } from "../lib/format";
import { useAppStore } from "../state/store";


export function TextReceivedModal() {
  const message = useAppStore((s) => s.incomingText);
  const clear = useAppStore((s) => s.clearIncomingText);
  const pushToast = useAppStore((s) => s.pushToast);
  const profiles = useAppStore((s) => s.profiles);
  const trusted = useAppStore((s) => s.trusted);

  if (!message) return null;

  const profile = profiles[message.device_key];
  const nickname = trusted.find((t) => t.fingerprint === message.device_key)?.nickname;
  const name = nickname || profile?.display_name || formatDeviceName(message.display_name);
  const avatarUrl = profile?.avatar_data_url ?? null;

  async function copy() {
    try {
      await navigator.clipboard.writeText(message!.text);
      pushToast("success", "Copied");
    } catch {
      pushToast("error", "Couldn't copy to clipboard");
    }
  }

  return (
    <div className="modal-backdrop" onClick={() => clear()}>
      <div className="modal textmsg" onClick={(e) => e.stopPropagation()}>
        <div className="textmsg__head">
          <div className="textmsg__avatar">
            {avatarUrl ? (
              <img className="textmsg__avatar-img" src={avatarUrl} alt="" />
            ) : (
              <span>{deviceInitials(name)}</span>
            )}
          </div>
          <div className="textmsg__who">
            <span className="textmsg__name" title={name}>
              {name}
            </span>
            <span className="textmsg__sub">sent you a message</span>
          </div>
        </div>

        <div className="textmsg__body">{message.text}</div>

        <div className="pairing-modal__actions">
          <button className="btn btn--outline" onClick={() => clear()}>
            Close
          </button>
          <button className="btn btn--accent" onClick={() => void copy()}>
            Copy
          </button>
        </div>
      </div>
    </div>
  );
}
