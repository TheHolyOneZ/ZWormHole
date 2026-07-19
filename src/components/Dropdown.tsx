import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

export interface DropdownOption {
  value: string;
  label: string;
}

interface DropdownProps {
  options: DropdownOption[];
  value: string;
  onChange: (value: string) => void;
  className?: string;
  "aria-label"?: string;
}


export function Dropdown({ options, value, onChange, className, ...aria }: DropdownProps) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const [pos, setPos] = useState({ left: 0, top: 0, width: 0, visible: false });
  const selected = options.find((o) => o.value === value) ?? options[0];


  useLayoutEffect(() => {
    if (!open) {
      setPos((p) => (p.visible ? { ...p, visible: false } : p));
      return;
    }
    const t = triggerRef.current;
    const list = listRef.current;
    if (!t || !list) return;
    const r = t.getBoundingClientRect();
    const h = list.offsetHeight;
    const pad = 8;
    const below = r.bottom + 6;
    const top = below + h > window.innerHeight - pad ? Math.max(pad, r.top - h - 6) : below;
    setPos({ left: r.left, top, width: r.width, visible: true });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    function handlePointerDown(e: MouseEvent) {
      const t = e.target as Node;
      if (triggerRef.current?.contains(t)) return;
      if (listRef.current?.contains(t)) return;
      setOpen(false);
    }
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKey);
    };
  }, [open]);

  return (
    <div className={`dropdown${className ? ` ${className}` : ""}`}>
      <button
        ref={triggerRef}
        type="button"
        className="dropdown__trigger"
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={aria["aria-label"]}
      >
        <span>{selected?.label ?? ""}</span>
        <svg className="dropdown__chevron" width="10" height="6" viewBox="0 0 10 6" aria-hidden>
          <path
            d="M1 1l4 4 4-4"
            stroke="currentColor"
            strokeWidth="1.5"
            fill="none"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>
      {open
        ? createPortal(
            <ul
              ref={listRef}
              className="dropdown__list"
              role="listbox"
              style={{
                left: pos.left,
                top: pos.top,
                minWidth: pos.width,
                visibility: pos.visible ? "visible" : "hidden",
              }}
            >
              {options.map((o) => (
                <li key={o.value}>
                  <button
                    type="button"
                    className={`dropdown__option${
                      o.value === value ? " dropdown__option--selected" : ""
                    }`}
                    role="option"
                    aria-selected={o.value === value}
                    onClick={() => {
                      onChange(o.value);
                      setOpen(false);
                    }}
                  >
                    {o.label}
                  </button>
                </li>
              ))}
            </ul>,
            document.body,
          )
        : null}
    </div>
  );
}
