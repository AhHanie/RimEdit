import { useTranslation } from "react-i18next";
import { RefreshCw, FolderOpen, FolderPlus, PanelLeft, Sun, Moon, Monitor, Command } from "lucide-react";
import type { ThemeMode } from "../../../types/ui";
import type { CommandAction, MenuDescriptor } from "../../commands/commandTypes";
import { AppMenuBar } from "../AppMenuBar/AppMenuBar";
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
  commands: CommandAction[];
  menus: MenuDescriptor[];
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
  commands,
  menus,
}: AppTitleBarProps) {
  const { t } = useTranslation(["shell", "common"]);
  const ThemeIcon = themeMode === "light" ? Sun : themeMode === "dark" ? Moon : Monitor;
  const themeLabel =
    themeMode === "light"
      ? t("shell:titleBar.themeLight")
      : themeMode === "dark"
        ? t("shell:titleBar.themeDark")
        : t("shell:titleBar.themeSystem");

  return (
    <header className={styles.root}>
      <AppMenuBar commands={commands} menus={menus} />

      <button
        className={styles.command}
        onClick={onTogglePalette}
        aria-label={t("shell:titleBar.commandPaletteAriaLabel")}
        title={t("shell:titleBar.openCommandPalette")}
      >
        <Command size={12} />
        <span className={styles.commandText}>
          {activeProjectName ? (
            <>
              <span className={styles.commandProject}>{activeProjectName}</span>
              {activeProjectRoot && (
                <span style={{ marginInlineStart: 6, opacity: 0.55, fontSize: 11 }}>
                  {activeProjectRoot.length > 48
                    ? "…" + activeProjectRoot.slice(-46)
                    : activeProjectRoot}
                </span>
              )}
            </>
          ) : (
            t("shell:titleBar.noProjectPrompt")
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
          aria-label={t("shell:titleBar.refreshProjectFiles")}
          title={t("shell:titleBar.refreshProjectFiles")}
          disabled={!activeProjectName}
        >
          <RefreshCw size={15} />
        </button>
        <button
          className="icon-btn"
          onClick={onOpenProject}
          aria-label={t("shell:titleBar.openProjectFolder")}
          title={t("shell:titleBar.openProjectFolder")}
        >
          <FolderOpen size={15} />
        </button>
        <button
          className="icon-btn"
          onClick={onAddSourceFolder}
          aria-label={t("shell:titleBar.addSourceFolder")}
          title={t("shell:titleBar.addSourceFolder")}
        >
          <FolderPlus size={15} />
        </button>
        <button
          className={`icon-btn${explorerVisible ? ` ${styles.btnActive}` : ""}`}
          onClick={onToggleExplorer}
          aria-label={t("shell:titleBar.toggleExplorer")}
          title={t("shell:titleBar.toggleExplorer")}
          aria-pressed={explorerVisible}
        >
          <PanelLeft size={15} />
        </button>
      </div>
    </header>
  );
}
