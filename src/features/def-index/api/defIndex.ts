import { invoke } from "@tauri-apps/api/core";
import { measureAsync } from "../../../instrumentation";
import type {
  DefDuplicateQueryResult,
  DefIndexFacetSummary,
  DefIndexSummary,
  DefReferenceResolution,
  DefReferenceSuggestion,
  DefXmlPreview,
  IndexedDefSearchResult,
  IndexingStatus,
} from "../types";

export function rebuildDefIndex(projectId?: string): Promise<DefIndexSummary> {
  return measureAsync("defIndex.rebuild", () => invoke("rebuild_def_index", { projectId }));
}

export function queryDefDuplicates(
  projectId: string,
  defType: string,
  defName: string,
): Promise<DefDuplicateQueryResult> {
  return invoke("query_def_duplicates", { projectId, defType, defName });
}

export function getDefIndexFacets(
  projectId: string,
  includeSources?: boolean,
): Promise<DefIndexFacetSummary> {
  return invoke("get_def_index_facets", { projectId, includeSources });
}

export function searchDefs(
  projectId: string,
  query: string,
  defType?: string,
  includeSources?: boolean,
  limit?: number,
): Promise<IndexedDefSearchResult[]> {
  return measureAsync(
    "defIndex.search",
    () => invoke("search_defs", { projectId, query, defType, includeSources, limit }),
    { queryLength: query.length },
  );
}

export function suggestDefReferences(
  projectId: string,
  targetDefTypes: string[],
  query: string,
  scope?: string,
  limit?: number,
): Promise<DefReferenceSuggestion[]> {
  return invoke("suggest_def_references_cmd", { projectId, targetDefTypes, query, scope, limit });
}

export function resolveDefReference(
  projectId: string,
  targetDefTypes: string[],
  defName: string,
  scope?: string,
): Promise<DefReferenceResolution> {
  return invoke("resolve_def_reference_cmd", { projectId, targetDefTypes, defName, scope });
}

export function readIndexedDefXml(
  projectId: string,
  locationId: string,
  relativePath: string,
  defType: string,
  defName: string,
): Promise<DefXmlPreview> {
  return invoke("read_indexed_def_xml", { projectId, locationId, relativePath, defType, defName });
}

export function getIndexingStatus(): Promise<IndexingStatus> {
  return invoke("get_indexing_status");
}

export function startBackgroundIndexing(projectId?: string): Promise<IndexingStatus> {
  return invoke("start_background_indexing", { projectId });
}
