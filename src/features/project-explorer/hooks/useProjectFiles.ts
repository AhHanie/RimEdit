import { useState, useEffect, useCallback, useRef } from "react";
import type {
  ProjectFileScan,
  ProjectFileContent,
  ProjectFileEntry,
  ProjectFolderEntry,
  ProjectPathMutationResult,
} from "../types";
import {
  scanProjectFiles,
  readProjectXmlFile,
  createProjectFile,
  createProjectFolder,
  renameProjectPath,
  deleteProjectPath,
} from "../api/projectFiles";
import { formatError } from "../../../lib/formatError";

interface UseProjectFilesReturn {
  scan: ProjectFileScan | null;
  selectedFile: ProjectFileContent | null;
  loadingScan: boolean;
  refreshingScan: boolean;
  loadingFile: boolean;
  scanError: string | null;
  fileError: string | null;
  mutatingPath: string | null;
  mutationError: string | null;
  refresh: () => Promise<void>;
  openFile: (relativePath: string) => Promise<void>;
  clearFile: () => void;
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

export function useProjectFiles(activeProjectId: string | undefined): UseProjectFilesReturn {
  const [scan, setScan] = useState<ProjectFileScan | null>(null);
  const [selectedFile, setSelectedFile] = useState<ProjectFileContent | null>(null);
  const [scanRequestCount, setScanRequestCount] = useState(0);
  const [loadingFile, setLoadingFile] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);
  const [fileError, setFileError] = useState<string | null>(null);
  const [mutatingPath, setMutatingPath] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);

  const loadingScan = scan === null && scanRequestCount > 0;
  const refreshingScan = scan !== null && scanRequestCount > 0;

  // Tracks the most recently requested path; stale responses are discarded.
  const latestRequestRef = useRef<string | null>(null);
  // Tracks the current project id so in-flight scans from previous projects
  // are discarded rather than applied to a different project's state.
  const activeProjectIdRef = useRef(activeProjectId);

  const refresh = useCallback(async () => {
    if (!activeProjectId) {
      setScan(null);
      return;
    }
    const snapshotId = activeProjectId;
    setScanRequestCount((n) => n + 1);
    setScanError(null);
    try {
      const result = await scanProjectFiles(activeProjectId);
      if (activeProjectIdRef.current === snapshotId) {
        setScan(result);
      }
    } catch (e: unknown) {
      if (activeProjectIdRef.current === snapshotId) {
        setScanError(formatError(e));
        // Keep the previous scan visible on error; don't wipe the tree.
      }
    } finally {
      setScanRequestCount((n) => n - 1);
    }
  }, [activeProjectId]);

  useEffect(() => {
    activeProjectIdRef.current = activeProjectId;
    setScan(null);
    setSelectedFile(null);
    void refresh();
  }, [activeProjectId, refresh]);

  const clearFile = useCallback(() => {
    latestRequestRef.current = null;
    setSelectedFile(null);
    setFileError(null);
    setLoadingFile(false);
  }, []);

  const clearMutationError = useCallback(() => {
    setMutationError(null);
  }, []);

  const openFile = useCallback(
    async (relativePath: string) => {
      if (!activeProjectId) return;
      latestRequestRef.current = relativePath;
      setLoadingFile(true);
      setFileError(null);
      try {
        const content = await readProjectXmlFile(activeProjectId, relativePath);
        if (latestRequestRef.current === relativePath) {
          setSelectedFile(content);
        }
      } catch (e: unknown) {
        if (latestRequestRef.current === relativePath) {
          setFileError(formatError(e));
        }
      } finally {
        if (latestRequestRef.current === relativePath) {
          setLoadingFile(false);
        }
      }
    },
    [activeProjectId],
  );

  const createFile = useCallback(
    async (parentPath: string, fileName: string, contents?: string) => {
      if (!activeProjectId) throw new Error("No active project");
      setMutationError(null);
      setMutatingPath(fileName);
      try {
        const entry = await createProjectFile(activeProjectId, parentPath, fileName, contents);
        await refresh();
        return entry;
      } catch (e: unknown) {
        setMutationError(formatError(e));
        throw e;
      } finally {
        setMutatingPath(null);
      }
    },
    [activeProjectId, refresh],
  );

  const createFolder = useCallback(
    async (parentPath: string, folderName: string) => {
      if (!activeProjectId) throw new Error("No active project");
      setMutationError(null);
      setMutatingPath(folderName);
      try {
        const entry = await createProjectFolder(activeProjectId, parentPath, folderName);
        await refresh();
        return entry;
      } catch (e: unknown) {
        setMutationError(formatError(e));
        throw e;
      } finally {
        setMutatingPath(null);
      }
    },
    [activeProjectId, refresh],
  );

  const renamePath = useCallback(
    async (relativePath: string, newName: string, kind: "file" | "folder") => {
      if (!activeProjectId) throw new Error("No active project");
      setMutationError(null);
      setMutatingPath(relativePath);
      try {
        const result = await renameProjectPath(activeProjectId, relativePath, newName, kind);
        await refresh();
        return result;
      } catch (e: unknown) {
        setMutationError(formatError(e));
        throw e;
      } finally {
        setMutatingPath(null);
      }
    },
    [activeProjectId, refresh],
  );

  const deletePath = useCallback(
    async (relativePath: string, kind: "file" | "folder") => {
      if (!activeProjectId) throw new Error("No active project");
      setMutationError(null);
      setMutatingPath(relativePath);
      try {
        const result = await deleteProjectPath(activeProjectId, relativePath, kind);
        await refresh();
        return result;
      } catch (e: unknown) {
        setMutationError(formatError(e));
        throw e;
      } finally {
        setMutatingPath(null);
      }
    },
    [activeProjectId, refresh],
  );

  return {
    scan,
    selectedFile,
    loadingScan,
    refreshingScan,
    loadingFile,
    scanError,
    fileError,
    mutatingPath,
    mutationError,
    refresh,
    openFile,
    clearFile,
    clearMutationError,
    createFile,
    createFolder,
    renamePath,
    deletePath,
  };
}
