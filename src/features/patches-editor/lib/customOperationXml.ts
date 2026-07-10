import type { PatchOperationMetadata } from "../../schema-catalog/types";
import type { PatchFile, PatchOperationNode, PatchSuccessMode, XmlAttributeModel } from "../types/patchFile";

const INDENT_UNIT = "  ";

/** Mirrors `patches::serializer::escape_text` exactly (order matters: `&` first). */
export function escapeText(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** Mirrors `patches::serializer::escape_attr` exactly (does not escape `>`, unlike text content). */
export function escapeAttr(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
}

function successXmlStr(success: PatchSuccessMode): string {
  switch (success) {
    case "normal":
      return "Normal";
    case "invert":
      return "Invert";
    case "always":
      return "Always";
    case "never":
      return "Never";
  }
}

/** A resolved value for one metadata-described field, mirroring
 * `patches::custom_metadata::CustomFieldValue`. `xml` values are inserted verbatim (not escaped),
 * matching how `<value>`-shaped fields carry already-serialized XML. */
export type CustomFieldValue = { kind: "text"; value: string } | { kind: "xml"; value: string };

/** Where a newly-created operation will live. The wrapper element name a custom operation's raw
 * XML must use depends on this: `patches::serializer::write_operation` writes an `Unknown` node's
 * `raw_xml` **verbatim**, so a custom operation added inside a sequence must already be tagged
 * `<li>` (not `<Operation>`) in its own text, and one added to a match/nomatch slot must already
 * be tagged `<match>`/`<nomatch>` -- there is no separate re-tagging step at serialize time. */
export type NestedOperationSlot = "top" | "sequenceChild" | "match" | "nomatch";

function tagNameForSlot(slot: NestedOperationSlot): string {
  switch (slot) {
    case "top":
      return "Operation";
    case "sequenceChild":
      return "li";
    case "match":
      return "match";
    case "nomatch":
      return "nomatch";
  }
}

/** Builds a full `<Tag Class="...">...</Tag>` XML block (tag depends on `slot`, see
 * `NestedOperationSlot`) for a metadata-defined (custom or built-in-via-metadata) operation from
 * user-entered field values, replicating `patches::custom_metadata::serialize_custom_operation_fields`
 * plus the operation-wrapper writing `patches::serializer::write_operation` does for built-ins.
 * Kept client-side (rather than a new Tauri command) because `PatchOperationMetadata`/`FieldSchema`
 * are catalog-output-only types on the backend (no `Deserialize`) -- see
 * docs/patches-editor/04-patch-editor-ui.md. */
export function buildCustomOperationXml(
  metadata: PatchOperationMetadata,
  values: Record<string, CustomFieldValue>,
  extraAttributes: XmlAttributeModel[],
  success: PatchSuccessMode,
  slot: NestedOperationSlot = "top",
): string {
  const attributes: XmlAttributeModel[] = [...extraAttributes];
  let body = "";

  for (const fieldName of metadata.fieldOrder) {
    const field = metadata.fields[fieldName];
    const value = values[fieldName];
    if (!field || !value || value.value === "") continue;

    if (field.xml === "attribute") {
      attributes.push({ name: fieldName, value: value.value });
      continue;
    }
    body += INDENT_UNIT;
    body += `<${fieldName}>`;
    body += value.kind === "xml" ? value.value : escapeText(value.value);
    body += `</${fieldName}>\n`;
  }

  let attrText = "";
  for (const attr of attributes) {
    attrText += ` ${attr.name}="${escapeAttr(attr.value)}"`;
  }

  const tag = tagNameForSlot(slot);
  let out = `<${tag} Class="${escapeAttr(metadata.className)}"${attrText}>\n`;
  if (success !== "normal") {
    out += `${INDENT_UNIT}<success>${successXmlStr(success)}</success>\n`;
  }
  out += body;
  out += `</${tag}>`;
  return out;
}

/** Wraps a single `<Operation ...>...</Operation>` (or any raw XML block RimEdit doesn't
 * structurally understand) in a synthetic `<Patch>` root so it can be round-tripped through the
 * `parse_patch_operations` command -- reused instead of adding a dedicated
 * "parse one operation" backend command. */
export function wrapAsPatchFileXml(operationXml: string): string {
  return `<Patch>\n${operationXml}\n</Patch>\n`;
}

/** Wraps an operation fragment (built by `buildCustomOperationXml` with a matching `slot`) so the
 * existing `parse_patch_operations` command -- which only accepts `<Patch>` documents whose direct
 * children are `<Operation>` -- can parse it regardless of the fragment's own tag name. A
 * `sequenceChild`/`match`/`nomatch` fragment is nested inside a synthetic container operation;
 * `extractOperationForSlot` reaches back into the parsed result to pull the actual fragment out.
 * `PatchOperationFindMod`'s `<mods>` field is used as the match/nomatch synthetic container (not
 * `PatchOperationConditional`) only because it makes no other field required. */
export function wrapOperationForSlot(operationXml: string, slot: NestedOperationSlot): string {
  switch (slot) {
    case "top":
      return wrapAsPatchFileXml(operationXml);
    case "sequenceChild":
      return wrapAsPatchFileXml(
        `<Operation Class="PatchOperationSequence">\n<operations>\n${operationXml}\n</operations>\n</Operation>`,
      );
    case "match":
    case "nomatch":
      return wrapAsPatchFileXml(
        `<Operation Class="PatchOperationFindMod">\n<mods>\n</mods>\n${operationXml}\n</Operation>`,
      );
  }
}

/** Inverse of `wrapOperationForSlot`: pulls the actual operation node back out of a `PatchFile`
 * parsed from a slot-wrapped fragment. Returns `null` if the wrapper shape wasn't produced as
 * expected (should not happen for XML this module built itself). */
export function extractOperationForSlot(
  file: PatchFile,
  slot: NestedOperationSlot,
): PatchOperationNode | null {
  const top = file.operations[0];
  if (!top) return null;
  if (slot === "top") return top;
  if (slot === "sequenceChild") {
    return top.kind.type === "sequence" ? (top.kind.data[0] ?? null) : null;
  }
  if (top.kind.type !== "findMod") return null;
  return slot === "match" ? top.kind.data.matchOp : top.kind.data.nomatchOp;
}
