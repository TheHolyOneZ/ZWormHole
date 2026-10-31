import { invoke } from "@tauri-apps/api/core";
import { open, save, type DialogFilter } from "@tauri-apps/plugin-dialog";

export type OsName = "windows" | "macos" | "linux" | "android" | "ios";

function detectOs(): OsName {
  const ua = navigator.userAgent;
  if (/Android/i.test(ua)) return "android";
  if (/iPhone|iPad|iPod/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1)) {
    return "ios";
  }
  if (/Windows/.test(ua)) return "windows";
  if (/Macintosh|Mac OS X/.test(ua)) return "macos";
  return "linux";
}

export const os: OsName = detectOs();
export const isMobile = os === "android" || os === "ios";
export const supportsDragDrop = !isMobile;

export const saveLocationLabel =
  os === "ios"
    ? "Files › On My iPhone › ZWormHole"
    : os === "android"
      ? "Download"
      : "Downloads";

const LONG_PRESS_MS = 500;
const LONG_PRESS_SLOP = 10;

function installLongPressContextMenu() {
  let timer: number | undefined;
  let origin: { x: number; y: number; target: EventTarget | null } | null = null;
  let suppressClickUntil = 0;

  const cancel = () => {
    if (timer !== undefined) window.clearTimeout(timer);
    timer = undefined;
    origin = null;
  };

  document.addEventListener(
    "touchstart",
    (e) => {
      cancel();
      if (e.touches.length !== 1) return;
      const touch = e.touches[0];
      origin = { x: touch.clientX, y: touch.clientY, target: e.target };
      timer = window.setTimeout(() => {
        const at = origin;
        cancel();
        if (!at?.target) return;
        suppressClickUntil = Date.now() + 700;
        at.target.dispatchEvent(
          new MouseEvent("contextmenu", {
            bubbles: true,
            cancelable: true,
            clientX: at.x,
            clientY: at.y,
          }),
        );
      }, LONG_PRESS_MS);
    },
    { passive: true },
  );

  document.addEventListener(
    "touchmove",
    (e) => {
      if (!origin) return;
      const touch = e.touches[0];
      if (
        Math.abs(touch.clientX - origin.x) > LONG_PRESS_SLOP ||
        Math.abs(touch.clientY - origin.y) > LONG_PRESS_SLOP
      ) {
        cancel();
      }
    },
    { passive: true },
  );

  document.addEventListener("touchend", cancel, { passive: true });
  document.addEventListener("touchcancel", cancel, { passive: true });

  document.addEventListener(
    "click",
    (e) => {
      if (Date.now() < suppressClickUntil) {
        suppressClickUntil = 0;
        e.preventDefault();
        e.stopPropagation();
      }
    },
    true,
  );
}

export function applyPlatformAttributes() {
  const root = document.documentElement;
  root.dataset.platform = os;
  root.dataset.mobile = isMobile ? "true" : "false";
  if (os === "ios") installLongPressContextMenu();
}

export async function pickFiles(): Promise<string[]> {
  if (isMobile) return invoke<string[]>("pick_files", { images: false });
  const selected = await open({ multiple: true, directory: false });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function pickFolder(): Promise<string | null> {
  if (isMobile) return invoke<string | null>("pick_folder");
  const selected = await open({ directory: true });
  return typeof selected === "string" ? selected : null;
}

export async function pickSaveDirectory(persist: boolean): Promise<string | null> {
  if (isMobile) return invoke<string | null>("pick_save_dir", { persist });
  const selected = await open({ directory: true });
  return typeof selected === "string" ? selected : null;
}

export async function pickImage(filters: DialogFilter[]): Promise<string | null> {
  if (isMobile) {
    const paths = await invoke<string[]>("pick_files", { images: true });
    return paths[0] ?? null;
  }
  const selected = await open({ multiple: false, directory: false, filters });
  return typeof selected === "string" ? selected : null;
}

export async function exportTextFile(
  fileName: string,
  contents: string,
  filters: DialogFilter[],
  mime: string,
  writeDesktop: (path: string, contents: string) => Promise<void>,
): Promise<boolean> {
  if (isMobile) {
    return invoke<boolean>("export_text_file", { fileName, contents, mime });
  }
  const path = await save({ defaultPath: fileName, filters });
  if (typeof path !== "string") return false;
  await writeDesktop(path, contents);
  return true;
}
