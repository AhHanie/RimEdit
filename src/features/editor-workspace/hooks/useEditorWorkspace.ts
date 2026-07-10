import { useState, useEffect, useCallback } from "react";
import {
  useProjectFiles,
  type ProjectFileScan,
  type ProjectFileEntry,
  type ProjectFolderEntry,
  type ProjectPathMutationResult,
} from "../../project-explorer";
import {
  makeOpenFileTabKey,
  type OpenFileEditorKind,
  type OpenFileRef,
  type OpenFileTab,
} from "../types";

export interface UseEditorWorkspaceReturn {
  scan: ProjectFileScan | null;
  loadingScan: boolean;
  refreshingScan: boolean;
  scanError: string | null;
  refresh: () => Promise<void>;
  tabs: OpenFileTab[];
  activeTabKey: string | null;
  openTab: (file: OpenFileRef, options?: { selectedDefNodeId?: number }) => void;
  activateTab: (tabKey: string) => void;
  closeTab: (tabKey: string) => void;
  setTabDirty: (tabKey: string, dirty: boolean) => void;
  reconcileRename: (oldPath: string, newPath: string) => void;
  reconcileDelete: (deletedPath: string) => void;
  forceCloseTabs: (tabKeys: string[]) => void;
  mutatingPath: string | null;
  mutationError: string | null;
  clearMutationError: () => void;
  createFile: (
    parentPath: string,
    fileName: string,
    contents?: string,
  ) => Promise<ProjectFileEntry>;
  createFolder: (parentPath: string, folderName: string) => Promise<ProjectFolderEntry>;
  renamePath: (
    relativePath: string,
    newName: string,
    kind: "file" | "folder",
  ) => Promise<ProjectPathMutationResult>;
  deletePath: (
    relativePath: string,
    kind: "file" | "folder",
  ) => Promise<ProjectPathMutationResult>;
}

function editorKindFromFileKind(
  fileKind: string | undefined,
): OpenFileEditorKind {
  return fileKind === "xml" ? "xml" : "unsupported";
}

