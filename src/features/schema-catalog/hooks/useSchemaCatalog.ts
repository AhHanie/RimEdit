import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import type {
  SchemaCatalog,
  DefTypeSchema,
  FieldSchema,
  PatchOperationMetadata,
  SchemaLoadDiagnostic,
} from "../types";
import { loadSchemaCatalog } from "../api/schemaPack";
import { formatCommandError } from "../../../i18n/diagnostics";

export interface UseSchemaCatalogReturn {
  catalog: SchemaCatalog | null;
  diagnostics: SchemaLoadDiagnostic[];
  loading: boolean;
  error: string | null;
  lookupDefType: (defType: string) => DefTypeSchema | undefined;
  lookupField: (defType: string, fieldName: string) => FieldSchema | undefined;
  lookupPatchOperationMetadata: (className: string) => PatchOperationMetadata | undefined;
  reload: () => Promise<void>;
}

export function useSchemaCatalog(
  extraSchemaRoots?: string[],
  gameVersion?: string,
  locale?: string,
): UseSchemaCatalogReturn {
  const [catalog, setCatalog] = useState<SchemaCatalog | null>(null);
  const [diagnostics, setDiagnostics] = useState<SchemaLoadDiagnostic[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  // Guards against an out-of-order response: a locale switch (or game-version/roots change)
  // fires a new `reload` before a previous in-flight one resolves. Only the response matching
  // the latest request token is ever applied, so a slow stale request can never clobber a
  // faster newer one's result (see issue 06's "Risks" section).
  const requestTokenRef = useRef(0);

  const reload = useCallback(async () => {
    const token = ++requestTokenRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await loadSchemaCatalog(extraSchemaRoots, gameVersion, locale);
      if (requestTokenRef.current !== token) return;
      setCatalog(result.catalog);
      setDiagnostics(result.diagnostics);
    } catch (e: unknown) {
      if (requestTokenRef.current !== token) return;
      setError(formatCommandError(e));
      setCatalog(null);
      setDiagnostics([]);
    } finally {
      if (requestTokenRef.current === token) setLoading(false);
    }
  }, [extraSchemaRoots, gameVersion, locale]);

  useEffect(() => {
    reload();
  }, [reload]);

  const lookupDefType = useCallback(
    (defType: string): DefTypeSchema | undefined => {
      return catalog?.defTypes[defType];
    },
    [catalog],
  );

  const lookupField = useMemo(
    () =>
      (defType: string, fieldName: string): FieldSchema | undefined => {
        if (!catalog) return undefined;
        const visited = new Set<string>();
        const search = (currentType: string): FieldSchema | undefined => {
          if (visited.has(currentType)) return undefined;
          visited.add(currentType);
          const schema = catalog.defTypes[currentType];
          if (!schema) return undefined;
          if (fieldName in schema.fields) return schema.fields[fieldName];
          for (const parent of schema.inherits) {
            const found = search(parent);
            if (found) return found;
          }
          return undefined;
        };
        return search(defType);
      },
    [catalog],
  );

  const lookupPatchOperationMetadata = useCallback(
    (className: string): PatchOperationMetadata | undefined => {
      return catalog?.patchOperations?.[className];
    },
    [catalog],
  );

  return {
    catalog,
    diagnostics,
    loading,
    error,
    lookupDefType,
    lookupField,
    lookupPatchOperationMetadata,
    reload,
  };
}
