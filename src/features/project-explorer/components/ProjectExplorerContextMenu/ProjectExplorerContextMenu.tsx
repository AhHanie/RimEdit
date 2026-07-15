import { useRef, useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { ContextMenuTarget } from "../FileTreeNode/FileTreeNode";
import styles from "./ProjectExplorerContextMenu.module.css";

interface ProjectExplorerContextMenuProps {
  target: ContextMenuTarget;
  x: number;
  y: number;
  onNewFile: () => void;
  onNewFolder: () => void;
  onNewDefsFile: () => void;
  onNewPatchesFile: () => void;
  onRename: () => void;
  onDelete: () => void;
  onClose: () => void;
}

export function ProjectExplorerContextMenu({
  target,
  x,
  y,
  onNewFile,
  onNewFolder,
  onNewDefsFile,
  onNewPatchesFile,
  onRename,
  onDelete,
  onClose,
}: ProjectExplorerContextMenuProps) {
  const { t } = useTranslation(["shell", "common"]);
  const menuRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  useEffect(() => {
    const el = menuRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const newX = Math.min(x, window.innerWidth - rect.width - 4);
    const newY = Math.min(y, window.innerHeight - rect.height - 4);
    setPos({ x: Math.max(4, newX), y: Math.max(4, newY) });
  }, [x, y]);

  useEffect(() => {
    function handleMouseDown(e: MouseEvent) {
      if (!menuRef.current?.contains(e.target as Node)) onClose();
    }
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    function handleClose() {
      onClose();
    }

    window.addEventListener("mousedown", handleMouseDown);
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("resize", handleClose);
    window.addEventListener("scroll", handleClose, true);

    return () => {
      window.removeEventListener("mousedown", handleMouseDown);
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("resize", handleClose);
      window.removeEventListener("scroll", handleClose, true);
    };
  }, [onClose]);

  const isFolder = target.kind === "folder";
  const showFolderActions = isFolder;
  const showMutateActions = !target.isRoot;

  return (
    <div
      ref={menuRef}
      role="menu"
      className={styles.menu}
      style={{ left: pos.x, top: pos.y }}
      onBlur={(e) => {
        if (!menuRef.current?.contains(e.relatedTarget as Node)) onClose();
      }}
    >
      {showFolderActions && (
        <>
          <button role="menuitem" className={styles.menuItem} onClick={onNewFile}>
            {t("shell:explorer.contextMenu.newFile")}
          </button>
          <button role="menuitem" className={styles.menuItem} onClick={onNewFolder}>
            {t("shell:explorer.contextMenu.newFolder")}
          </button>
          <button role="menuitem" className={styles.menuItem} onClick={onNewDefsFile}>
            {t("shell:explorer.contextMenu.newDefsFile")}
          </button>
          <button role="menuitem" className={styles.menuItem} onClick={onNewPatchesFile}>
            {t("shell:explorer.contextMenu.newPatchesFile")}
          </button>
        </>
      )}
      {showFolderActions && showMutateActions && <div className={styles.separator} />}
      {showMutateActions && (
        <>
          <button role="menuitem" className={styles.menuItem} onClick={onRename}>
            {t("shell:explorer.contextMenu.rename")}
          </button>
          <button
            role="menuitem"
            className={`${styles.menuItem} ${styles.menuItemDestructive}`}
            onClick={onDelete}
          >
            {t("shell:explorer.contextMenu.delete")}
          </button>
        </>
      )}
    </div>
  );
}
