import { invoke } from "@tauri-apps/api/core";
import type { SchemaCatalogLoadResult } from "../types";

export function loadSchemaCatalog(
  extraSchemaRoots?: string[],
  gameVersion?: string,
  locale?: string,
): Promise<SchemaCatalogLoadResult> {
  return invoke("load_schema_catalog", { extraSchemaRoots, gameVersion, locale });
}
