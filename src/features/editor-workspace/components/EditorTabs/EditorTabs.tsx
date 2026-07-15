import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation("editor");
  if (tabs.length === 0) return null;

  return (
    <div className={styles.root} role="tablist" aria-label={t("workspace.tabs.ariaLabel")}>
      {tabs.map((tab) => {
        const isActive = tab.tabKey === activeTabKey;
        const label = tab.readOnly && tab.locationName
          ? t("workspace.tabs.tabLabelWithLocation", {
              fileName: tab.fileName,
              locationName: tab.locationName,
            })
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
                title={t("workspace.tabs.unsavedChanges")}
                aria-label={t("workspace.tabs.unsavedChanges")}
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
              aria-label={t("workspace.tabs.closeTab", { fileName: tab.fileName })}
              title={t("workspace.tabs.closeTab", { fileName: tab.fileName })}
            >
              <X size={11} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
