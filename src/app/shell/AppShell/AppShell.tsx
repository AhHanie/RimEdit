import {
  useState,
  useEffect,
  useRef,
  useMemo,
  useCallback,
  type CSSProperties,
} from "react";
import { useTranslation } from "react-i18next";
import {
  FolderOpen,
  FolderPlus,
  FilePlus,
  RefreshCw,
  PanelLeft,
  Search,
  Settings,
  Command,
  Info,
  X,
} from "lucide-react";
import { openAppDataFolder } from "../../commands/appDataCommands";
import {
  pickProjectFolder,
  pickSourceFolder,
  useProjectSettings,
  PreferencesDialog,
  type ProjectSettingsLoadResult,
} from "../../../features/project-settings";
import {
  EditorWorkspace,
  useEditorWorkspace,
} from "../../../features/editor-workspace";
import type {
  ActiveEditorCommands,
  OpenFileRef,
} from "../../../features/editor-workspace";
import { useSchemaCatalog } from "../../../features/schema-catalog";
import { useTheme } from "../../../hooks/useTheme";
import { useLocale } from "../../../i18n/LocaleProvider";
import { confirmDiscardChanges } from "../../../lib/confirmDiscardChanges";
import {
  buildFileTree,
  filterFiles,
  collectFolderIds,
  ProjectExplorerPanel,
} from "../../../features/project-explorer";
import type { ProjectFileEntry } from "../../../features/project-explorer";
import { DefSearchPanel, useIndexingStatus } from "../../../features/def-index";
import type { IndexingStatus } from "../../../features/def-index";
import type { ActivityView } from "../types";
import type { CommandAction, MenuDescriptor } from "../../commands/commandTypes";
import { AppTitleBar } from "../AppTitleBar/AppTitleBar";
import { ActivityRail } from "../ActivityRail/ActivityRail";
import { StatusBar } from "../StatusBar/StatusBar";
import { CommandPalette } from "../../commands/CommandPalette/CommandPalette";
import { AboutDialog } from "../AboutDialog/AboutDialog";
import { ResizablePaneHandle } from "../ResizablePaneHandle/ResizablePaneHandle";
import { usePersistentLayoutState } from "../layout/usePersistentLayoutState";
import { LAYOUT_DEFAULTS } from "../layout/layoutState";
import styles from "./AppShell.module.css";

/** Pure decision for the `validationRefreshRevision` effect below, extracted so it's directly
 * unit-testable without rendering `AppShell`. Distinct from `indexRevision`'s own condition: this
 * only bumps after the index itself actually changes, never merely because a trusted hydrated
 * cache becomes verified in the background (checking -> verified leaves the same `DefIndex` in
 * place, untouched -- see `CacheVerification`).
 *
 * Compares `status.indexBuiltAtUnixMs` (the completed index's own build timestamp) against the
 * value observed at the last "complete" status for the *same* active project (see the effect
 * below, which resets `prevIndexBuiltAtUnixMs` to `null` whenever `activeProjectId` changes),
 * rather than diffing phase transitions or def/error counts: the backend emits a separate status
 * event per phase transition, but nothing guarantees the frontend observes each one as its own
 * render -- a very fast rebuild's pending -> running -> complete events could in principle land
 * close enough together to be batched into one commit, in which case a phase-transition check
 * would miss it entirely. Comparing counts instead of a build timestamp has the same gap in the
 * opposite direction: a genuine rebuild whose def/error counts happen to come out identical to the
 * prior snapshot (e.g. one Def edited, none added/removed) would be indistinguishable from a
 * verification-only confirmation. `indexBuiltAtUnixMs` only ever changes when the index was
 * actually (re)built (a fresh full rebuild or an applied incremental update), never on a mere
 * verification flag flip, so comparing it catches every real change and only real changes,
 * regardless of how the intermediate phase events were batched.
 *
 * `prevIndexBuiltAtUnixMs === null` bumps unconditionally (rather than treating "no prior
 * snapshot" as "nothing to compare, don't bump"): a document can already be open -- and validated
 * against an empty/partial `IndexLoadPolicy::Interactive` fallback index -- before this component
 * has ever observed a "complete" status at all (e.g. the window became interactive before the
 * very first hydration/rebuild finished), so the *first* observed completion still needs to
 * trigger a catch-up revalidation for any tab already open at that point. If nothing is open yet,
 * bumping the revision is simply inert.
 *
 * `status.projectId !== activeProjectId` also refuses to act, independent of the ref-reset the
 * calling effect does on a project switch: `useIndexingStatus` clears its own status to `null` on
 * an active-project change, but that clear only takes effect on the *next* render -- the render
 * where `activeProjectId` first changes still has the *previous* project's `indexingStatus` object
 * in scope. Checking `projectId` directly here closes that gap regardless of render/effect
 * ordering, rather than relying on the reset happening to land before this function is next
 * evaluated.
 */
