import { useState, useRef, useEffect, useCallback } from "react";
import { resolveGraphicPreviewAssets } from "../api/xmlDocument";
import type { GraphicPreviewAssetResult, GraphicPreviewVariant } from "../types/graphicPreview";
import { formatError } from "../../../lib/formatError";

export interface GraphicDataPreviewState {
  status: "idle" | "loading" | "ready" | "error";
  result: GraphicPreviewAssetResult | null;
  selectedIndex: number;
  selectedVariant: GraphicPreviewVariant | null;
  warnings: string[];
  error: string | null;
  canGoPrevious: boolean;
  canGoNext: boolean;
  goPrevious: () => void;
  goNext: () => void;
  selectVariant: (index: number) => void;
}

export function useGraphicDataPreview(
  projectId: string | undefined,
  texPath: string,
  graphicClass: string,
  maskPath?: string,
): GraphicDataPreviewState {
  const [status, setStatus] = useState<"idle" | "loading" | "ready" | "error">("idle");
  const [result, setResult] = useState<GraphicPreviewAssetResult | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const seqRef = useRef(0);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const variantsRef = useRef<GraphicPreviewVariant[]>([]);

  const normalizedProjectId = projectId?.trim() ?? "";
  const normalizedTexPath = texPath.trim();
  const normalizedGraphicClass = graphicClass.trim();
  const normalizedMaskPath = maskPath?.trim();

  useEffect(() => {
    setSelectedIndex(0);

    // Always advance the sequence so any in-flight request is invalidated.
    const seq = ++seqRef.current;

    if (!normalizedProjectId || !normalizedTexPath || !normalizedGraphicClass) {
      setStatus("idle");
      setResult(null);
      setError(null);
      if (debounceRef.current) clearTimeout(debounceRef.current);
      return;
    }
    setStatus("loading");

    if (debounceRef.current) clearTimeout(debounceRef.current);

    debounceRef.current = setTimeout(() => {
      void resolveGraphicPreviewAssets(
        normalizedProjectId,
        normalizedTexPath,
        normalizedGraphicClass,
        normalizedMaskPath || undefined,
      )
        .then((res) => {
          if (seq !== seqRef.current) return;
          const firstAvailable = res.variants.findIndex((v) => !v.missing);
          setResult(res);
          setSelectedIndex(firstAvailable >= 0 ? firstAvailable : 0);
          setError(null);
          setStatus("ready");
        })
        .catch((e: unknown) => {
          if (seq !== seqRef.current) return;
          setError(formatError(e));
          setStatus("error");
          setResult(null);
        });
    }, 200);

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [normalizedProjectId, normalizedTexPath, normalizedGraphicClass, normalizedMaskPath]);

  const variants = result?.variants ?? [];
  variantsRef.current = variants;

  const goPrevious = useCallback(() => {
    const len = variantsRef.current.length;
    if (len < 2) return;
    setSelectedIndex((prev) => (prev - 1 + len) % len);
  }, []);

  const goNext = useCallback(() => {
    const len = variantsRef.current.length;
    if (len < 2) return;
    setSelectedIndex((prev) => (prev + 1) % len);
  }, []);

  const selectVariant = useCallback((index: number) => {
    setSelectedIndex(index);
  }, []);

  return {
    status,
    result,
    selectedIndex,
    selectedVariant: variants[selectedIndex] ?? null,
    warnings: result?.warnings ?? [],
    error,
    canGoPrevious: variants.length > 1,
    canGoNext: variants.length > 1,
    goPrevious,
    goNext,
    selectVariant,
  };
}
