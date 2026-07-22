import { useTranslation } from "react-i18next";
import { Files, Search, Settings } from "lucide-react";
import type { ActivityView } from "../types";
import styles from "./ActivityRail.module.css";

interface ActivityRailProps {
  activeView: ActivityView | null;
  onSelectView: (view: ActivityView) => void;
  onOpenPreferences: () => void;
}

export function ActivityRail({ activeView, onSelectView, onOpenPreferences }: ActivityRailProps) {
  const { t } = useTranslation(["shell", "common"]);
  return (
    <nav className={styles.root} aria-label={t("shell:activityRail.ariaLabel")}>
      <button
        className={`icon-btn ${styles.btn}${activeView === "explorer" ? ` ${styles.btnActive}` : ""}`}
        onClick={() => onSelectView("explorer")}
        aria-label={t("shell:activityRail.explorer")}
        title={t("shell:activityRail.explorer")}
        aria-pressed={activeView === "explorer"}
      >
        <Files size={20} />
      </button>
      <button
        className={`icon-btn ${styles.btn}${activeView === "search" ? ` ${styles.btnActive}` : ""}`}
        onClick={() => onSelectView("search")}
        aria-label={t("shell:activityRail.search")}
        title={t("shell:activityRail.search")}
        aria-pressed={activeView === "search"}
      >
        <Search size={20} />
      </button>
      <div className={styles.spacer} />
      <button
        className={`icon-btn ${styles.btn}`}
        onClick={onOpenPreferences}
        aria-label={t("shell:activityRail.preferences")}
        title={t("shell:activityRail.preferences")}
      >
        <Settings size={20} />
      </button>
    </nav>
  );
}
