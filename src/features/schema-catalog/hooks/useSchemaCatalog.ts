import { useState, useEffect, useCallback, useMemo } from "react";
import type {
  SchemaCatalog,
  DefTypeSchema,
  FieldSchema,
  PatchOperationMetadata,
  SchemaLoadDiagnostic,
} from "../types";
import { loadSchemaCatalog } from "../api/schemaPack";
import { formatError } from "../../../lib/formatError";

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
): UseSchemaCatalogReturn {
  const [catalog, setCatalog] = useState<SchemaCatalog | null>(null);
  const [diagnostics, setDiagnostics] = useState<SchemaLoadDiagnostic[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await loadSchemaCatalog(extraSchemaRoots, gameVersion);
      setCatalog(result.catalog);
      setDiagnostics(result.diagnostics);
    } catch (e: unknown) {
      setError(formatError(e));
      setCatalog(null);
      setDiagnostics([]);
    } finally {
      setLoading(false);
    }
  }, [extraSchemaRoots, gameVersion]);

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
