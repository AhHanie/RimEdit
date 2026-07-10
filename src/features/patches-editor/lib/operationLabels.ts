import type { PatchOperationMetadata, SchemaCatalog } from "../../schema-catalog/types";
import { BUILT_IN_OPERATION_CLASSES } from "../types/patchFile";

export interface OperationTypeOption {
  className: string;
  label: string;
  description?: string;
  isBuiltIn: boolean;
}

/** Every operation type offered by the "add operation" type picker: the 13 built-in classes
 * (labelled from their shipped metadata when available, falling back to the class name) followed
 * by any additional metadata-defined custom classes from the schema catalog. */
export function listOperationTypeOptions(catalog: SchemaCatalog | null): OperationTypeOption[] {
  const patchOperations: Record<string, PatchOperationMetadata> = catalog?.patchOperations ?? {};
  const seen = new Set<string>();
  const options: OperationTypeOption[] = [];

  for (const className of BUILT_IN_OPERATION_CLASSES) {
    seen.add(className);
    const meta = patchOperations[className];
    options.push({
      className,
      label: meta?.label || className,
      description: meta?.description,
      isBuiltIn: true,
    });
  }

  for (const [className, meta] of Object.entries(patchOperations)) {
    if (seen.has(className)) continue;
    options.push({
      className,
      label: meta.label || className,
      description: meta.description,
      isBuiltIn: false,
    });
  }

  return options;
}

export function filterOperationTypeOptions(
  options: OperationTypeOption[],
  query: string,
): OperationTypeOption[] {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) return options;
  return options.filter(
    (o) =>
      o.className.toLowerCase().includes(trimmed) || o.label.toLowerCase().includes(trimmed),
  );
}
