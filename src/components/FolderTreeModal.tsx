import { useEffect, useMemo, useRef, useState } from "react";
import { sendFolder, sendFolderMulti } from "../ipc/commands";
import { describeError } from "../lib/errors";
import { formatBytes } from "../lib/format";
import { useAppStore } from "../state/store";
import type { FolderNode } from "../types/tree";


function filesUnder(node: FolderNode): string[] {
  if (!node.is_dir) return [node.rel_path];
  return node.children.flatMap(filesUnder);
}

function TriCheckbox({
  checked,
  indeterminate,
  onChange,
}: {
  checked: boolean;
  indeterminate: boolean;
  onChange: () => void;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = indeterminate;
  }, [indeterminate]);
  return (
    <input
      ref={ref}
      type="checkbox"
      className="tree-row__check"
      checked={checked}
      onChange={onChange}
      onClick={(e) => e.stopPropagation()}
    />
  );
}

function TreeRow({
  node,
  depth,
  selected,
  expanded,
  toggleSelect,
  toggleExpand,
}: {
  node: FolderNode;
  depth: number;
  selected: Set<string>;
  expanded: Set<string>;
  toggleSelect: (node: FolderNode) => void;
  toggleExpand: (relPath: string) => void;
}) {
  const files = filesUnder(node);
  const selCount = files.filter((f) => selected.has(f)).length;
  const checked = files.length > 0 && selCount === files.length;
  const indeterminate = selCount > 0 && selCount < files.length;
  const isOpen = node.is_dir && expanded.has(node.rel_path);

  return (
    <>
      <div className="tree-row" style={{ paddingLeft: 6 + depth * 16 }}>
        {node.is_dir ? (
          <button
            className="tree-row__twist"
            onClick={() => toggleExpand(node.rel_path)}
            aria-label={isOpen ? "Collapse" : "Expand"}
          >
            {isOpen ? "▾" : "▸"}
          </button>
        ) : (
          <span className="tree-row__twist-spacer" />
        )}
        <TriCheckbox checked={checked} indeterminate={indeterminate} onChange={() => toggleSelect(node)} />
        <span
          className={`tree-row__name${node.is_dir ? " tree-row__name--dir" : ""}`}
          onClick={() => (node.is_dir ? toggleExpand(node.rel_path) : toggleSelect(node))}
          title={node.name}
        >
          {node.name}
        </span>
        <span className="tree-row__meta">
          {node.is_dir ? `${files.length} file${files.length === 1 ? "" : "s"}` : formatBytes(node.size)}
        </span>
      </div>
      {isOpen
        ? node.children.map((c) => (
            <TreeRow
              key={c.rel_path}
              node={c}
              depth={depth + 1}
              selected={selected}
              expanded={expanded}
              toggleSelect={toggleSelect}
              toggleExpand={toggleExpand}
            />
          ))
        : null}
    </>
  );
}

export function FolderTreeModal() {
  const pick = useAppStore((s) => s.folderPick);
  const close = useAppStore((s) => s.setFolderPick);
  const pushToast = useAppStore((s) => s.pushToast);

  const allFiles = useMemo(() => (pick ? filesUnder(pick.tree) : []), [pick]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [sending, setSending] = useState(false);


  useEffect(() => {
    setSelected(new Set(allFiles));
    setExpanded(new Set());
  }, [pick?.folderPath, allFiles]);

  if (!pick) return null;

  const toggleSelect = (node: FolderNode) => {
    const files = filesUnder(node);
    setSelected((prev) => {
      const next = new Set(prev);
      const allSel = files.length > 0 && files.every((f) => next.has(f));
      for (const f of files) {
        if (allSel) next.delete(f);
        else next.add(f);
      }
      return next;
    });
  };

  const toggleExpand = (relPath: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(relPath)) next.delete(relPath);
      else next.add(relPath);
      return next;
    });
  };

  const setAllExpanded = (open: boolean) => {
    if (!open) {
      setExpanded(new Set());
      return;
    }
    const dirs = new Set<string>();
    const walk = (n: FolderNode) => {
      if (n.is_dir) {
        dirs.add(n.rel_path);
        n.children.forEach(walk);
      }
    };
    pick.tree.children.forEach(walk);
    setExpanded(dirs);
  };

  const selectedSize = allFiles.length
    ? sumSelectedSize(pick.tree, selected)
    : 0;
  const allSelected = selected.size === allFiles.length && allFiles.length > 0;

  async function send() {
    if (selected.size === 0) return;
    setSending(true);
    try {

      const files = allSelected ? null : Array.from(selected);
      const fileWord = `${selected.size} file${selected.size === 1 ? "" : "s"}`;
      if (pick!.deviceKeys && pick!.deviceKeys.length > 0) {
        const r = await sendFolderMulti(pick!.deviceKeys, pick!.folderPath, files);
        const skip = r.skipped.length ? `, ${r.skipped.length} skipped` : "";
        pushToast("success", `Zipping and sending ${fileWord} to ${r.jobs.length} devices${skip}`);
      } else {
        await sendFolder(pick!.deviceKey, pick!.folderPath, files);
        pushToast("success", `Zipping and sending ${fileWord}`);
      }
      close(null);
    } catch (e) {
      pushToast("error", describeError(e));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={() => (sending ? null : close(null))}>
      <div className="modal foldertree" onClick={(e) => e.stopPropagation()}>
        <p className="pairing-modal__eyebrow">Send folder to {pick.displayName}</p>
        <h2 className="foldertree__title">{pick.tree.name}</h2>

        <div className="foldertree__toolbar">
          <div className="foldertree__toolbar-left">
            <button
              className="btn btn--outline btn--small"
              onClick={() => setSelected(allSelected ? new Set() : new Set(allFiles))}
            >
              {allSelected ? "Deselect all" : "Select all"}
            </button>
            <button className="btn btn--outline btn--small" onClick={() => setAllExpanded(true)}>
              Expand all
            </button>
            <button className="btn btn--outline btn--small" onClick={() => setAllExpanded(false)}>
              Collapse all
            </button>
          </div>
        </div>

        <div className="foldertree__body">
          {pick.tree.children.map((c) => (
            <TreeRow
              key={c.rel_path}
              node={c}
              depth={0}
              selected={selected}
              expanded={expanded}
              toggleSelect={toggleSelect}
              toggleExpand={toggleExpand}
            />
          ))}
        </div>

        <div className="foldertree__summary">
          {selected.size} of {allFiles.length} file{allFiles.length === 1 ? "" : "s"} ·{" "}
          {formatBytes(selectedSize)}
        </div>

        <div className="pairing-modal__actions">
          <button className="btn btn--outline" onClick={() => close(null)} disabled={sending}>
            Cancel
          </button>
          <button
            className="btn btn--accent"
            onClick={() => void send()}
            disabled={sending || selected.size === 0}
          >
            {sending ? "Sending…" : `Send ${selected.size} file${selected.size === 1 ? "" : "s"}`}
          </button>
        </div>
      </div>
    </div>
  );
}


function sumSelectedSize(node: FolderNode, selected: Set<string>): number {
  if (!node.is_dir) return selected.has(node.rel_path) ? node.size : 0;
  return node.children.reduce((s, c) => s + sumSelectedSize(c, selected), 0);
}
