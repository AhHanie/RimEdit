import { useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { FolderOpen, FolderPlus, X } from "lucide-react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { confirmDiscardChanges } from "../../../../lib/confirmDiscardChanges";
import { formatError } from "../../../../lib/formatError";
import { SUPPORTED_LOCALES } from "../../../../i18n/locale";
import { useDialogKeyboard } from "../../../../lib/useDialogKeyboard";
import type { ThemeMode } from "../../../../types/ui";
import type { ProjectSettings, RegisteredLocation, RegisteredLocationUpdate } from "../../types";
import { LocationSettingsRow } from "../LocationSettingsRow/LocationSettingsRow";
import styles from "./PreferencesDialog.module.css";

type PreferencesCategory = "general" | "rimworld" | "locations";

const CATEGORIES: PreferencesCategory[] = ["general", "rimworld", "locations"];
const THEME_MODES: ThemeMode[] = ["light", "dark", "system"];

interface PreferencesDialogProps {
  onClose: () => void;
  settings: ProjectSettings | null;
  loading: boolean;
  loadError: string | null;
  hasDirtyTabs: boolean;
  installedSchemaVersions: string[];
  locale: string;
  themeMode: ThemeMode;
  onChangeTheme: (mode: ThemeMode) => void;
  onEditLocation: (update: RegisteredLocationUpdate) => Promise<void>;
  onRemoveLocation: (id: string) => Promise<void>;
  onUpdateGameVersion: (version: string) => Promise<void>;
  onChangeLocale: (locale: string) => Promise<void>;
  onOpenProject: () => void;
  onAddSourceFolder: () => void;
}

/**
 * File > Preferences / activity-rail gear modal. Replaces the old resizable Settings sidebar
 * (`ProjectSettingsPanel`) with an accessible dialog (Plan.md "Preferences window implementation
 * plan"). Mounted conditionally by `AppShell` (`{preferencesOpen && <PreferencesDialog .../>}`),
 * matching `AboutDialog`'s convention -- this is what makes `useDialogKeyboard`'s mount-time focus
 * capture/restore and the "always defaults to General" requirement work for free on every open,
 * without a separate `open` prop toggling internal visibility.
 */
export function PreferencesDialog({
  onClose,
  settings,
  loading,
  loadError,
  hasDirtyTabs,
  installedSchemaVersions,
  locale,
  themeMode,
  onChangeTheme,
  onEditLocation,
  onRemoveLocation,
  onUpdateGameVersion,
  onChangeLocale,
  onOpenProject,
  onAddSourceFolder,
}: PreferencesDialogProps) {
  const { t } = useTranslation(["settings", "common"]);
  const containerRef = useRef<HTMLDivElement>(null);
  useDialogKeyboard(containerRef, onClose);

  const [category, setCategory] = useState<PreferencesCategory>("general");
  const [panelError, setPanelError] = useState<string | null>(null);
  const [versionChangePending, setVersionChangePending] = useState(false);
  const [localeChangePending, setLocaleChangePending] = useState(false);

  async function handleSaveLocation(update: RegisteredLocationUpdate) {
    setPanelError(null);
    try {
      await onEditLocation(update);
    } catch (e) {
      setPanelError(formatError(e));
      throw e;
    }
  }

  async function handleRemoveLocation(location: RegisteredLocation) {
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

  function categoryLabel(cat: PreferencesCategory): string {
    return t(`settings:preferences.categories.${cat}`);
  }

  function tabId(cat: PreferencesCategory): string {
    return `preferences-tab-${cat}`;
  }

  function panelId(cat: PreferencesCategory): string {
    return `preferences-panel-${cat}`;
  }

  // Errors are scoped to whichever category's operation raised them (a locale save failure, a
  // location edit failure, ...) -- switching category leaves that operation behind, so carrying
  // the banner over would misleadingly read as belonging to the newly selected category too.
  function selectCategory(next: PreferencesCategory) {
    setCategory(next);
    setPanelError(null);
  }

  function handleTabKeyDown(e: KeyboardEvent<HTMLButtonElement>) {
    const idx = CATEGORIES.indexOf(category);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const nextIdx =
        e.key === "ArrowDown"
          ? (idx + 1) % CATEGORIES.length
          : (idx - 1 + CATEGORIES.length) % CATEGORIES.length;
      const next = CATEGORIES[nextIdx];
      selectCategory(next);
      document.getElementById(tabId(next))?.focus();
    } else if (e.key === "Home") {
      e.preventDefault();
      selectCategory(CATEGORIES[0]);
      document.getElementById(tabId(CATEGORIES[0]))?.focus();
    } else if (e.key === "End") {
      e.preventDefault();
      const last = CATEGORIES[CATEGORIES.length - 1];
      selectCategory(last);
      document.getElementById(tabId(last))?.focus();
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
        onSave={handleSaveLocation}
        onRemove={(_id) => handleRemoveLocation(loc)}
      />
    );
  }

  return (
    <div
      className={styles.overlay}
      role="dialog"
      aria-modal="true"
      aria-label={t("settings:preferences.dialogAriaLabel")}
      onClick={onClose}
    >
      <div className={styles.panel} ref={containerRef} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.title}>{t("settings:preferences.title")}</span>
          <button
            className={styles.closeBtn}
            onClick={onClose}
            aria-label={t("common:actions.close")}
          >
            <X size={14} />
          </button>
        </div>

        <div className={styles.body}>
          <div
            className={styles.categoryNav}
            role="tablist"
            aria-label={t("settings:preferences.categoriesAriaLabel")}
            aria-orientation="vertical"
          >
            {CATEGORIES.map((cat) => (
              <button
                key={cat}
                type="button"
                id={tabId(cat)}
                role="tab"
                aria-selected={category === cat}
                aria-controls={panelId(cat)}
                tabIndex={category === cat ? 0 : -1}
                className={`${styles.categoryBtn}${category === cat ? ` ${styles.categoryBtnActive}` : ""}`}
                onClick={() => selectCategory(cat)}
                onKeyDown={handleTabKeyDown}
              >
                {categoryLabel(cat)}
              </button>
            ))}
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

            {category === "general" && (
              <div
                id={panelId("general")}
                role="tabpanel"
                aria-labelledby={tabId("general")}
                tabIndex={-1}
              >
                <div className={styles.sectionHeader}>{t("settings:preferences.appearanceHeader")}</div>
                <p className={styles.fieldDescription}>
                  {t("settings:preferences.appearanceDescription")}
                </p>
                <div
                  role="radiogroup"
                  aria-label={t("settings:preferences.appearanceHeader")}
                  className={styles.radioGroup}
                >
                  {THEME_MODES.map((m) => (
                    <label key={m} className={styles.radioOption}>
                      <input
                        type="radio"
                        name="preferences-theme-mode"
                        value={m}
                        checked={themeMode === m}
                        onChange={() => onChangeTheme(m)}
                      />
                      <span>{t(`settings:preferences.theme.${m}`)}</span>
                    </label>
                  ))}
                </div>

                <div className={styles.sectionHeader}>{t("settings:preferences.languageHeader")}</div>
                <div className={styles.fieldRow}>
                  <select
                    className={styles.select}
                    value={locale}
                    onChange={(e) => void handleLocaleChange(e)}
                    disabled={localeChangePending}
                    aria-label={t("settings:preferences.languageAriaLabel")}
                  >
                    {SUPPORTED_LOCALES.map((l) => (
                      <option key={l.code} value={l.code}>
                        {l.displayName}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
            )}

            {category === "rimworld" && (
              <div
                id={panelId("rimworld")}
                role="tabpanel"
                aria-labelledby={tabId("rimworld")}
                tabIndex={-1}
              >
                <div className={styles.sectionHeader}>{t("settings:preferences.gameVersionHeader")}</div>
                <div className={styles.fieldRow}>
                  {loading && <p>{t("settings:panel.loading")}</p>}
                  {!loading && !settings && (
                    <p className={styles.fieldDescription}>
                      {loadError ?? t("settings:panel.noSettingsLoaded")}
                    </p>
                  )}
                  {!loading && settings && installedSchemaVersions.length > 0 && (
                    <select
                      className={styles.select}
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
                  )}
                  {!loading && settings && installedSchemaVersions.length === 0 && (
                    <span className={styles.gameVersionText}>{settings.gameVersion}</span>
                  )}
                </div>
              </div>
            )}

            {category === "locations" && (
              <div
                id={panelId("locations")}
                role="tabpanel"
                aria-labelledby={tabId("locations")}
                tabIndex={-1}
              >
                <p className={styles.fieldDescription}>
                  {t("settings:preferences.locationsDescription")}
                </p>
                <div className={styles.locationsHeader}>
                  <span className={styles.sectionHeaderInline}>
                    {t("settings:preferences.categories.locations")}
                  </span>
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
                        <div className={styles.sectionHeader}>
                          {t("settings:panel.activeProjectHeader")}
                        </div>
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
                        <div className={styles.sectionHeader}>
                          {t("settings:panel.readOnlySourcesHeader")}
                        </div>
                        {sources.map((loc) => renderRow(loc, false))}
                      </>
                    )}
                    {sources.length === 0 && (
                      <>
                        <div className={styles.sectionHeader}>
                          {t("settings:panel.readOnlySourcesHeader")}
                        </div>
                        <p className={styles.emptySection}>
                          {t("settings:panel.noSourceFoldersRegistered")}
                        </p>
                      </>
                    )}
                  </>
                )}
              </div>
            )}
          </div>
        </div>

        <div className={styles.footer}>
          <button className={styles.closeFooterBtn} onClick={onClose}>
            {t("common:actions.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
