import { useEffect, useRef, useState } from "react";
import { respondToConnectionTest } from "../ipc/commands";
import { formatBytes } from "../lib/format";
import { useFocusTrap } from "../lib/useFocusTrap";
import { useAppStore } from "../state/store";


export function ConnectionTestModal() {
  const request = useAppStore((s) => s.incomingConnectionTest);
  const clear = useAppStore((s) => s.clearIncomingConnectionTest);
  const pushToast = useAppStore((s) => s.pushToast);
  const [remember, setRemember] = useState(false);
  const modalRef = useRef<HTMLDivElement>(null);
  useFocusTrap(modalRef, !!request);

  useEffect(() => {
    setRemember(false);
  }, [request?.request_id]);

  if (!request) return null;

  const isBenchmark = request.kind === "benchmark";
  const label = isBenchmark ? "run a benchmark" : "run a connection test";
  const size = formatBytes(request.total_bytes);

  async function respond(accept: boolean) {
    try {
      await respondToConnectionTest(request!.request_id, accept, accept && remember);
    } catch (e) {
      pushToast("error", String(e));
    }
    clear();
  }

  return (
    <div className="modal-backdrop">
      <div className="modal incoming-modal" ref={modalRef} role="dialog" aria-modal="true">
        <p className="pairing-modal__eyebrow">Connection test</p>
        <h2 className="pairing-modal__requester">{request.display_name} wants to {label}</h2>

        <p className="conntest-modal__detail">
          This sends throwaway data over the encrypted connection to measure speed —{" "}
          <strong>
            {size}
            {isBenchmark ? " each way" : ""}
          </strong>
          . Nothing touches your files, but it uses your bandwidth and CPU while it runs.
        </p>

        <label className="conntest-modal__remember">
          <input
            type="checkbox"
            checked={remember}
            onChange={(e) => setRemember(e.target.checked)}
          />
          Always allow {request.display_name} to run these without asking
        </label>

        <div className="pairing-modal__actions">
          <button className="btn btn--outline" onClick={() => void respond(false)}>
            Decline
          </button>
          <button className="btn btn--accent" onClick={() => void respond(true)}>
            Allow
          </button>
        </div>
      </div>
    </div>
  );
}