export function shouldBumpValidationRefreshRevision(
  status: IndexingStatus | null | undefined,
  prevIndexBuiltAtUnixMs: number | null,
  activeProjectId: string | undefined,
): boolean {
  if (status?.phase !== "complete") return false;
  if (status.indexBuiltAtUnixMs === undefined) return false;
  if (status.projectId !== activeProjectId) return false;
  return prevIndexBuiltAtUnixMs !== status.indexBuiltAtUnixMs;
}

export interface AppShellProps {
  /** Forwarded to `useProjectSettings` -- see that hook's doc comment. Set by `main.tsx`'s
   * pre-`LocaleProvider` bootstrap fetch so the initial locale sync effect below never has
   * anything to reconcile (`settings.locale` and the provider's `initialLocale` come from the
   * same resolved settings) and so `get_project_settings` is only ever called once at startup. */
  initialProjectSettingsPromise?: Promise<ProjectSettingsLoadResult>;
}

export function AppShell({ initialProjectSettingsPromise }: AppShellProps = {}) {
  const { t } = useTranslation(["shell", "common"]);
  const {
    settings,
    loading,
    error: settingsLoadError,
    startupNotice,
    clearStartupNotice,
    activateProject,
    replaceSettings,
    deleteLocation,
    editLocation,
    updateGameVersion,
    installedSchemaVersions,
  } = useProjectSettings(initialProjectSettingsPromise);
  const activeProjectId = settings?.activeProjectId;
  const activeLocation = settings?.locations.find(
    (l) => l.id === activeProjectId,
  );

  const workspace = useEditorWorkspace(activeProjectId);
  // Catalog context must match the same "every registered location root" policy the backend
  // uses for document validation and patch preview (see `services::validation::schema_pack_roots`
  // and `services::patch_preview::preview_def_for_project`) -- there is no separate "configured
  // external schema roots" setting anywhere in `ProjectSettings`/`RegisteredLocation` today, so
  // this reuses the same project-location data rather than inventing a second registry.
  // `schema_pack::loader` searches each root, its `About/`, and its `SchemaPacks/<name>/` for an
  // embedded schema pack.
  // Memoized on `settings?.locations`'s own reference (not recomputed on every unrelated AppShell
  // re-render) so it doesn't churn `useSchemaCatalog`'s reload effect.
  const extraSchemaRoots = useMemo(
    () => settings?.locations.map((l) => l.rootPath) ?? [],
    [settings?.locations],
  );
  const { mode: themeMode, setMode } = useTheme();
  const { locale, changeLocale } = useLocale();
  // Locale is threaded through so catalog labels/descriptions reload for the active locale;
  // `useSchemaCatalog` discards any in-flight response superseded by a newer switch.
  const { catalog } = useSchemaCatalog(extraSchemaRoots, settings?.gameVersion, locale);

  // Defensive fallback only: `main.tsx` already resolves the persisted locale from the very same
  // `get_project_settings` call (via `initialProjectSettingsPromise` above) and passes it as
  // `LocaleProvider`'s `initialLocale` *before* this component -- and its locale-sensitive
  // `useSchemaCatalog` call above -- ever mounts, so `settings.locale` and `locale` already agree
  // by the time this effect's condition is evaluated in the normal startup path: the settings
  // command returns the saved locale before locale-sensitive catalog loading. This
  // only does anything when a caller renders `AppShell` without going through that bootstrap
  // (e.g. a future test harness that mounts it under a plain `LocaleProvider` default), in which
  // case it still reconciles the provider to the loaded settings' locale rather than leaving it
  // English forever. Only runs once per app lifetime (guarded, like `useProjectSettings`' own
  // load effect, against React StrictMode's double-invoked effects).
  const hasAppliedPersistedLocaleRef = useRef(false);
  useEffect(() => {
    if (hasAppliedPersistedLocaleRef.current) return;
    if (!settings) return;
    hasAppliedPersistedLocaleRef.current = true;
    if (settings.locale !== locale) {
      // `changeLocale` also persists the (unchanged) resolved value back through
      // `persistLocale` -- normally a no-op round-trip, but its rejection must not vanish
      // silently, so surface it the same way every other best-effort startup failure in this
      // component is surfaced (see `handleAddSourceFolder`/`handleOpenProject` above).
      changeLocale(settings.locale).catch((e: unknown) => {
        console.error("Failed to apply persisted locale on startup:", e);
      });
    }
  }, [settings, locale, changeLocale]);

  const [activeView, setActiveView] = useState<ActivityView | null>("explorer");
  const [fileFilterQuery, setFileFilterQuery] = useState("");
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(
    new Set(),
  );
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [preferencesOpen, setPreferencesOpen] = useState(false);
  // Non-blocking reminder shown after adding a source folder that looked like it could be a
  // Steam Workshop collection root but wasn't confidently detected as one (see
  // `pickSourceFolder`'s `ambiguousWorkshopRoot`) -- holds the new location's display name, or
  // `null` when dismissed/not applicable.
  const [workshopRootNotice, setWorkshopRootNotice] = useState<string | null>(null);
  const [createDefSignal, setCreateDefSignal] = useState(0);
  const activeEditorCommandsRef = useRef<ActiveEditorCommands | null>(null);
  const handleActiveCommandsChange = useCallback(
    (commands: ActiveEditorCommands | null) => {
      activeEditorCommandsRef.current = commands;
    },
    [],
  );
  const fileSearchInputRef = useRef<HTMLInputElement>(null);
  const defSearchInputRef = useRef<HTMLInputElement>(null);
  const workspaceRef = useRef<HTMLDivElement>(null);
  const {
    explorerWidth,
    minExplorerWidth,
    maxExplorerWidth,
    setExplorerWidth,
  } = usePersistentLayoutState(workspaceRef);

  const explorerVisible = activeView === "explorer";
  const searchPanelVisible = activeView === "search";

  const indexingStatus = useIndexingStatus(activeProjectId);
  const [indexRevision, setIndexRevision] = useState(0);
  const prevIndexPhaseRef = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (
      indexingStatus?.phase === "complete" &&
      prevIndexPhaseRef.current !== "complete"
    ) {
      setIndexRevision((r) => r + 1);
    }
    prevIndexPhaseRef.current = indexingStatus?.phase;
  }, [indexingStatus?.phase]);

  // Distinct from `indexRevision` above: this only bumps after a *rebuild* completes (a
  // pending/running -> complete transition), never merely because a trusted hydrated cache
  // becomes verified in the background (that transition never leaves `phase: "complete"`, so it
  // can't trigger this effect at all -- see `CacheVerification`). Threaded down to open XML
  // editor sessions so they can re-run validation against the rebuilt index without discarding
  // in-progress edits/undo history/dirty state -- see `useXmlEditorSession`'s revalidation effect.
  const [validationRefreshRevision, setValidationRefreshRevision] = useState(0);
  // Tracks the completed index's own build timestamp as of the last observed "complete" status --
  // see `shouldBumpValidationRefreshRevision`. Reset to `null` on every active-project change (see
  // `prevValidationProjectIdRef` below): project A's and project B's `indexBuiltAtUnixMs` values
  // are unrelated timestamps, so comparing across the switch would just be comparing arbitrary
  // numbers rather than detecting a real rebuild -- resetting establishes a fresh baseline for
  // whichever project just became active instead.
  const prevIndexBuiltAtRef = useRef<number | null>(null);
  const prevValidationProjectIdRef = useRef<string | undefined>(activeProjectId);
  useEffect(() => {
    if (prevValidationProjectIdRef.current !== activeProjectId) {
      prevValidationProjectIdRef.current = activeProjectId;
      prevIndexBuiltAtRef.current = null;
    }
    if (
      shouldBumpValidationRefreshRevision(
        indexingStatus,
        prevIndexBuiltAtRef.current,
        activeProjectId,
      )
    ) {
      setValidationRefreshRevision((r) => r + 1);
    }
    // Guarded by the same `projectId` check as `shouldBumpValidationRefreshRevision`: a status
    // object left over from the *previous* active project (possible on the very render
    // `activeProjectId` changes, before `useIndexingStatus`'s own reset-to-null takes effect on
    // the next render) must never seed the baseline for whichever project is actually current --
    // otherwise a later, real status for the new project would be compared against another
    // project's unrelated timestamp.
    if (
      indexingStatus?.phase === "complete" &&
      indexingStatus.indexBuiltAtUnixMs !== undefined &&
      indexingStatus.projectId === activeProjectId
    ) {
      prevIndexBuiltAtRef.current = indexingStatus.indexBuiltAtUnixMs;
    }
  }, [
    activeProjectId,
    indexingStatus?.phase,
    indexingStatus?.indexBuiltAtUnixMs,
    indexingStatus?.projectId,
  ]);

  const fileTree = useMemo(
    () =>
      workspace.scan
        ? buildFileTree(
            workspace.scan,
            activeLocation?.displayName ?? t("shell:explorer.projectFallbackName"),
            locale,
          )
        : null,
    [workspace.scan, activeLocation?.displayName, t, locale],
  );

  const filteredFiles = useMemo(
    () =>
      workspace.scan ? filterFiles(workspace.scan.files, fileFilterQuery) : [],
    [workspace.scan, fileFilterQuery],
  );

  const activeTab = useMemo(
    () =>
      workspace.tabs.find((tab) => tab.tabKey === workspace.activeTabKey) ?? null,
    [workspace.tabs, workspace.activeTabKey],
  );
  // Only highlight project files in the explorer and status bar; source tabs
  // have no corresponding entry in the project scan.
  const activeProjectTab =
    activeTab?.sourceKind === "project" ? activeTab : null;
  const activeFilePath = activeProjectTab?.relativePath ?? null;
  const activeFileEntry = useMemo(
    () =>
      activeFilePath && workspace.scan
        ? (workspace.scan.files.find(
            (f) => f.relativePath === activeFilePath,
          ) ?? null)
        : null,
    [activeFilePath, workspace.scan],
  );
  const hasDirtyTabs = useMemo(
    () => workspace.tabs.some((tab) => tab.dirty),
    [workspace.tabs],
  );

  // Refresh the project file scan when the game version changes so that
  // active/inactive file styling updates immediately without a manual refresh.
  const prevGameVersionRef = useRef<string | undefined>(undefined);
  useEffect(() => {
    const ver = settings?.gameVersion;
    if (
      ver !== undefined &&
      ver !== prevGameVersionRef.current &&
      prevGameVersionRef.current !== undefined
    ) {
      void workspace.refresh();
    }
    prevGameVersionRef.current = ver;
  }, [settings?.gameVersion, workspace.refresh]);

  // Initialize expansion state when the active project changes.
  const prevProjectIdRef = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (activeProjectId !== prevProjectIdRef.current) {
      prevProjectIdRef.current = activeProjectId;
      setExpandedFolders(activeProjectId ? new Set([""]) : new Set());
    }
  }, [activeProjectId]);

  // After each scan, prune expansion ids that no longer exist in the tree.
  useEffect(() => {
    if (!fileTree) return;
    const validIds = collectFolderIds(fileTree);
    setExpandedFolders((prev) => {
      const next = new Set([...prev].filter((id) => validIds.has(id)));
      next.add(""); // always keep root expanded
      return next;
    });
  }, [fileTree]);

  const handleAddSourceFolder = useCallback(async () => {
    try {
      const result = await pickSourceFolder(settings);
      if (!result) return;
      replaceSettings(result.settings);
      if (result.ambiguousWorkshopRoot) {
        setWorkshopRootNotice(
          result.settings.locations.find((l) => l.id === result.locationId)?.displayName ?? null,
        );
      }
    } catch (e: unknown) {
      console.error("Failed to add source folder:", e);
    }
  }, [replaceSettings, settings]);

  const handleOpenProject = useCallback(async () => {
    try {
      if (
        hasDirtyTabs &&
        !(await confirmDiscardChanges(t("shell:confirm.openAnotherProject")))
      ) {
        return;
      }
      const result = await pickProjectFolder();
      if (!result) return;
      await activateProject(result.locationId);
    } catch (e: unknown) {
      console.error("Failed to open project:", e);
    }
  }, [activateProject, hasDirtyTabs, t]);

  const handleOpenAppDataFolder = useCallback(async () => {
    try {
      await openAppDataFolder();
    } catch (e: unknown) {
      console.error("Failed to open RimEdit data folder:", e);
    }
  }, []);

  function fileEntryToOpenFileRef(file: ProjectFileEntry): OpenFileRef {
    return {
      locationId: file.locationId ?? activeProjectId!,
      locationName: file.locationName ?? activeLocation?.displayName,
      sourceKind: file.sourceKind ?? "project",
      readOnly: file.readOnly ?? false,
      relativePath: file.relativePath,
      // The project scan refresh triggered by creating this file is
      // asynchronous, so it may not have landed yet when we open the tab --
      // use the fileKind from the just-created entry instead of relying on
      // openTab's scan lookup finding it.
      editorKindHint: file.fileKind === "xml" ? "xml" : undefined,
    };
  }

  function projectPathToOpenFileRef(relativePath: string): OpenFileRef {
    return {
      locationId: activeProjectId!,
      locationName: activeLocation?.displayName,
      sourceKind: "project",
      readOnly: false,
      relativePath,
    };
  }

  function handleSelectView(view: ActivityView) {
    setActiveView((prev) => (prev === view ? null : view));
    if (view === "search") {
      setTimeout(() => defSearchInputRef.current?.focus(), 0);
    }
    if (view === "explorer") {
      setTimeout(() => fileSearchInputRef.current?.focus(), 0);
    }
  }

  function handleToggleFolder(id: string) {
    setExpandedFolders((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  // Global keyboard shortcuts
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Preferences is a modal dialog with its own Escape/Tab handling (`useDialogKeyboard`) --
      // none of these background-workspace shortcuts (command palette, Def Search focus, editor
      // undo/redo/save/close) should fire while it's open, or they'd act on the workspace behind
      // the dialog instead of leaving it focus-isolated.
      if (preferencesOpen) return;
      if (e.ctrlKey && e.shiftKey && e.key === "P") {
        e.preventDefault();
        setPaletteOpen((open) => !open);
        return;
      }
      if (e.ctrlKey && !e.shiftKey && e.key === "p") {
        e.preventDefault();
        setActiveView("search");
        setTimeout(() => defSearchInputRef.current?.focus(), 0);
        return;
      }
      if (e.key === "Escape") {
        if (paletteOpen) {
          setPaletteOpen(false);
          return;
        }
        if (fileFilterQuery) {
          setFileFilterQuery("");
        }
        return;
      }

      // Editor shortcuts - skip if the palette is open, Alt is held, or the
      // raw editor already handled the event (e.defaultPrevented is true when
      // CodeMirror's high-precedence keymap returned true for the same key).
      const isCtrlOrMeta = e.ctrlKey || e.metaKey;
      if (isCtrlOrMeta && !e.altKey && !paletteOpen && !e.defaultPrevented) {
        const cmd = activeEditorCommandsRef.current;
        if (cmd) {
          const key = e.key.toLowerCase();
          // Always preventDefault for recognized editor shortcuts when an active
          // editor handle exists - this prevents form inputs from falling back to
          // native text-undo/redo even when the app command is currently disabled.
          if (key === "z" && !e.shiftKey) {
            e.preventDefault();
            if (cmd.canUndo) cmd.undo();
            return;
          }
          if (key === "z" && e.shiftKey) {
            e.preventDefault();
            if (cmd.canRedo) cmd.redo();
            return;
          }
          if (key === "y") {
            e.preventDefault();
            if (cmd.canRedo) cmd.redo();
            return;
          }
          if (key === "s") {
            e.preventDefault();
            if (cmd.canSave) void cmd.save();
            return;
          }
          if (key === "w") {
            e.preventDefault();
            if (cmd.canClose) void cmd.close();
            return;
          }
        }
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [paletteOpen, fileFilterQuery, preferencesOpen]);

  const commands = useMemo<CommandAction[]>(
    () => [
      {
        id: "open-project",
        labelKey: "shell:commands.openProject.label",
        keywordsKey: "shell:commands.openProject.keywords",
        icon: FolderOpen,
        run: handleOpenProject,
      },
      {
        id: "open-command-palette",
        labelKey: "shell:commands.openCommandPalette.label",
        keywordsKey: "shell:commands.openCommandPalette.keywords",
        icon: Command,
        run: () => setPaletteOpen(true),
      },
      {
        id: "show-about",
        labelKey: "shell:commands.showAbout.label",
        keywordsKey: "shell:commands.showAbout.keywords",
        icon: Info,
        run: () => setAboutOpen(true),
      },
      {
        id: "add-source-folder",
        labelKey: "shell:commands.addSourceFolder.label",
        keywordsKey: "shell:commands.addSourceFolder.keywords",
        icon: FolderPlus,
        run: handleAddSourceFolder,
      },
      {
        id: "open-app-data-folder",
        labelKey: "shell:commands.openAppDataFolder.label",
        keywordsKey: "shell:commands.openAppDataFolder.keywords",
        icon: FolderOpen,
        run: handleOpenAppDataFolder,
      },
      {
        id: "refresh",
        labelKey: "shell:commands.refresh.label",
        keywordsKey: "shell:commands.refresh.keywords",
        icon: RefreshCw,
        run: workspace.refresh,
        disabled: !activeProjectId,
      },
      {
        id: "open-settings",
        labelKey: "shell:commands.openSettings.label",
        keywordsKey: "shell:commands.openSettings.keywords",
        icon: Settings,
        run: () => setPreferencesOpen(true),
      },
      {
        id: "toggle-explorer",
        labelKey: "shell:commands.toggleExplorer.label",
        keywordsKey: "shell:commands.toggleExplorer.keywords",
        icon: PanelLeft,
        run: () => setActiveView((v) => (v === "explorer" ? null : "explorer")),
      },
      {
        id: "focus-search",
        labelKey: "shell:commands.focusSearch.label",
        keywordsKey: "shell:commands.focusSearch.keywords",
        icon: Search,
        run: () => {
          setActiveView("search");
          setTimeout(() => defSearchInputRef.current?.focus(), 0);
        },
      },
      {
        id: "create-def",
        labelKey: "shell:commands.createDef.label",
        keywordsKey: "shell:commands.createDef.keywords",
        icon: FilePlus,
        run: () => setCreateDefSignal((s) => s + 1),
        disabled: !activeProjectId || !activeTab || activeTab.readOnly,
      },
    ],
    [
      handleOpenProject,
      handleAddSourceFolder,
      handleOpenAppDataFolder,
      workspace.refresh,
      activeProjectId,
      activeTab,
      setActiveView,
      setPaletteOpen,
      setAboutOpen,
      setPreferencesOpen,
    ],
  );

  const menus = useMemo<MenuDescriptor[]>(
    () => [
      {
        id: "file",
        labelKey: "shell:menuBar.file",
        entries: [
          { kind: "command", commandId: "open-project" },
          { kind: "command", commandId: "add-source-folder" },
          { kind: "command", commandId: "open-app-data-folder" },
          { kind: "separator" },
          { kind: "command", commandId: "open-settings" },
          { kind: "separator" },
          { kind: "command", commandId: "refresh" },
        ],
      },
      {
        id: "view",
        labelKey: "shell:menuBar.view",
        entries: [
          { kind: "command", commandId: "open-command-palette" },
          { kind: "command", commandId: "focus-search" },
          { kind: "separator" },
          { kind: "command", commandId: "toggle-explorer", checked: explorerVisible },
        ],
      },
      {
        id: "help",
        labelKey: "shell:menuBar.help",
        entries: [{ kind: "command", commandId: "show-about" }],
      },
    ],
    [explorerVisible],
  );

  return (
    <div className={styles.root}>
      <AppTitleBar
        activeProjectName={activeLocation?.displayName ?? null}
        activeProjectRoot={activeLocation?.rootPath ?? null}
        onOpenProject={handleOpenProject}
        onAddSourceFolder={handleAddSourceFolder}
        onRefresh={workspace.refresh}
        onTogglePalette={() => setPaletteOpen((o) => !o)}
        onToggleExplorer={() => handleSelectView("explorer")}
        explorerVisible={explorerVisible}
        commands={commands}
        menus={menus}
      />
      <div className={styles.middle}>
        {startupNotice && (
          <div className={styles.startupNotice}>
            <span className={styles.startupNoticeMessage}>
              {t("shell:startupNotice.projectNotFound", {
                displayName: startupNotice.displayName,
                rootPath: startupNotice.rootPath,
              })}
            </span>
            <button
              className="icon-btn"
              style={{ width: 20, height: 20, flexShrink: 0 }}
              onClick={clearStartupNotice}
              aria-label={t("shell:startupNotice.dismiss")}
              title={t("shell:startupNotice.dismiss")}
            >
              <X size={12} />
            </button>
          </div>
        )}
        {workshopRootNotice && (
          <div className={styles.startupNotice}>
            <span className={styles.startupNoticeMessage}>
              {t("shell:workshopRootNotice.message", { displayName: workshopRootNotice })}
            </span>
            <button
              className="icon-btn"
              style={{ width: 20, height: 20, flexShrink: 0 }}
              onClick={() => setWorkshopRootNotice(null)}
              aria-label={t("shell:workshopRootNotice.dismiss")}
              title={t("shell:workshopRootNotice.dismiss")}
            >
              <X size={12} />
            </button>
          </div>
        )}
        <div
          className={styles.workspace}
          ref={workspaceRef}
          style={{ "--explorer-width": `${explorerWidth}px` } as CSSProperties}
        >
          <ActivityRail
            activeView={activeView}
            onSelectView={handleSelectView}
            onOpenPreferences={() => setPreferencesOpen(true)}
          />
          <ProjectExplorerPanel
            visible={explorerVisible}
            scan={workspace.scan}
            fileTree={fileTree}
            activeFilePath={activeFilePath}
            loadingScan={workspace.loadingScan}
            refreshingScan={workspace.refreshingScan}
            hasActiveProject={!!activeProjectId}
            searchQuery={fileFilterQuery}
            filteredFiles={filteredFiles}
            expandedFolders={expandedFolders}
            onSearchChange={setFileFilterQuery}
            onToggleFolder={handleToggleFolder}
            onSelectFile={(file) =>
              void workspace.openTab(fileEntryToOpenFileRef(file))
            }
            onSelectFilePath={(relativePath) =>
              activeProjectId &&
              void workspace.openTab(projectPathToOpenFileRef(relativePath))
            }
            onRefresh={workspace.refresh}
            onOpenProject={handleOpenProject}
            onAddSourceFolder={handleAddSourceFolder}
            searchInputRef={fileSearchInputRef}
            mutationError={workspace.mutationError}
            onClearMutationError={workspace.clearMutationError}
            onCreateFile={async (parentPath, fileName) => {
              await workspace.createFile(parentPath, fileName);
            }}
            onCreateAndOpenFile={async (parentPath, fileName, contents) => {
              const entry = await workspace.createFile(parentPath, fileName, contents);
              workspace.openTab(fileEntryToOpenFileRef(entry));
            }}
            onCreateFolder={async (parentPath, folderName) => {
              await workspace.createFolder(parentPath, folderName);
            }}
            onRename={async (relativePath, newName, kind) => {
              const affectedDirty = workspace.tabs.filter(
                (tab) =>
                  tab.dirty &&
                  tab.sourceKind === "project" &&
                  (tab.relativePath === relativePath ||
                    tab.relativePath.startsWith(relativePath + "/")),
              );
              if (affectedDirty.length > 0) {
                if (
                  !(await confirmDiscardChanges(
                    t("shell:confirm.renameDiscard", { count: affectedDirty.length }),
                  ))
                )
                  return;
              }
              const result = await workspace.renamePath(
                relativePath,
                newName,
                kind,
              );
              if (affectedDirty.length > 0) {
                workspace.forceCloseTabs(affectedDirty.map((tab) => tab.tabKey));
              }
              workspace.reconcileRename(result.oldPath, result.newPath);
            }}
            onDelete={async (relativePath, kind) => {
              const affectedDirty = workspace.tabs.filter(
                (tab) =>
                  tab.dirty &&
                  tab.sourceKind === "project" &&
                  (tab.relativePath === relativePath ||
                    tab.relativePath.startsWith(relativePath + "/")),
              );
              if (affectedDirty.length > 0) {
                if (
                  !(await confirmDiscardChanges(
                    t("shell:confirm.deleteDiscard", { count: affectedDirty.length }),
                  ))
                )
                  return;
              }
              await workspace.deletePath(relativePath, kind);
              if (affectedDirty.length > 0) {
                workspace.forceCloseTabs(affectedDirty.map((tab) => tab.tabKey));
              }
              workspace.reconcileDelete(relativePath);
            }}
          />
          <DefSearchPanel
            visible={searchPanelVisible}
            projectId={activeProjectId}
            hasActiveProject={!!activeProjectId}
            indexRevision={indexRevision}
            onOpenProjectDef={(relativePath, nodeId) =>
              activeProjectId &&
              void workspace.openTab(
                projectPathToOpenFileRef(relativePath),
                nodeId !== undefined ? { selectedDefNodeId: nodeId } : undefined,
              )
            }
            onOpenSourceDef={(locationId, locationName, relativePath, nodeId) =>
              void workspace.openTab(
                {
                  locationId,
                  locationName,
                  sourceKind: "source",
                  readOnly: true,
                  relativePath,
                },
                nodeId !== undefined ? { selectedDefNodeId: nodeId } : undefined,
              )
            }
            onOpenProject={handleOpenProject}
            onAddSourceFolder={handleAddSourceFolder}
            searchInputRef={defSearchInputRef}
          />
          {activeView !== null && (
            <ResizablePaneHandle
              width={explorerWidth}
              minWidth={minExplorerWidth}
              maxWidth={maxExplorerWidth}
              defaultWidth={LAYOUT_DEFAULTS.explorerWidth}
              onChange={setExplorerWidth}
            />
          )}
          <EditorWorkspace
            tabs={workspace.tabs}
            activeTabKey={workspace.activeTabKey}
            projectId={activeProjectId}
            catalog={catalog}
            gameVersion={settings?.gameVersion}
            validationRefreshRevision={validationRefreshRevision}
            createDefSignal={createDefSignal}
            onActivateTab={workspace.activateTab}
            onCloseTab={workspace.closeTab}
            onTabDirtyChange={workspace.setTabDirty}
            onNavigateDef={(fileRef, nodeId) =>
              workspace.openTab(
                fileRef,
                nodeId !== null ? { selectedDefNodeId: nodeId } : undefined,
              )
            }
            onActiveCommandsChange={handleActiveCommandsChange}
          />
        </div>
      </div>
      <StatusBar
        hasActiveProject={!!activeProjectId}
        loadingScan={workspace.loadingScan}
        scanError={workspace.scanError}
        fileCount={workspace.scan?.files.length ?? 0}
        activeFilePath={activeFilePath}
        activeFileSizeBytes={activeFileEntry?.sizeBytes ?? null}
        indexingStatus={indexingStatus}
        projectId={activeProjectId}
      />
      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        commands={commands}
      />
      {aboutOpen && <AboutDialog onClose={() => setAboutOpen(false)} />}
      {preferencesOpen && (
        <PreferencesDialog
          onClose={() => setPreferencesOpen(false)}
          settings={settings}
          loading={loading}
          loadError={settingsLoadError}
          hasDirtyTabs={hasDirtyTabs}
          installedSchemaVersions={installedSchemaVersions}
          locale={locale}
          themeMode={themeMode}
          onChangeTheme={setMode}
          onEditLocation={editLocation}
          onRemoveLocation={deleteLocation}
          onUpdateGameVersion={updateGameVersion}
          onChangeLocale={changeLocale}
          onOpenProject={handleOpenProject}
          onAddSourceFolder={handleAddSourceFolder}
        />
      )}
    </div>
  );
}
