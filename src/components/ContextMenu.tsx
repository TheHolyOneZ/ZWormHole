import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";


export function ContextMenu({
  point,
  onClose,
  children,
}: {
  point: { x: number; y: number };
  onClose: () => void;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: point.x, top: point.y, visible: false });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const pad = 8;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const left = Math.max(pad, Math.min(point.x, vw - w - pad));
    let top = point.y + h > vh - pad ? Math.max(pad, point.y - h) : point.y;
    top = Math.max(pad, Math.min(top, vh - h - pad));
    setPos({ left, top, visible: true });
  }, [point.x, point.y]);

  useLayoutEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  return createPortal(
    <>
      <div
        className="side-menu-backdrop"
        onClick={onClose}
        onContextMenu={(e) => {
          e.preventDefault();
          onClose();
        }}
      />
      <div
        ref={ref}
        className="side-menu"
        role="menu"
        style={{ left: pos.left, top: pos.top, visibility: pos.visible ? "visible" : "hidden" }}
        onClick={onClose}
      >
        {children}
      </div>
    </>,
    document.body,
  );
}
