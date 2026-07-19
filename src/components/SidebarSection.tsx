import { useState, type ReactNode } from "react";


export function SidebarSection({
  title,
  count,
  defaultOpen = true,
  action,
  children,
}: {
  title: string;
  count?: number;
  defaultOpen?: boolean;
  action?: ReactNode;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <section className="side-sec">
      <div className="side-sec__head">
        <button
          className="side-sec__toggle"
          onClick={() => setOpen((o) => !o)}
          aria-expanded={open}
        >
          <span className={`side-sec__chev${open ? " side-sec__chev--open" : ""}`} aria-hidden>
            ›
          </span>
          <span className="side-sec__title">{title}</span>
          {count !== undefined ? <span className="side-sec__count">{count}</span> : null}
        </button>
        {action ? <div className="side-sec__action">{action}</div> : null}
      </div>
      {open ? <div className="side-sec__body">{children}</div> : null}
    </section>
  );
}
