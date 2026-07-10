import { Blocks, RefreshCw, FolderOpen, FolderPlus, PanelLeft, Sun, Moon, Monitor, Command } from "lucide-react";
import type { ThemeMode } from "../../../types/ui";
import styles from "./AppTitleBar.module.css";

interface AppTitleBarProps {
  activeProjectName: string | null;
  activeProjectRoot: string | null;
  themeMode: ThemeMode;
  onCycleTheme: () => void;
  onOpenProject: () => void;
  onAddSourceFolder: () => void;
  onRefresh: () => void;
  onTogglePalette: () => void;
  onToggleExplorer: () => void;
  explorerVisible: boolean;
}

export function AppTitleBar({
  activeProjectName,
  activeProjectRoot,
  themeMode,
  onCycleTheme,
  onOpenProject,
  onAddSourceFolder,
  onRefresh,
  onTogglePalette,
  onToggleExplorer,
  explorerVisible,
}: AppTitleBarProps) {
  const ThemeIcon = themeMode === "light" ? Sun : themeMode === "dark" ? Moon : Monitor;
  const themeLabel =
    themeMode === "light"
      ? "Light theme (click for Dark)"
      : themeMode === "dark"
        ? "Dark theme (click for System)"
        : "System theme (click for Light)";

  return (
    <header className={styles.root}>
      <div className={styles.brand}>
        <Blocks size={16} className={styles.brandIcon} />
        <span>RimEdit</span>
      </div>

      <button
        className={styles.command}
        onClick={onTogglePalette}
        aria-label="Open command palette"
        title="Open command palette (Ctrl+Shift+P)"
      >
        <Command size={12} />
        <span className={styles.commandText}>
          {activeProjectName ? (
            <>
              <span className={styles.commandProject}>{activeProjectName}</span>
              {activeProjectRoot && (
                <span style={{ marginLeft: 6, opacity: 0.55, fontSize: 11 }}>
                  {activeProjectRoot.length > 48
                    ? "…" + activeProjectRoot.slice(-46)
                    : activeProjectRoot}
                </span>
              )}
            </>
          ) : (
            "Open a RimWorld mod folder…"
          )}
        </span>
      </button>

      <div className={styles.actions}>
        <button
          className="icon-btn"
          onClick={onCycleTheme}
          aria-label={themeLabel}
          title={themeLabel}
        >
          <ThemeIcon size={15} />
        </button>
        <button
          className="icon-btn"
          onClick={onRefresh}
          aria-label="Refresh project files"
          title="Refresh project files"
          disabled={!activeProjectName}
        >
          <RefreshCw size={15} />
        </button>
        <button
          className="icon-btn"
          onClick={onOpenProject}
          aria-label="Open project folder"
          title="Open project folder"
        >
          <FolderOpen size={15} />
        </button>
        <button
          className="icon-btn"
          onClick={onAddSourceFolder}
          aria-label="Add source folder"
          title="Add source folder"
        >
          <FolderPlus size={15} />
        </button>
        <button
          className={`icon-btn${explorerVisible ? ` ${styles.btnActive}` : ""}`}
          onClick={onToggleExplorer}
          aria-label="Toggle explorer panel"
          title="Toggle explorer panel"
          aria-pressed={explorerVisible}
        >
          <PanelLeft size={15} />
        </button>
      </div>
    </header>
  );
}
