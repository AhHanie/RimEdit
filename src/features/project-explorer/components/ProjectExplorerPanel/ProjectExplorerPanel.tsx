import React from "react";
import { useTranslation } from "react-i18next";
import { Search, RefreshCw, X, FolderOpen, Loader2, FileCode2, FileText, File } from "lucide-react";
import type { FileTreeFolderNode, ProjectFileEntry, ProjectFileScan, ProjectFileKind } from "../../types";
import { FileTree } from "../FileTree/FileTree";
import styles from "./ProjectExplorerPanel.module.css";

function fileSearchIcon(fileKind: ProjectFileKind) {
  switch (fileKind) {
    case "xml":
      return FileCode2;
    case "text":
      return FileText;
    default:
      return File;
  }
}

interface ProjectExplorerPanelProps {
  visible: boolean;
  scan: ProjectFileScan | null;
  fileTree: FileTreeFolderNode | null;
  activeFilePath: string | null;
  loadingScan: boolean;
  refreshingScan: boolean;
  hasActiveProject: boolean;
  searchQuery: string;
  filteredFiles: ProjectFileEntry[];
  expandedFolders: Set<string>;
  onSearchChange: (q: string) => void;
  onToggleFolder: (id: string) => void;
  onSelectFile: (file: ProjectFileEntry) => void;
  onSelectFilePath?: (relativePath: string) => void;
  onRefresh: () => void;
  onOpenProject: () => void;
  onAddSourceFolder: () => void;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  mutationError: string | null;
  onClearMutationError: () => void;
  onCreateFile: (parentPath: string, fileName: string) => Promise<void>;
  onCreateAndOpenFile: (
    parentPath: string,
    fileName: string,
    contents: string,
  ) => Promise<void>;
  onCreateFolder: (parentPath: string, folderName: string) => Promise<void>;
  onRename: (relativePath: string, newName: string, kind: "file" | "folder") => Promise<void>;
  onDelete: (relativePath: string, kind: "file" | "folder") => Promise<void>;
}

export function ProjectExplorerPanel({
  visible,
  scan,
  fileTree,
  activeFilePath,
  loadingScan,
  refreshingScan,
  hasActiveProject,
  searchQuery,
  filteredFiles,
  expandedFolders,
  onSearchChange,
  onToggleFolder,
  onSelectFile,
  onSelectFilePath,
  onRefresh,
  onOpenProject,
  onAddSourceFolder,
  searchInputRef,
  mutationError,
  onClearMutationError,
  onCreateFile,
  onCreateAndOpenFile,
  onCreateFolder,
  onRename,
  onDelete,
}: ProjectExplorerPanelProps) {
  const { t } = useTranslation(["shell", "common"]);

  function renderContent() {
    if (!hasActiveProject) {
      return (
        <div className="state-empty">
          <FolderOpen size={32} className="state-empty-icon" />
          <p className="state-empty-text">{t("shell:explorer.noProjectOpen")}</p>
          <button className="btn-primary" onClick={onOpenProject}>
            {t("common:actions.openProject")}
          </button>
          <button className="btn-secondary" onClick={onAddSourceFolder}>
            {t("common:actions.addSourceFolder")}
          </button>
        </div>
      );
    }

    if (loadingScan && !scan) {
      return (
        <div className="state-loading">
          <Loader2 size={14} className="spin" />
          <span>{t("shell:explorer.scanningProject")}</span>
        </div>
      );
    }

    if (searchQuery) {
      if (filteredFiles.length === 0) {
        return (
          <div className="state-empty">
            <p className="state-empty-text">{t("shell:explorer.noMatchingFiles")}</p>
          </div>
        );
      }
      return (
        <div
          className={styles.searchResults}
          role="list"
          aria-label={t("shell:explorer.searchResultsAriaLabel")}
        >
          {filteredFiles.map((file) => {
            const SearchIcon = fileSearchIcon(file.fileKind);
            return (
              <button
                key={file.relativePath}
                className={`${styles.searchRow}${file.relativePath === activeFilePath ? ` ${styles.searchRowActive}` : ""}`}
                style={{ paddingInlineStart: 8 }}
                onClick={() => onSelectFile(file)}
                title={file.relativePath}
                aria-current={file.relativePath === activeFilePath ? "true" : undefined}
                role="listitem"
              >
                <SearchIcon size={14} className={styles.fileIcon} />
                <span className={styles.fileLabel}>{file.fileName}</span>
                {file.folderPath && (
                  <span className={styles.fileSubtitle}>{file.folderPath}</span>
                )}
              </button>
            );
          })}
        </div>
      );
    }

    if (!fileTree) return null;

    return (
      <FileTree
        root={fileTree}
        activeFilePath={activeFilePath}
        expandedFolders={expandedFolders}
        onToggleFolder={onToggleFolder}
        onSelectFile={onSelectFilePath ?? (() => undefined)}
        onCreateFile={onCreateFile}
        onCreateAndOpenFile={onCreateAndOpenFile}
        onCreateFolder={onCreateFolder}
        onRename={onRename}
        onDelete={onDelete}
      />
    );
  }

  return (
    <aside className={styles.root} data-visible={visible ? "true" : "false"}>
      <div className={styles.header}>
        <span className={styles.title}>
          {scan ? t("shell:explorer.filesTitle", { count: scan.files.length }) : t("shell:explorer.title")}
        </span>
        <button
          className="icon-btn"
          onClick={onRefresh}
          aria-label={t("shell:explorer.refreshFiles")}
          title={t("shell:explorer.refreshFiles")}
          disabled={!hasActiveProject || loadingScan || refreshingScan}
        >
          <RefreshCw size={13} className={refreshingScan ? "spin" : undefined} />
        </button>
      </div>

      <div className={styles.search}>
        <Search size={13} className={styles.searchIcon} aria-hidden="true" />
        <input
          ref={searchInputRef}
          className={styles.searchInput}
          type="search"
          placeholder={t("shell:explorer.filterPlaceholder")}
          value={searchQuery}
          onChange={(e) => onSearchChange(e.currentTarget.value)}
          aria-label={t("shell:explorer.filterAriaLabel")}
        />
        {searchQuery && (
          <button
            className="icon-btn"
            style={{ width: 20, height: 20 }}
            onClick={() => onSearchChange("")}
            aria-label={t("shell:explorer.clearSearch")}
            title={t("shell:explorer.clearSearch")}
          >
            <X size={12} />
          </button>
        )}
      </div>

      {mutationError && (
        <div className={styles.errorBanner}>
          <span className={styles.errorMessage}>{mutationError}</span>
          <button
            className="icon-btn"
            style={{ width: 20, height: 20, flexShrink: 0 }}
            onClick={onClearMutationError}
            aria-label={t("shell:explorer.dismissError")}
            title={t("shell:explorer.dismissError")}
          >
            <X size={12} />
          </button>
        </div>
      )}
      <div className={styles.content}>{renderContent()}</div>
    </aside>
  );
}
