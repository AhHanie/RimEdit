import { useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderOpen, FolderPlus, X } from "lucide-react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { confirmDiscardChanges } from "../../../../lib/confirmDiscardChanges";
import { formatError } from "../../../../lib/formatError";
import { SUPPORTED_LOCALES } from "../../../../i18n/locale";
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
  locale: string;
  onEditLocation: (update: RegisteredLocationUpdate) => Promise<void>;
  onRemoveLocation: (id: string) => Promise<void>;
  onUpdateGameVersion: (version: string) => Promise<void>;
  onChangeLocale: (locale: string) => Promise<void>;
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
  locale,
  onEditLocation,
  onRemoveLocation,
  onUpdateGameVersion,
  onChangeLocale,
  onOpenProject,
  onAddSourceFolder,
}: ProjectSettingsPanelProps) {
  const { t } = useTranslation(["settings", "common"]);
  const [panelError, setPanelError] = useState<string | null>(null);
  const [versionChangePending, setVersionChangePending] = useState(false);
  const [localeChangePending, setLocaleChangePending] = useState(false);

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
      const ok = await confirmDiscardChanges(t("settings:confirm.removeActiveDirty"));
      if (!ok) return;
    } else {
      const ok = await confirm(
        isActive
          ? t("settings:confirm.removeActive")
          : t("settings:confirm.removeNamed", { displayName: location.displayName }),
        {
          title: t("settings:confirm.removeLocationTitle"),
          kind: "warning",
          okLabel: t("common:actions.remove"),
          cancelLabel: t("common:actions.cancel"),
        },
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
      const ok = await confirmDiscardChanges(t("settings:confirm.versionChange"));
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

  async function handleLocaleChange(e: React.ChangeEvent<HTMLSelectElement>) {
    const nextLocale = e.target.value;
    if (!nextLocale || nextLocale === locale) return;

    setPanelError(null);
    setLocaleChangePending(true);
    try {
      // `onChangeLocale` (`LocaleProvider.changeLocale`) itself guarantees that a persistence
      // failure reverts i18next/document/state back to the prior locale before rejecting -- see
      // its doc comment -- so no caller-side rollback is needed here, only surfacing the error.
      await onChangeLocale(nextLocale);
    } catch (err) {
      setPanelError(formatError(err));
    } finally {
      setLocaleChangePending(false);
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
        <span className={styles.title}>{t("settings:panel.title")}</span>
        <button
          className="icon-btn"
          onClick={onOpenProject}
          aria-label={t("settings:panel.openProject")}
          title={t("settings:panel.openProject")}
        >
          <FolderOpen size={14} />
        </button>
        <button
          className="icon-btn"
          onClick={onAddSourceFolder}
          aria-label={t("settings:panel.addSourceFolder")}
          title={t("settings:panel.addSourceFolder")}
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
              aria-label={t("settings:panel.dismissError")}
            >
              <X size={12} />
            </button>
          </div>
        )}
        {loading && (
          <div className="state-loading">
            <p>{t("settings:panel.loading")}</p>
          </div>
        )}
        {!loading && !settings && (
          <div className="state-empty">
            <p className="state-empty-text">
              {loadError ?? t("settings:panel.noSettingsLoaded")}
            </p>
          </div>
        )}
        {!loading && settings && (
          <>
            <div className={styles.sectionHeader}>{t("settings:panel.gameVersionHeader")}</div>
            <div className={styles.gameVersionRow}>
              {installedSchemaVersions.length > 0 ? (
                <select
                  className={styles.gameVersionSelect}
                  value={settings.gameVersion}
                  onChange={(e) => void handleVersionChange(e)}
                  disabled={versionChangePending}
                  aria-label={t("settings:panel.gameVersionAriaLabel")}
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
            <div className={styles.sectionHeader}>{t("settings:panel.languageHeader")}</div>
            <div className={styles.gameVersionRow}>
              <select
                className={styles.gameVersionSelect}
                value={locale}
                onChange={(e) => void handleLocaleChange(e)}
                disabled={localeChangePending}
                aria-label={t("settings:panel.languageAriaLabel")}
              >
                {SUPPORTED_LOCALES.map((l) => (
                  <option key={l.code} value={l.code}>
                    {l.displayName}
                  </option>
                ))}
              </select>
            </div>
          </>
        )}
        {!loading && settings && !hasAnyLocations && (
          <div className="state-empty">
            <FolderOpen size={32} className="state-empty-icon" />
            <p className="state-empty-text">{t("settings:panel.noLocationsRegistered")}</p>
            <button className="btn-primary" onClick={onOpenProject}>
              {t("common:actions.openProject")}
            </button>
            <button className="btn-secondary" onClick={onAddSourceFolder}>
              {t("common:actions.addSourceFolder")}
            </button>
          </div>
        )}
        {!loading && settings && hasAnyLocations && (
          <>
            {activeProject && (
              <>
                <div className={styles.sectionHeader}>{t("settings:panel.activeProjectHeader")}</div>
                {renderRow(activeProject, true)}
              </>
            )}
            {otherProjects.length > 0 && (
              <>
                <div className={styles.sectionHeader}>
                  {activeProject
                    ? t("settings:panel.otherProjectsHeader")
                    : t("settings:panel.projectsHeader")}
                </div>
                {otherProjects.map((loc) => renderRow(loc, false))}
              </>
            )}
            {!activeProject && otherProjects.length === 0 && (
              <>
                <div className={styles.sectionHeader}>{t("settings:panel.projectsHeader")}</div>
                <p className={styles.emptySection}>{t("settings:panel.noProjectsRegistered")}</p>
              </>
            )}
            {sources.length > 0 && (
              <>
                <div className={styles.sectionHeader}>{t("settings:panel.readOnlySourcesHeader")}</div>
                {sources.map((loc) => renderRow(loc, false))}
              </>
            )}
            {sources.length === 0 && (
              <>
                <div className={styles.sectionHeader}>{t("settings:panel.readOnlySourcesHeader")}</div>
                <p className={styles.emptySection}>{t("settings:panel.noSourceFoldersRegistered")}</p>
              </>
            )}
          </>
        )}
      </div>
    </aside>
  );
}
