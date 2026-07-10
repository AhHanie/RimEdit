import { useState } from "react";
import { FolderOpen, FolderPlus, X } from "lucide-react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { confirmDiscardChanges } from "../../../../lib/confirmDiscardChanges";
import { formatError } from "../../../../lib/formatError";
import type { ProjectSettings, RegisteredLocation, RegisteredLocationUpdate } from "../../types";
import { LocationSettingsRow } from "../LocationSettingsRow/LocationSettingsRow";
import styles from "./ProjectSettingsPanel.module.css";

interface ProjectSettingsPanelProps {
  visible: boolean;
  settings: ProjectSettings | null;
  loading: boolean;
  loadError: string | null;
  hasDirtyTabs: boolean;
  installedSchemaVersions: string[];
  onEditLocation: (update: RegisteredLocationUpdate) => Promise<void>;
  onRemoveLocation: (id: string) => Promise<void>;
  onUpdateGameVersion: (version: string) => Promise<void>;
  onOpenProject: () => void;
  onAddSourceFolder: () => void;
}

export function ProjectSettingsPanel({
  visible,
  settings,
  loading,
  loadError,
  hasDirtyTabs,
  installedSchemaVersions,
  onEditLocation,
  onRemoveLocation,
  onUpdateGameVersion,
  onOpenProject,
  onAddSourceFolder,
}: ProjectSettingsPanelProps) {
  const [panelError, setPanelError] = useState<string | null>(null);
  const [versionChangePending, setVersionChangePending] = useState(false);

  async function handleSave(update: RegisteredLocationUpdate) {
    setPanelError(null);
    try {
      await onEditLocation(update);
    } catch (e) {
      setPanelError(formatError(e));
      throw e;
    }
  }

  async function handleRemove(location: RegisteredLocation) {
    setPanelError(null);
    const isActive = location.id === settings?.activeProjectId;

    if (isActive && hasDirtyTabs) {
      const ok = await confirmDiscardChanges(
        "This is the active project. Removing it will close unsaved tabs.",
      );
      if (!ok) return;
    } else {
      const ok = await confirm(
        isActive
          ? "Remove the active project? It will be deactivated."
          : `Remove "${location.displayName}"?`,
        { title: "Remove location", kind: "warning", okLabel: "Remove", cancelLabel: "Cancel" },
      );
      if (!ok) return;
    }

    try {
      await onRemoveLocation(location.id);
    } catch (e) {
      setPanelError(formatError(e));
    }
  }

  async function handleVersionChange(e: React.ChangeEvent<HTMLSelectElement>) {
    const version = e.target.value;
    if (!version || version === settings?.gameVersion) return;

    if (hasDirtyTabs) {
      const ok = await confirmDiscardChanges(
        "Changing the game version will rebuild the Def index and may change form schemas. Existing open files remain open.",
      );
      if (!ok) return;
    }

    setPanelError(null);
    setVersionChangePending(true);
    try {
      await onUpdateGameVersion(version);
    } catch (err) {
      setPanelError(formatError(err));
    } finally {
      setVersionChangePending(false);
    }
  }

  const activeProject =
    settings?.locations.find(
      (l) => l.kind === "project" && l.id === settings.activeProjectId,
    ) ?? null;
  const otherProjects =
    settings?.locations.filter(
      (l) => l.kind === "project" && l.id !== settings?.activeProjectId,
    ) ?? [];
  const sources = settings?.locations.filter((l) => l.kind === "source") ?? [];
  const hasAnyLocations = (settings?.locations.length ?? 0) > 0;

  function renderRow(loc: RegisteredLocation, isActive: boolean) {
    return (
      <LocationSettingsRow
        key={loc.id}
        location={loc}
        isActive={isActive}
        onSave={handleSave}
        onRemove={(_id) => handleRemove(loc)}
      />
    );
  }

  return (
    <aside className={styles.root} data-visible={visible ? "true" : "false"}>
      <div className={styles.header}>
        <span className={styles.title}>Settings</span>
        <button
          className="icon-btn"
          onClick={onOpenProject}
          aria-label="Open project"
          title="Open project"
        >
          <FolderOpen size={14} />
        </button>
        <button
          className="icon-btn"
          onClick={onAddSourceFolder}
          aria-label="Add source folder"
          title="Add source folder"
        >
          <FolderPlus size={14} />
        </button>
      </div>
      <div className={styles.content}>
        {panelError && (
          <div className={styles.errorBanner} role="alert">
            <span className={styles.errorMessage}>{panelError}</span>
            <button
              className="icon-btn"
              onClick={() => setPanelError(null)}
              aria-label="Dismiss error"
            >
              <X size={12} />
            </button>
          </div>
        )}
        {loading && (
          <div className="state-loading">
            <p>Loading…</p>
          </div>
        )}
        {!loading && !settings && (
          <div className="state-empty">
            <p className="state-empty-text">
              {loadError ?? "No settings loaded."}
            </p>
          </div>
        )}
        {!loading && settings && (
          <>
            <div className={styles.sectionHeader}>Game Version</div>
            <div className={styles.gameVersionRow}>
              {installedSchemaVersions.length > 0 ? (
                <select
                  className={styles.gameVersionSelect}
                  value={settings.gameVersion}
                  onChange={(e) => void handleVersionChange(e)}
                  disabled={versionChangePending}
                  aria-label="Game version"
                >
                  {installedSchemaVersions.map((v) => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
              ) : (
                <span className={styles.gameVersionText}>{settings.gameVersion}</span>
              )}
            </div>
          </>
        )}
        {!loading && settings && !hasAnyLocations && (
          <div className="state-empty">
            <FolderOpen size={32} className="state-empty-icon" />
            <p className="state-empty-text">No locations registered.</p>
            <button className="btn-primary" onClick={onOpenProject}>
              Open Project
            </button>
            <button className="btn-secondary" onClick={onAddSourceFolder}>
              Add Source Folder
            </button>
          </div>
        )}
        {!loading && settings && hasAnyLocations && (
          <>
            {activeProject && (
              <>
                <div className={styles.sectionHeader}>Active Project</div>
                {renderRow(activeProject, true)}
              </>
            )}
            {otherProjects.length > 0 && (
              <>
                <div className={styles.sectionHeader}>
                  {activeProject ? "Other Projects" : "Projects"}
                </div>
                {otherProjects.map((loc) => renderRow(loc, false))}
              </>
            )}
            {!activeProject && otherProjects.length === 0 && (
              <>
                <div className={styles.sectionHeader}>Projects</div>
                <p className={styles.emptySection}>No projects registered.</p>
              </>
            )}
            {sources.length > 0 && (
              <>
                <div className={styles.sectionHeader}>Read-only Sources</div>
                {sources.map((loc) => renderRow(loc, false))}
              </>
            )}
            {sources.length === 0 && (
              <>
                <div className={styles.sectionHeader}>Read-only Sources</div>
                <p className={styles.emptySection}>No source folders registered.</p>
              </>
            )}
          </>
        )}
      </div>
    </aside>
  );
}
