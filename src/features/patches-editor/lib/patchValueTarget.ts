import type { FieldSchema, SchemaCatalog } from "../../schema-catalog";
import type { XPathTarget } from "../types/xpathCompletion";

/** Which patch operation kinds can carry a structured `<value>` payload. `"custom"` covers
 * metadata-defined operation fields with `role: "xmlValue"` -- these never resolve to a Def
 * schema field (they aren't xpath-targeted at all), so they always fall back to raw XML; the
 * variant exists only so callers don't have to lie about which built-in kind they mean. */
export type PatchValueOperationType = "add" | "insert" | "replace" | "addModExtension" | "custom";

export interface PatchValueEditTarget {
  fieldName: string;
  field: FieldSchema;
}

/** Fields declared directly on `defType`'s own schema (not walked through `inherits`), in the
 * same "direct only" sense `patches::xpath::direct_field` uses for `resolvedField` -- a field
 * only reachable via the schema's inheritance chain may not physically exist yet in the XML being
 * patched (RimWorld patches apply before XML inheritance), so it isn't offered as an Add/Insert
 * target here either. */
export function listDirectDefTypeFields(
  defType: string,
  catalog: SchemaCatalog | null,
): [string, FieldSchema][] {
  const schema = catalog?.defTypes[defType];
  if (!schema) return [];
  const ordered = [
    ...schema.fieldOrder.filter((name) => name in schema.fields),
    ...Object.keys(schema.fields).filter((name) => !schema.fieldOrder.includes(name)),
  ];
  return ordered.map((name) => [name, schema.fields[name]]);
}

/** `modExtensions` is declared once, on the universal base `Def` schema (see
 * `schema-packs/.../def-types/Def.json`) -- every concrete Def type inherits it in the schema
 * sense, so `patches::xpath`'s "direct field only" resolution (correctly) never resolves it for a
 * concrete type like `ThingDef`. Unlike a genuinely inherited-but-not-yet-present field, though,
 * `modExtensions` is universally a real, structurally valid direct child of any concrete Def XML
 * node -- `PatchOperationAddModExtension` always targets the Def node itself and always means
 * this one well-known field, so it's resolved as a narrow, explicit exception rather than a
 * general inherited-field walk. */
export function resolveModExtensionsField(catalog: SchemaCatalog | null): FieldSchema | null {
  return catalog?.defTypes["Def"]?.fields.modExtensions ?? null;
}

/** The Def type an Add/Insert/AddModExtension operation's xpath resolves to, when it names a
 * concrete Def type or Def (not a deeper field) -- the case where the value editor should offer a
 * field picker instead of a single fixed target. */
export function targetDefType(target: XPathTarget | null): string | null {
  if (!target) return null;
  if (target.kind === "def" || target.kind === "defType") return target.defType;
  return null;
}
