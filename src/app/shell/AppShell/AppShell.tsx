import {
  useState,
  useEffect,
  useRef,
  useMemo,
  useCallback,
  type CSSProperties,
} from "react";
import {
  FolderOpen,
  FolderPlus,
  FilePlus,
  RefreshCw,
  PanelLeft,
  Search,
  Settings,
  Sun,
  Moon,
  Monitor,
  X,
} from "lucide-react";
import {
  pickProjectFolder,
  pickSourceFolder,
  useProjectSettings,
  ProjectSettingsPanel,
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
import { confirmDiscardChanges } from "../../../lib/confirmDiscardChanges";
import {
  buildFileTree,
  filterFiles,
  collectFolderIds,
  ProjectExplorerPanel,
} from "../../../features/project-explorer";
import type { ProjectFileEntry } from "../../../features/project-explorer";
import { DefSearchPanel, useIndexingStatus } from "../../../features/def-index";
import type { ActivityView } from "../types";
import type { CommandAction } from "../../commands/commandTypes";
import { AppTitleBar } from "../AppTitleBar/AppTitleBar";
import { ActivityRail } from "../ActivityRail/ActivityRail";
import { StatusBar } from "../StatusBar/StatusBar";
import { CommandPalette } from "../../commands/CommandPalette/CommandPalette";
import { ResizablePaneHandle } from "../ResizablePaneHandle/ResizablePaneHandle";
import { usePersistentLayoutState } from "../layout/usePersistentLayoutState";
import { LAYOUT_DEFAULTS } from "../layout/layoutState";
import styles from "./AppShell.module.css";

export function AppShell() {
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
  } = useProjectSettings();
  const activeProjectId = settings?.activeProjectId;
  const activeLocation = settings?.locations.find(
    (l) => l.id === activeProjectId,
  );

  const workspace = useEditorWorkspace(activeProjectId);
  // Catalog context must match the same "every registered location root" policy the backend
  // uses for document validation and patch preview (see `services::validation::schema_pack_roots`
  // and `services::patch_preview::preview_def_for_project`) -- there is no separate "configured
  // external schema roots" setting anywhere in `ProjectSettings`/`RegisteredLocation` today, so
  // this reuses the same project-location data rather than inventing a second registry (Plan.md
  // section 2/15, issue 09's "avoid inventing a second registry"). `schema_pack::loader` searches
  // each root, its `About/`, and its `SchemaPacks/<name>/` for an embedded schema pack.
  // Memoized on `settings?.locations`'s own reference (not recomputed on every unrelated AppShell
  // re-render) so it doesn't churn `useSchemaCatalog`'s reload effect.
  const extraSchemaRoots = useMemo(
    () => settings?.locations.map((l) => l.rootPath) ?? [],
    [settings?.locations],
  );
  const { catalog } = useSchemaCatalog(extraSchemaRoots, settings?.gameVersion);
  const { mode: themeMode, setMode, cycleMode: cycleTheme } = useTheme();

  const [activeView, setActiveView] = useState<ActivityView | null>("explorer");
  const [fileFilterQuery, setFileFilterQuery] = useState("");
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(
    new Set(),
  );
  const [paletteOpen, setPaletteOpen] = useState(false);
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
  const settingsVisible = activeView === "settings";

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

  const fileTree = useMemo(
    () =>
      workspace.scan
        ? buildFileTree(
            workspace.scan,
            activeLocation?.displayName ?? "Project",
          )
        : null,
    [workspace.scan, activeLocation?.displayName],
  );

  const filteredFiles = useMemo(
    () =>
      workspace.scan ? filterFiles(workspace.scan.files, fileFilterQuery) : [],
    [workspace.scan, fileFilterQuery],
  );

  const activeTab = useMemo(
    () =>
      workspace.tabs.find((t) => t.tabKey === workspace.activeTabKey) ?? null,
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
    } catch (e: unknown) {
      console.error("Failed to add source folder:", e);
    }
  }, [replaceSettings, settings]);

  const handleOpenProject = useCallback(async () => {
    try {
      if (
        hasDirtyTabs &&
        !(await confirmDiscardChanges(
          "Open another project and discard unsaved changes?",
        ))
      ) {
        return;
      }
      const result = await pickProjectFolder();
      if (!result) return;
      await activateProject(result.locationId);
    } catch (e: unknown) {
      console.error("Failed to open project:", e);
    }
  }, [activateProject, hasDirtyTabs]);

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
  }, [paletteOpen, fileFilterQuery]);

  const commands = useMemo<CommandAction[]>(
    () => [
      {
        id: "open-project",
        label: "Open Project",
        keywords: ["folder", "open", "browse"],
        icon: FolderOpen,
        run: handleOpenProject,
      },
      {
        id: "add-source-folder",
        label: "Add Source Folder",
        keywords: ["source", "reference", "readonly", "folder", "index"],
        icon: FolderPlus,
        run: handleAddSourceFolder,
      },
      {
        id: "refresh",
        label: "Refresh Project Files",
        keywords: ["scan", "reload", "refresh"],
        icon: RefreshCw,
        run: workspace.refresh,
        disabled: !activeProjectId,
      },
      {
        id: "open-settings",
        label: "Open Settings",
        keywords: ["settings", "locations", "sources", "projects", "configure"],
        icon: Settings,
        run: () => setActiveView("settings"),
      },
      {
        id: "toggle-explorer",
        label: "Toggle Explorer",
        keywords: ["sidebar", "panel", "files", "toggle"],
        icon: PanelLeft,
        run: () => setActiveView((v) => (v === "explorer" ? null : "explorer")),
      },
      {
        id: "focus-search",
        label: "Focus Def Search",
        keywords: ["filter", "find", "search", "def"],
        icon: Search,
        run: () => {
          setActiveView("search");
          setTimeout(() => defSearchInputRef.current?.focus(), 0);
        },
      },
      {
        id: "theme-light",
        label: "Theme: Light",
        keywords: ["color", "appearance", "light"],
        icon: Sun,
        run: () => setMode("light"),
      },
      {
        id: "theme-dark",
        label: "Theme: Dark",
        keywords: ["color", "appearance", "dark"],
        icon: Moon,
        run: () => setMode("dark"),
      },
      {
        id: "theme-system",
        label: "Theme: System",
        keywords: ["color", "appearance", "system", "auto"],
        icon: Monitor,
        run: () => setMode("system"),
      },
      {
        id: "create-def",
        label: "Create Def from Template",
        keywords: ["new", "def", "template", "insert", "create", "add"],
        icon: FilePlus,
        run: () => setCreateDefSignal((s) => s + 1),
        disabled: !activeProjectId || !activeTab || activeTab.readOnly,
      },
    ],
    [
      handleOpenProject,
      handleAddSourceFolder,
      workspace.refresh,
      activeProjectId,
      activeTab,
      setMode,
      setActiveView,
    ],
  );

  return (
    <div className={styles.root}>
      <AppTitleBar
        activeProjectName={activeLocation?.displayName ?? null}
        activeProjectRoot={activeLocation?.rootPath ?? null}
        themeMode={themeMode}
        onCycleTheme={cycleTheme}
        onOpenProject={handleOpenProject}
        onAddSourceFolder={handleAddSourceFolder}
        onRefresh={workspace.refresh}
        onTogglePalette={() => setPaletteOpen((o) => !o)}
        onToggleExplorer={() => handleSelectView("explorer")}
        explorerVisible={explorerVisible}
      />
      <div className={styles.middle}>
        {startupNotice && (
          <div className={styles.startupNotice}>
            <span className={styles.startupNoticeMessage}>
              Project folder not found: {startupNotice.displayName} (
              {startupNotice.rootPath})
            </span>
            <button
              className="icon-btn"
              style={{ width: 20, height: 20, flexShrink: 0 }}
              onClick={clearStartupNotice}
              aria-label="Dismiss project notice"
              title="Dismiss project notice"
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
          <ActivityRail activeView={activeView} onSelectView={handleSelectView} />
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
                (t) =>
                  t.dirty &&
                  t.sourceKind === "project" &&
                  (t.relativePath === relativePath ||
                    t.relativePath.startsWith(relativePath + "/")),
              );
              if (affectedDirty.length > 0) {
                const noun =
                  affectedDirty.length === 1 ? "file has" : "files have";
                if (
                  !(await confirmDiscardChanges(
                    `${affectedDirty.length} open ${noun} unsaved changes. Rename and discard them?`,
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
                workspace.forceCloseTabs(affectedDirty.map((t) => t.tabKey));
              }
              workspace.reconcileRename(result.oldPath, result.newPath);
            }}
            onDelete={async (relativePath, kind) => {
              const affectedDirty = workspace.tabs.filter(
                (t) =>
                  t.dirty &&
                  t.sourceKind === "project" &&
                  (t.relativePath === relativePath ||
                    t.relativePath.startsWith(relativePath + "/")),
              );
              if (affectedDirty.length > 0) {
                const noun =
                  affectedDirty.length === 1 ? "file has" : "files have";
                if (
                  !(await confirmDiscardChanges(
                    `${affectedDirty.length} open ${noun} unsaved changes. Delete and discard them?`,
                  ))
                )
                  return;
              }
              await workspace.deletePath(relativePath, kind);
              if (affectedDirty.length > 0) {
                workspace.forceCloseTabs(affectedDirty.map((t) => t.tabKey));
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
          <ProjectSettingsPanel
            visible={settingsVisible}
            settings={settings}
            loading={loading}
            loadError={settingsLoadError}
            hasDirtyTabs={hasDirtyTabs}
            installedSchemaVersions={installedSchemaVersions}
            onEditLocation={editLocation}
            onRemoveLocation={deleteLocation}
            onUpdateGameVersion={updateGameVersion}
            onOpenProject={handleOpenProject}
            onAddSourceFolder={handleAddSourceFolder}
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
        themeMode={themeMode}
        indexingStatus={indexingStatus}
      />
      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        commands={commands}
      />
    </div>
  );
}
