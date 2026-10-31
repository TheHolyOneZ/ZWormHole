import { useEffect, useRef, useState } from "react";
import { respondToIncomingTransfer } from "../ipc/commands";
import { describeError } from "../lib/errors";
import { formatBytes, formatDeviceName } from "../lib/format";
import { pickSaveDirectory } from "../lib/platform";
import { useFocusTrap } from "../lib/useFocusTrap";
import { useAppStore } from "../state/store";
import type { AutoExtractMode } from "../types/settings";

const FOLDER_OPTIONS: { value: AutoExtractMode; label: string }[] = [
  { value: "off", label: "Leave as .zip" },
  { value: "extract_and_delete", label: "Extract, delete the .zip" },
  { value: "extract_and_keep", label: "Extract, keep the .zip too" },
];


function fileGlyph(name: string): string {
  if (/\(folder\)\s*$/.test(name)) return "\u{1F4C1}";
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  if (["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "heic", "avif"].includes(ext))
    return "\u{1F5BC}\u{FE0F}";
  if (["mp4", "mov", "mkv", "avi", "webm", "m4v"].includes(ext)) return "\u{1F3AC}";
  if (["mp3", "wav", "flac", "ogg", "m4a", "aac"].includes(ext)) return "\u{1F3B5}";
  if (["zip", "rar", "7z", "tar", "gz", "bz2", "xz"].includes(ext)) return "\u{1F5DC}\u{FE0F}";
  if (ext === "pdf") return "\u{1F4D5}";
  if (["doc", "docx", "txt", "md", "rtf", "odt", "pages"].includes(ext)) return "\u{1F4C4}";
  if (
    ["js", "ts", "tsx", "jsx", "rs", "py", "go", "c", "cpp", "h", "java", "json", "html", "css", "sh"].includes(
      ext,
    )
  )
    return "\u{1F4BB}";
  return "\u{1F4E6}";
}


export function IncomingTransferModal() {
  const request = useAppStore((s) => s.incomingTransfer);
  const clear = useAppStore((s) => s.clearIncomingTransfer);
  const pushToast = useAppStore((s) => s.pushToast);
  const trusted = useAppStore((s) => s.trusted);
  const [saveDir, setSaveDir] = useState<string | null>(null);
  const [extractMode, setExtractMode] = useState<AutoExtractMode>("off");
  const modalRef = useRef<HTMLDivElement>(null);
  useFocusTrap(modalRef, !!request);

  useEffect(() => {
    if (request) setExtractMode(request.default_extract_mode);
  }, [request?.request_id]);

  if (!request) return null;

  const effectiveDir = saveDir ?? request.default_save_dir;
  const isFolder = request.folder_archive_name !== null;


  const senderName =
    trusted.find((t) => t.fingerprint === request.device_key)?.display_name ??
    formatDeviceName(request.display_name);

  async function chooseFolder() {
    try {
      const selected = await pickSaveDirectory(false);
      if (selected) setSaveDir(selected);
    } catch (e) {
      pushToast("error", describeError(e));
    }
  }

  async function respond(accept: boolean) {
    try {
      await respondToIncomingTransfer(
        request!.request_id,
        accept,
        accept ? (saveDir ?? undefined) : undefined,
        accept && isFolder ? extractMode : undefined,
      );
    } catch (e) {
      pushToast("error", describeError(e));
    }
    setSaveDir(null);
    clear();
  }

  return (
    <div className="modal-backdrop">
      <div className="modal incoming-modal" ref={modalRef} role="dialog" aria-modal="true">
        <p className="pairing-modal__eyebrow">Incoming transfer</p>
        <h2 className="pairing-modal__requester">{senderName} wants to send you files</h2>

        <ul className="incoming-modal__files">
          {request.files.map((f) => (
            <li key={f.name} className="incoming-modal__file">
              <span className="incoming-modal__file-icon" aria-hidden>
                {fileGlyph(f.name)}
              </span>
              <span className="incoming-modal__file-name">{f.name}</span>
              <span className="incoming-modal__file-size">{formatBytes(f.size)}</span>
            </li>
          ))}
        </ul>

        <div className="incoming-modal__total">
          {request.files.length} file{request.files.length === 1 ? "" : "s"} ·{" "}
          {formatBytes(request.total_size)}
        </div>

        <div className="incoming-modal__dest">
          <span className="incoming-modal__dest-label">Save to</span>
          <button className="btn btn--outline btn--small" onClick={() => void chooseFolder()}>
            {effectiveDir ?? "Choose folder…"}
          </button>
        </div>

        {isFolder ? (
          <div className="incoming-modal__extract">
            <span className="incoming-modal__dest-label">This is a folder — what should happen to it?</span>
            <div className="incoming-modal__extract-options">
              {FOLDER_OPTIONS.map((opt) => (
                <label key={opt.value} className="incoming-modal__extract-option">
                  <input
                    type="radio"
                    name="incoming-extract-mode"
                    checked={extractMode === opt.value}
                    onChange={() => setExtractMode(opt.value)}
                  />
                  {opt.label}
                </label>
              ))}
            </div>
          </div>
        ) : null}

        <div className="pairing-modal__actions">
          <button className="btn btn--outline" onClick={() => void respond(false)}>
            Decline
          </button>
          <button className="btn btn--accent" onClick={() => void respond(true)}>
            Accept
          </button>
        </div>
      </div>
    </div>
  );
}
