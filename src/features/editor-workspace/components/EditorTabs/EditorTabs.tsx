import { X } from "lucide-react";
import type { OpenFileTab } from "../../types";
import styles from "./EditorTabs.module.css";

interface EditorTabsProps {
  tabs: OpenFileTab[];
  activeTabKey: string | null;
  onActivate: (tabKey: string) => void;
  onClose: (tabKey: string) => void;
}

export function EditorTabs({ tabs, activeTabKey, onActivate, onClose }: EditorTabsProps) {
  if (tabs.length === 0) return null;

  return (
    <div className={styles.root} role="tablist" aria-label="Open files">
      {tabs.map((tab) => {
        const isActive = tab.tabKey === activeTabKey;
        const label = tab.readOnly && tab.locationName
          ? `${tab.fileName} · ${tab.locationName}`
          : tab.fileName;
        return (
          <div
            key={tab.tabKey}
            className={`${styles.tab}${isActive ? ` ${styles.tabActive}` : ""}`}
            role="tab"
            aria-selected={isActive}
          >
            <button
              className={styles.tabLabel}
              onClick={() => onActivate(tab.tabKey)}
              title={tab.relativePath}
            >
              {label}
            </button>
            {tab.dirty && (
              <span
                className={styles.dirtyDot}
                title="Unsaved changes"
                aria-label="Unsaved changes"
              />
            )}
            <button
              className={`icon-btn ${styles.tabClose}`}
              onMouseDown={(e) => {
                e.preventDefault();
                e.stopPropagation();
              }}
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                void onClose(tab.tabKey);
              }}
              aria-label={`Close ${tab.fileName}`}
              title={`Close ${tab.fileName}`}
            >
              <X size={11} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
