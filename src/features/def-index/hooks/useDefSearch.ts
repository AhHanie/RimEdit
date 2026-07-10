import { useState, useEffect } from "react";
import { getDefIndexFacets, searchDefs } from "../api/defIndex";
import type { DefIndexFacetSummary, IndexedDefSearchResult } from "../types";

export interface UseDefSearchReturn {
  query: string;
  setQuery: (q: string) => void;
  selectedDefType: string | undefined;
  setSelectedDefType: (t: string | undefined) => void;
  includeSources: boolean;
  setIncludeSources: (v: boolean) => void;
  loading: boolean;
  results: IndexedDefSearchResult[];
  facets: DefIndexFacetSummary | null;
  selectedPreviewResult: IndexedDefSearchResult | null;
  setSelectedPreviewResult: (r: IndexedDefSearchResult | null) => void;
  error: string | null;
  reloadFacets: () => void;
}

export function useDefSearch(projectId: string | undefined): UseDefSearchReturn {
  const [query, setQuery] = useState("");
  const [selectedDefType, setSelectedDefType] = useState<string | undefined>(undefined);
  const [includeSources, setIncludeSources] = useState(true);
  const [loading, setLoading] = useState(false);
  const [results, setResults] = useState<IndexedDefSearchResult[]>([]);
  const [facets, setFacets] = useState<DefIndexFacetSummary | null>(null);
  const [selectedPreviewResult, setSelectedPreviewResult] =
    useState<IndexedDefSearchResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [facetTick, setFacetTick] = useState(0);
  const [searchTick, setSearchTick] = useState(0);

  const reloadFacets = () => {
    setFacetTick((t) => t + 1);
    setSearchTick((t) => t + 1);
  };

  useEffect(() => {
    if (!projectId) {
      setFacets(null);
      return;
    }
    getDefIndexFacets(projectId, includeSources)
      .then(setFacets)
      .catch((e: unknown) => setError(String(e)));
  }, [projectId, includeSources, facetTick]);

  useEffect(() => {
    if (!projectId) {
      setResults([]);
      return;
    }
    let cancelled = false;
    const id = setTimeout(() => {
      setLoading(true);
      searchDefs(projectId, query, selectedDefType, includeSources, 200)
        .then((r) => {
          if (!cancelled) {
            setResults(r);
            setError(null);
          }
        })
        .catch((e: unknown) => {
          if (!cancelled) setError(String(e));
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    }, 150);
    return () => {
      cancelled = true;
      clearTimeout(id);
    };
  }, [projectId, query, selectedDefType, includeSources, searchTick]);

  return {
    query,
    setQuery,
    selectedDefType,
    setSelectedDefType,
    includeSources,
    setIncludeSources,
    loading,
    results,
    facets,
    selectedPreviewResult,
    setSelectedPreviewResult,
    error,
    reloadFacets,
  };
}
