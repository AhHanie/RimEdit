import { Files, Search, Settings } from "lucide-react";
import type { ActivityView } from "../types";
import styles from "./ActivityRail.module.css";

interface ActivityRailProps {
  activeView: ActivityView | null;
  onSelectView: (view: ActivityView) => void;
}

export function ActivityRail({ activeView, onSelectView }: ActivityRailProps) {
  return (
    <nav className={styles.root} aria-label="Activity">
      <button
        className={`icon-btn ${styles.btn}${activeView === "explorer" ? ` ${styles.btnActive}` : ""}`}
        onClick={() => onSelectView("explorer")}
        aria-label="Explorer"
        title="Explorer"
        aria-pressed={activeView === "explorer"}
      >
        <Files size={20} />
      </button>
      <button
        className={`icon-btn ${styles.btn}${activeView === "search" ? ` ${styles.btnActive}` : ""}`}
        onClick={() => onSelectView("search")}
        aria-label="Search"
        title="Search"
        aria-pressed={activeView === "search"}
      >
        <Search size={20} />
      </button>
      <div className={styles.spacer} />
      <button
        className={`icon-btn ${styles.btn}${activeView === "settings" ? ` ${styles.btnActive}` : ""}`}
        onClick={() => onSelectView("settings")}
        aria-label="Settings"
        title="Settings"
        aria-pressed={activeView === "settings"}
      >
        <Settings size={20} />
      </button>
    </nav>
  );
}