export function useEditorWorkspace(activeProjectId: string | undefined): UseEditorWorkspaceReturn {
  const {
    scan,
    loadingScan,
    refreshingScan,
    scanError,
    refresh,
    mutatingPath,
    mutationError,
    clearMutationError,
    createFile,
    createFolder,
    renamePath,
    deletePath,
  } = useProjectFiles(activeProjectId);

  const [tabs, setTabs] = useState<OpenFileTab[]>([]);
  const [activeTabKey, setActiveTabKey] = useState<string | null>(null);

  useEffect(() => {
    setTabs([]);
    setActiveTabKey(null);
  }, [activeProjectId]);

  const activateTab = useCallback((tabKey: string) => {
    setActiveTabKey(tabKey);
  }, []);

  const openTab = useCallback(
    (file: OpenFileRef, options?: { selectedDefNodeId?: number }) => {
      const tabKey = makeOpenFileTabKey(file);
      const entry =
        file.sourceKind === "project"
          ? scan?.files.find((f) => f.relativePath === file.relativePath)
          : undefined;
      const lastSlash = file.relativePath.lastIndexOf("/");

      setTabs((prev) => {
        const existing = prev.find((t) => t.tabKey === tabKey);
        if (existing) {
          if (options?.selectedDefNodeId !== undefined) {
            return prev.map((tab) =>
              tab.tabKey === tabKey
                ? {
                    ...tab,
                    selectedDefNodeId: options.selectedDefNodeId,
                    selectionRequestId: (tab.selectionRequestId ?? 0) + 1,
                  }
                : tab,
            );
          }
          return prev;
        }
        const editorKind: OpenFileEditorKind =
          file.editorKindHint ??
          (file.sourceKind === "source" ? "xml" : editorKindFromFileKind(entry?.fileKind));
        const newTab: OpenFileTab = {
          ...file,
          tabKey,
          fileName:
            entry?.fileName ??
            (lastSlash === -1 ? file.relativePath : file.relativePath.slice(lastSlash + 1)),
          folderPath:
            entry?.folderPath ?? (lastSlash === -1 ? "" : file.relativePath.slice(0, lastSlash)),
          dirty: false,
          editorKind,
          selectedDefNodeId: options?.selectedDefNodeId,
          selectionRequestId: options?.selectedDefNodeId !== undefined ? 1 : undefined,
        };
        return [...prev, newTab];
      });

      setActiveTabKey(tabKey);
    },
    [scan],
  );

  const closeTab = useCallback(
    (tabKey: string) => {
      setTabs((prev) => {
        const idx = prev.findIndex((t) => t.tabKey === tabKey);
        if (idx === -1) return prev;
        const filtered = prev.filter((t) => t.tabKey !== tabKey);

        if (activeTabKey === tabKey) {
          const neighbor = filtered[Math.max(0, idx - 1)] ?? filtered[0] ?? null;
          setActiveTabKey(neighbor ? neighbor.tabKey : null);
        }

        return filtered;
      });
    },
    [activeTabKey],
  );

  const setTabDirty = useCallback((tabKey: string, dirty: boolean) => {
    setTabs((prev) => {
      const current = prev.find((tab) => tab.tabKey === tabKey);
      if (!current || current.readOnly || current.dirty === dirty) return prev;
      return prev.map((tab) => (tab.tabKey === tabKey ? { ...tab, dirty } : tab));
    });
  }, []);

  // Update clean project tabs whose path has changed after a rename.
  const reconcileRename = useCallback(
    (oldPath: string, newPath: string) => {
      if (!activeProjectId) return;
      setTabs((prev) => {
        let changed = false;
        const next = prev.map((tab) => {
          if (tab.sourceKind !== "project" || tab.dirty) return tab;
          // Exact file match
          if (tab.relativePath === oldPath) {
            const lastSlash = newPath.lastIndexOf("/");
            const fileName =
              lastSlash === -1 ? newPath : newPath.slice(lastSlash + 1);
            const folderPath = lastSlash === -1 ? "" : newPath.slice(0, lastSlash);
            const ext = fileName.includes(".")
              ? fileName.slice(fileName.lastIndexOf(".") + 1).toLowerCase()
              : "";
            const editorKind: OpenFileEditorKind = ext === "xml" ? "xml" : "unsupported";
            const newTabKey = makeOpenFileTabKey({
              locationId: tab.locationId,
              relativePath: newPath,
            });
            changed = true;
            return {
              ...tab,
              relativePath: newPath,
              fileName,
              folderPath,
              tabKey: newTabKey,
              editorKind,
            };
          }
          // Child of renamed folder
          if (tab.relativePath.startsWith(oldPath + "/")) {
            const rest = tab.relativePath.slice(oldPath.length);
            const updatedPath = newPath + rest;
            const lastSlash = updatedPath.lastIndexOf("/");
            const fileName =
              lastSlash === -1 ? updatedPath : updatedPath.slice(lastSlash + 1);
            const folderPath = lastSlash === -1 ? "" : updatedPath.slice(0, lastSlash);
            const ext = fileName.includes(".")
              ? fileName.slice(fileName.lastIndexOf(".") + 1).toLowerCase()
              : "";
            const editorKind: OpenFileEditorKind = ext === "xml" ? "xml" : "unsupported";
            const newTabKey = makeOpenFileTabKey({
              locationId: tab.locationId,
              relativePath: updatedPath,
            });
            changed = true;
            return {
              ...tab,
              relativePath: updatedPath,
              fileName,
              folderPath,
              tabKey: newTabKey,
              editorKind,
            };
          }
          return tab;
        });
        if (!changed) return prev;
        return next;
      });
      setActiveTabKey((prev) => {
        if (!prev) return prev;
        // If the active tab key matched the old path, update it
        const oldTabKey = `${activeProjectId}:${oldPath}`;
        if (prev === oldTabKey) {
          return `${activeProjectId}:${newPath}`;
        }
        if (prev.startsWith(`${activeProjectId}:${oldPath}/`)) {
          const rest = prev.slice(`${activeProjectId}:${oldPath}`.length);
          return `${activeProjectId}:${newPath}${rest}`;
        }
        return prev;
      });
    },
    [activeProjectId],
  );

  // Force-close tabs regardless of dirty state (called before a backend op that will make them stale).
  const forceCloseTabs = useCallback((tabKeys: string[]) => {
    const keySet = new Set(tabKeys);
    setTabs((prev) => {
      const filtered = prev.filter((t) => !keySet.has(t.tabKey));
      setActiveTabKey((active) => {
        if (!active || !keySet.has(active)) return active;
        const removedIdx = prev.findIndex((t) => t.tabKey === active);
        const neighbor = filtered[Math.max(0, removedIdx - 1)] ?? filtered[0] ?? null;
        return neighbor ? neighbor.tabKey : null;
      });
      return filtered;
    });
  }, []);

  // Close clean project tabs at or under a deleted path.
  const reconcileDelete = useCallback(
    (deletedPath: string) => {
      if (!activeProjectId) return;
      setTabs((prev) => {
        const toRemove = new Set<string>();
        for (const tab of prev) {
          if (tab.sourceKind !== "project" || tab.dirty) continue;
          if (
            tab.relativePath === deletedPath ||
            tab.relativePath.startsWith(deletedPath + "/")
          ) {
            toRemove.add(tab.tabKey);
          }
        }
        if (toRemove.size === 0) return prev;
        const filtered = prev.filter((t) => !toRemove.has(t.tabKey));
        setActiveTabKey((active) => {
          if (!active || !toRemove.has(active)) return active;
          const removedIdx = prev.findIndex((t) => t.tabKey === active);
          const remaining = prev.filter((t) => !toRemove.has(t.tabKey));
          const neighbor = remaining[Math.max(0, removedIdx - 1)] ?? remaining[0] ?? null;
          return neighbor ? neighbor.tabKey : null;
        });
        return filtered;
      });
    },
    [activeProjectId],
  );

  return {
    scan,
    loadingScan,
    refreshingScan,
    scanError,
    refresh,
    tabs,
    activeTabKey,
    openTab,
    activateTab,
    closeTab,
    setTabDirty,
    reconcileRename,
    reconcileDelete,
    forceCloseTabs,
    mutatingPath,
    mutationError,
    clearMutationError,
    createFile,
    createFolder,
    renamePath,
    deletePath,
  };
}
