import { useRef } from "react";
import { confirmPairingCode } from "../ipc/commands";
import { describeError } from "../lib/errors";
import { useFocusTrap } from "../lib/useFocusTrap";
import { useAppStore } from "../state/store";

export function PairingModal() {
  const incoming = useAppStore((s) => s.incomingPairing);
  const outgoing = useAppStore((s) => s.outgoingPairing);
  const clearIncoming = useAppStore((s) => s.clearIncomingPairing);
  const setOutgoing = useAppStore((s) => s.setOutgoingPairing);
  const pushToast = useAppStore((s) => s.pushToast);
  const blockDevice = useAppStore((s) => s.blockDevice);
  const modalRef = useRef<HTMLDivElement>(null);
  useFocusTrap(modalRef, !!(incoming || outgoing));

  if (!incoming && !outgoing) return null;

  if (incoming) {
    return (
      <div className="modal-backdrop">
        <div className="modal pairing-modal" ref={modalRef} role="dialog" aria-modal="true">
          <p className="pairing-modal__eyebrow">Connection request</p>
          <h2 className="pairing-modal__requester">{incoming.requester_name} wants to connect</h2>
          <p className="pairing-modal__instruction">
            Confirm this code matches what's shown on their screen.
          </p>
          <div className="pairing-modal__code">{incoming.code}</div>
          <div className="pairing-modal__actions">
            <button
              className="btn btn--outline"
              onClick={() => {
                confirmPairingCode(incoming.request_id, false).catch((e) =>
                  pushToast("error", describeError(e)),
                );
                clearIncoming();
              }}
            >
              Reject
            </button>
            <button
              className="btn btn--accent"
              onClick={() => {
                confirmPairingCode(incoming.request_id, true).catch((e) =>
                  pushToast("error", describeError(e)),
                );
                clearIncoming();
              }}
            >
              Codes match — Confirm
            </button>
          </div>
          <button
            className="pairing-modal__block"
            onClick={() => {
              confirmPairingCode(incoming.request_id, false).catch(() => {});
              void blockDevice(incoming.requester_fingerprint, incoming.requester_name).catch((e) =>
                pushToast("error", describeError(e)),
              );
              clearIncoming();
            }}
            title="Reject and never allow this device to request again"
          >
            Block this device
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="modal-backdrop">
      <div className="modal pairing-modal" ref={modalRef} role="dialog" aria-modal="true">
        <p className="pairing-modal__eyebrow">Connecting to {outgoing!.displayName}</p>
        <h2 className="pairing-modal__requester">Waiting for confirmation</h2>
        <p className="pairing-modal__instruction">
          Check that this code matches what appears on the other device.
        </p>
        <div className="pairing-modal__code">{outgoing!.code}</div>
        <div className="pairing-modal__actions">
          <button className="btn btn--outline" onClick={() => setOutgoing(null)}>
            Dismiss
          </button>
        </div>
      </div>
    </div>
  );
}
