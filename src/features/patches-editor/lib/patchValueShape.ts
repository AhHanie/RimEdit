import {
  buildObjectFieldValue,
  fieldSchemaToControl,
  objectFieldValueToInitialElement,
} from "../../xml-editor";
import type {
  ObjectFieldValue,
  XmlChildView,
  XmlInitialElement,
  XmlNestedChildView,
} from "../../xml-editor";
import type { FieldSchema, SchemaCatalog } from "../../schema-catalog";

/** Control kinds `PatchValueEditor` knows how to render a structured editor for. Everything else
 * (`reference`, `typedReferenceList`, `color`, `flags`, `readonlyUnknown`) falls back to raw XML --
 * these either need UI this feature doesn't have yet (Def reference lookup, color picker) or
 * cannot be round-tripped without more context than a bare `<value>` fragment carries. */
const SUPPORTED_CONTROLS = new Set([
  "text",
  "textarea",
  "number",
  "checkbox",
  "select",
  "list",
  "object",
  "objectList",
  "namedMap",
]);

export function isStructurallySupportedField(field: FieldSchema): boolean {
  const control = fieldSchemaToControl("", field);
  if (control === "objectList") {
    // `objectFieldValueToInitialElement` always serializes an "objectList" value as `<li
    // Class="...">` items -- correct for the plain listOfLi shape, but `keyedObjectList` (keyed
    // child elements, e.g. `<Rain>1</Rain>`) and `keyedObjectMap` (`<li><key>/<value></li>`
    // entries) also classify as "objectList" via `fieldSchemaToControl` and need a different wire
    // shape the shared serializer doesn't produce. Editing those structurally would silently
    // write invalid RimWorld XML, so only the plain listOfLi shape is treated as supported here.
    return field.xml === "listOfLi";
  }
  return SUPPORTED_CONTROLS.has(control);
}

export type ParsedFieldValueResult =
  | { kind: "empty" }
  | { kind: "ok"; value: ObjectFieldValue }
  | { kind: "unsupportedShape"; reason: string }
  | { kind: "mismatch"; actualName: string };

function matchesFieldName(viewName: string, fieldName: string, field: FieldSchema): boolean {
  return viewName === fieldName || (field.xmlAliases ?? []).includes(viewName);
}

/** `<li>`/named-map-entry items with real element structure (attributes or nested children) that
 * `buildObjectFieldValue`'s scalar `list`/`namedMap` handling would read as plain text, silently
 * dropping everything else -- e.g. `<li Class="...">...</li>` mod extension entries when the
 * field's schema doesn't declare an item shape (see `resolveModExtensionsField`'s doc comment). */
function hasNonScalarListItems(view: XmlChildView): boolean {
  return (view.liItems ?? []).some((li) => li.attributes.length > 0 || li.children.length > 0);
}

function hasNonScalarMapEntries(view: XmlChildView): boolean {
  return (view.children ?? []).some(
    (child) => (child.attributes?.length ?? 0) > 0 || (child.children?.length ?? 0) > 0,
  );
}

/** Convert parsed `<value>` child views (from `parsePatchValueXml`) into an `ObjectFieldValue` for
 * the resolved target field, or a reason structured editing isn't possible for this payload.
 * `views` is expected to be either empty (fresh/blank value) or the exact one-element result for
 * `fieldName` -- more than one top-level element, a name that doesn't match the target field (nor
 * one of its XML aliases), or a shape `buildObjectFieldValue` would read lossily are all reported
 * as unsupported rather than silently guessed at, per "Fall back to raw XML for ambiguous or
 * unsupported shapes" in docs/patches-editor/06-structured-patch-value-editor.md. */
export function parsedViewsToFieldValue(
  views: XmlChildView[],
  fieldName: string,
  field: FieldSchema,
  catalog: SchemaCatalog,
): ParsedFieldValueResult {
  if (views.length === 0) return { kind: "empty" };
  if (views.length > 1) {
    return {
      kind: "unsupportedShape",
      reason: "The value contains more than one top-level element.",
    };
  }

  const view = views[0];
  if (!matchesFieldName(view.name, fieldName, field)) {
    return { kind: "mismatch", actualName: view.name };
  }

  const control = fieldSchemaToControl(fieldName, field);
  if (control === "list" && hasNonScalarListItems(view)) {
    return {
      kind: "unsupportedShape",
      reason: "The value's list items have their own attributes or child elements.",
    };
  }
  if (control === "namedMap" && hasNonScalarMapEntries(view)) {
    return {
      kind: "unsupportedShape",
      reason: "The value's entries have their own attributes or child elements.",
    };
  }

  const value = buildObjectFieldValue(
    view as unknown as XmlNestedChildView,
    fieldName,
    field,
    catalog,
    0,
  );
  if (value.kind === "readonly") {
    return { kind: "unsupportedShape", reason: value.reason };
  }
  return { kind: "ok", value };
}

/** A blank starting value for a field with no existing `<value>` content yet, using the same
 * shape dispatch `parsedViewsToFieldValue` uses for existing content. */
export function emptyFieldValue(
  fieldName: string,
  field: FieldSchema,
  catalog: SchemaCatalog,
): ObjectFieldValue {
  return buildObjectFieldValue(undefined, fieldName, field, catalog, 0);
}

/** Build the `XmlInitialElement` fragment to send to `serializePatchValueFragment` for an edited
 * field value. Returns `null` for a value that serializes to nothing (e.g. all-empty scalar) --
 * callers should treat that as an empty `<value>` (`valueXml: null`), not an empty element. */
export function fieldValueToInitialElement(
  fieldXmlName: string,
  value: ObjectFieldValue,
): XmlInitialElement | null {
  return objectFieldValueToInitialElement(fieldXmlName, value);
}
