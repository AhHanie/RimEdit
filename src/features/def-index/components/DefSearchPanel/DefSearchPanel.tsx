import React, { useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  Search,
  X,
  RefreshCw,
  FolderOpen,
  FolderPlus,
  Loader2,
} from "lucide-react";
import { rebuildDefIndex } from "../../api/defIndex";
import { useDefSearch } from "../../hooks/useDefSearch";
import type { IndexedDefSearchResult } from "../../types";
import styles from "./DefSearchPanel.module.css";

interface DefSearchPanelProps {
  visible: boolean;
  projectId: string | undefined;
  hasActiveProject: boolean;
  indexRevision?: number;
  onOpenProjectDef: (relativePath: string, nodeId?: number) => void;
  onOpenSourceDef: (
    locationId: string,
    locationName: string | undefined,
    relativePath: string,
    nodeId?: number,
  ) => void;
  onOpenProject: () => void;
  onAddSourceFolder: () => void;
  searchInputRef?: React.RefObject<HTMLInputElement | null>;
}

export function DefSearchPanel({
  visible,
  projectId,
  hasActiveProject,
  indexRevision,
  onOpenProjectDef,
  onOpenSourceDef,
  onOpenProject,
  onAddSourceFolder,
  searchInputRef,
}: DefSearchPanelProps) {
  const { t } = useTranslation(["shell", "common"]);
  const {
    query,
    setQuery,
    selectedDefType,
    setSelectedDefType,
    includeSources,
    setIncludeSources,
    loading,
    results,
    facets,
    reloadFacets,
  } = useDefSearch(projectId);

  // Reload facets and search results when indexing completes
  useEffect(() => {
    if (indexRevision !== undefined && indexRevision > 0) {
      reloadFacets();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [indexRevision]);

  useEffect(() => {
    if (visible && searchInputRef?.current) {
      searchInputRef.current.focus();
    }
  }, [visible, searchInputRef]);

  async function handleRebuild() {
    if (!projectId) return;
    try {
      await rebuildDefIndex(projectId);
      reloadFacets();
    } catch {
      // ignore
    }
  }

  function handleSelectResult(result: IndexedDefSearchResult) {
    const isProject = result.def.source.sourceKind === "project";
    if (isProject) {
      onOpenProjectDef(result.def.relativePath, result.def.nodeId);
    } else {
      onOpenSourceDef(
        result.def.source.locationId,
        result.def.source.locationName,
        result.def.relativePath,
        result.def.nodeId,
      );
    }
  }

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

    if (loading) {
      return (
        <div className="state-loading">
          <Loader2 size={14} className="spin" />
          <span>{t("shell:search.searching")}</span>
        </div>
      );
    }

    if (results.length === 0) {
      return (
        <div className="state-empty">
          <p className="state-empty-text">
            {query ? t("shell:search.noMatchingDefs") : t("shell:search.typeToSearch")}
          </p>
          {!query && (
            <button className="btn-secondary" onClick={onAddSourceFolder}>
              <FolderPlus size={13} />
              {t("common:actions.addSourceFolder")}
            </button>
          )}
        </div>
      );
    }

    return (
      <div
        className={styles.results}
        role="list"
        aria-label={t("shell:search.resultsAriaLabel")}
      >
        {results.map((result, i) => {
          const isProject = result.def.source.sourceKind === "project";
          const pathMeta = result.def.line
            ? `${result.def.relativePath}:${result.def.line}`
            : result.def.relativePath;
          return (
            <button
              key={`${result.def.source.locationId}:${result.def.relativePath}:${result.def.defName}:${i}`}
              className={styles.resultRow}
              onClick={() => handleSelectResult(result)}
              title={t("shell:search.resultTitle", {
                defName: result.def.defName,
                relativePath: result.def.relativePath,
              })}
              role="listitem"
            >
              <span className={styles.resultName}>{result.def.defName}</span>
              {result.def.label && (
                <span className={styles.resultLabel}>{result.def.label}</span>
              )}
              <span className={styles.resultMeta}>
                <span className={styles.resultPath}>
                  {result.def.defType} · {pathMeta}
                </span>
                {!isProject && (
                  <span className={styles.resultSource}>
                    {result.def.source.locationName}
                  </span>
                )}
                <span
                  className={`${styles.badge} ${isProject ? styles.badgeProject : styles.badgeSource}`}
                >
                  {isProject ? t("shell:search.project") : t("shell:search.readOnlySource")}
                </span>
              </span>
            </button>
          );
        })}
      </div>
    );
  }

  return (
    <aside className={styles.root} data-visible={visible ? "true" : "false"}>
      <div className={styles.header}>
        <span className={styles.title}>{t("shell:search.title")}</span>
        <button
          className="icon-btn"
          onClick={() => void handleRebuild()}
          aria-label={t("shell:search.rebuildIndex")}
          title={t("shell:search.rebuildIndex")}
          disabled={!hasActiveProject}
        >
          <RefreshCw size={13} />
        </button>
      </div>

      <div className={styles.search}>
        <Search size={13} className={styles.searchIcon} aria-hidden="true" />
        <input
          ref={searchInputRef}
          className={styles.searchInput}
          type="search"
          placeholder={t("shell:search.searchPlaceholder")}
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
          aria-label={t("shell:search.searchAriaLabel")}
          disabled={!hasActiveProject}
        />
        {query && (
          <button
            className="icon-btn"
            style={{ width: 20, height: 20 }}
            onClick={() => setQuery("")}
            aria-label={t("shell:search.clearSearch")}
            title={t("shell:search.clearSearch")}
          >
            <X size={12} />
          </button>
        )}
      </div>

      <div className={styles.filters}>
        <select
          className={styles.typeSelect}
          value={selectedDefType ?? ""}
          onChange={(e) =>
            setSelectedDefType(e.currentTarget.value || undefined)
          }
          aria-label={t("shell:search.filterByDefType")}
          disabled={!hasActiveProject}
        >
          <option value="">{t("shell:search.allDefTypes")}</option>
          {facets?.defTypes.map((ft) => (
            <option key={ft.defType} value={ft.defType}>
              {ft.defType} ({ft.totalCount})
            </option>
          ))}
        </select>
        <label className={styles.includeSourcesLabel}>
          <input
            type="checkbox"
            checked={includeSources}
            onChange={(e) => setIncludeSources(e.currentTarget.checked)}
            disabled={!hasActiveProject}
          />
          {t("shell:search.includeSources")}
        </label>
      </div>

      <div className={styles.content}>{renderContent()}</div>
    </aside>
  );
}
