import type {
  DefEditorView,
  XmlChildView,
  XmlNestedChildView,
} from "../types/xmlDocument";
import type {
  DefTypeSchema,
  FieldSchema,
  ObjectTypeSchema,
  SchemaCatalog,
} from "../../schema-catalog";
import type {
  FormControlKind,
  FormFieldDescriptor,
  FormFieldModel,
  FormFieldPath,
  FormSectionDefaults,
  FormValue,
  TypedReferenceItem,
} from "../types/editorForm";
import {
  fieldSchemaToControl,
  isEditableXmlShape,
  SCALAR_LIKE_CONTROLS,
  getOrderedSchemaFields,
  getAllObjectFields,
  resolveObjectSchema,
  buildObjectListItemValue,
  buildKeyedObjectListItemValue,
  buildKeyedObjectMapItemValue,
} from "./objectDescriptors";
import { initI18n } from "../../../i18n";

/** This module's readonly-value/reason strings below are built by plain module functions, not
 * React components, so there is no `useTranslation()` hook to call -- resolves translated text
 * from the app-wide i18next singleton instead, same as `objectDescriptors.ts` (see that module's
 * top-of-file doc comment for the full rationale, including why `initI18n().t(...)` is called
 * directly at each site rather than through a same-signature local wrapper). */

export function findFieldSchema(
  fieldName: string,
  defTypeSchema: DefTypeSchema,
  catalog: SchemaCatalog,
): FieldSchema | null {
  const visited = new Set<string>();

  function search(typeName: string): FieldSchema | null {
    if (visited.has(typeName)) return null;
    visited.add(typeName);
    const schema = catalog.defTypes[typeName];
    if (!schema) return null;
    if (fieldName in schema.fields) return schema.fields[fieldName];
    for (const f of Object.values(schema.fields)) {
      if (f.xmlAliases?.includes(fieldName)) return f;
    }
    for (const parent of schema.inherits) {
      const found = search(parent);
      if (found) return found;
    }
    return null;
  }

  if (fieldName in defTypeSchema.fields) return defTypeSchema.fields[fieldName];
  for (const f of Object.values(defTypeSchema.fields)) {
    if (f.xmlAliases?.includes(fieldName)) return f;
  }
  for (const parent of defTypeSchema.inherits) {
    const found = search(parent);
    if (found) return found;
  }
  return null;
}

function getAllSchemaFields(
  defSchema: DefTypeSchema,
  catalog: SchemaCatalog,
): Map<string, FieldSchema> {
  const fields = new Map<string, FieldSchema>();
  const visited = new Set<string>();

  function collect(schema: DefTypeSchema) {
    // Ancestors first so inherited base fields precede the concrete type's own fields.
    for (const parent of schema.inherits) {
      if (visited.has(parent)) continue;
      visited.add(parent);
      const parentSchema = catalog.defTypes[parent];
      if (parentSchema) collect(parentSchema);
    }
    for (const [name, fieldSchema] of getOrderedSchemaFields(schema)) {
      if (!fields.has(name)) fields.set(name, fieldSchema);
    }
  }

  collect(defSchema);
  return fields;
}

/**
 * Form Views (issue 06, Plan.md section 5/7): the shared, explicit `knownTopLevel` resolver --
 * every canonical top-level Def schema field id (`DefTypeSchema.fields` key, ancestor-first,
 * first-encountered-wins) that `buildFormDescriptors` would render for this Def type. This is
 * intentionally the exact same traversal `buildFormDescriptors` itself uses (`getAllSchemaFields`
 * above), not a reimplementation, so the Form View resolver's `knownTopLevel` set can never
 * disagree with what the form actually renders (Plan.md section 5: "avoiding a backend/frontend
 * disagreement"). Used to intersect a selected/overridden hidden-field set so a stale or
 * removed field reference can never hide (or fail to hide) anything the form doesn't actually
 * have a field for.
 */
export function collectEffectiveTopLevelDefFields(
  defSchema: DefTypeSchema,
  catalog: SchemaCatalog,
): ReadonlySet<string> {
  return new Set(getAllSchemaFields(defSchema, catalog).keys());
}

/** One row's worth of presentation metadata for the Form View customization checklist
 * (issue 07). */
export interface TopLevelFieldSummary {
  /** Canonical `DefTypeSchema.fields` key -- the same `TopLevelFieldId` a Form View hides. */
  id: string;
  label: string;
  /** True for an object-root field (Plan.md section 8: "gives each object-root a 'section'
   * badge") -- the same expandable-object-shape test `buildFormDescriptors` uses to decide
   * whether to recurse into `buildNestedObjectDescriptors`. */
  isSection: boolean;
  /** The control this field would render as if visible -- used by the checklist UI to pick a
   * friendlier type badge than the raw schema type kind. */
  controlKind: FormControlKind;
  /** Whether the CURRENT XML instance already has data under this field (mirrors
   * `xmlChildHasData`'s "has data" definition, the same one `buildNestedObjectDescriptors` uses
   * to seed a section's default-collapsed state). A cheap presence check, not full validation. */
  hasValue: boolean;
}

/**
 * Form Views (issue 07, Plan.md section 8 step 1): the customization checklist's field list --
 * every canonical top-level Def schema field, in the exact same universe/order as
 * `collectEffectiveTopLevelDefFields` above, each summarized with just enough presentation
 * metadata for a checkbox row. Built directly from the schema + raw XML child/attribute maps,
 * NOT from rendered `FormFieldModel`s -- a hidden field's models never reach the form (issue 05
 * skips their entire descriptor subtree), so a checklist sourced from rendered models could never
 * list (or un-hide) a currently-hidden field. Also deliberately does not expand into nested
 * descriptors or object-list items ("not rendered model descendants" -- issue 07's step 1); only
 * a cheap top-level presence check is needed here.
 */
export function collectTopLevelFieldSummaries(
  def: DefEditorView,
  defSchema: DefTypeSchema,
  catalog: SchemaCatalog,
): TopLevelFieldSummary[] {
  const childByName = new Map(def.children.map((c) => [c.name, c]));
  const attributeByName = new Map(def.attributes.map((a) => [a.name, a]));
  const allFields = getAllSchemaFields(defSchema, catalog);

  const summaries: TopLevelFieldSummary[] = [];
  for (const [fieldName, fieldSchema] of allFields) {
    const isAttribute = fieldSchema.xml === "attribute";
    let child = isAttribute ? undefined : childByName.get(fieldName);
    if (!child && !isAttribute) {
      for (const alias of fieldSchema.xmlAliases ?? []) {
        const aliasedChild = childByName.get(alias);
        if (aliasedChild) {
          child = aliasedChild;
          break;
        }
      }
    }

    const objSchema = getObjectSchemaForField(fieldSchema, catalog);
    const isExpandableShape =
      fieldSchema.xml === "object" || fieldSchema.xml === "element";
    const isSection = !!(objSchema && isExpandableShape);

    const hasValue = isAttribute
      ? !!attributeByName.get(fieldName)?.value
      : xmlChildHasData(child);

    summaries.push({
      id: fieldName,
      label: fieldSchema.label ?? fieldName,
      isSection,
      controlKind: fieldSchemaToControl(fieldName, fieldSchema),
      hasValue,
    });
  }
  return summaries;
}

type AnyXmlChild = XmlChildView | XmlNestedChildView;

function xmlChildHasData(child: AnyXmlChild | undefined): boolean {
  if (!child) return false;
  if (child.textValue !== null && child.textValue !== "") return true;
  if (child.listItems.length > 0) return true;
  if ((child.children ?? []).length > 0) return true;
  if ((child.liObjectItems?.length ?? 0) > 0) return true;
  return false;
}

function getObjectSchemaForField(
  fieldSchema: FieldSchema,
  catalog: SchemaCatalog,
): ObjectTypeSchema | null {
  if (fieldSchema.type.kind !== "object") return null;
  const ref = fieldSchema.type.schemaRef;
  if (!ref) return null;
  return catalog.objectTypes[ref] ?? null;
}

function buildNestedObjectDescriptors(args: {
  objectSchema: ObjectTypeSchema;
  objectSchemaRef: string;
  objectChild: AnyXmlChild | undefined;
  objectPath: string[];
  catalog: SchemaCatalog;
  ancestorRefs: Set<string>;
  sectionChain: FormSectionDefaults[];
}): FormFieldDescriptor[] {
  const {
    objectSchemaRef,
    objectChild,
    objectPath,
    catalog,
    ancestorRefs,
    sectionChain,
  } = args;
  const descriptors: FormFieldDescriptor[] = [];

  const nestedChildByName = new Map(
    (objectChild?.children ?? []).map((c): [string, XmlNestedChildView] => [
      c.name,
      c,
    ]),
  );

  // Use getAllObjectFields to include inherited fields (e.g. Class, selectionWeight from QuestNode base).
  for (const [fieldName, fieldSchema] of getAllObjectFields(
    objectSchemaRef,
    catalog,
  )) {
    const isAttr = fieldSchema.xml === "attribute";
    let nestedChild = isAttr ? undefined : nestedChildByName.get(fieldName);
    let actualFieldName = fieldName;
    if (!nestedChild && !isAttr) {
      for (const alias of fieldSchema.xmlAliases ?? []) {
        nestedChild = nestedChildByName.get(alias);
        if (nestedChild) {
          actualFieldName = alias;
          break;
        }
      }
    }
    const fieldPath = [...objectPath, fieldName];
    const key = fieldPath.join(".");

    // Attribute fields live on the object element itself, not in child elements.
    if (isAttr) {
      const attrValue =
        objectChild?.attributes?.find((a) => a.name === fieldName)?.value ?? "";
      const control = fieldSchemaToControl(fieldName, fieldSchema);
      descriptors.push({
        key,
        label: fieldSchema.label ?? fieldName,
        description: fieldSchema.description,
        control,
        value: attrValue,
        defaultValue: fieldSchema.defaultValue,
        examples: fieldSchema.examples,
        required: fieldSchema.required,
        repeatable: fieldSchema.repeatable,
        xmlShape: fieldSchema.xml,
        fieldPath,
        nodeId: objectChild?.nodeId ?? null,
        dirty: false,
        readonly: false,
        diagnostics: [],
        allowedValues: fieldSchema.validationHints?.allowedValues,
        validationHints: fieldSchema.validationHints,
        reference: fieldSchema.reference,
        sectionDefaults: sectionChain,
        xmlElementName:
          actualFieldName !== fieldName ? actualFieldName : undefined,
      });
      continue;
    }

    const objSchema = getObjectSchemaForField(fieldSchema, catalog);
    const childRef =
      fieldSchema.type.kind === "object"
        ? (fieldSchema.type.schemaRef ?? null)
        : null;
    const isExpandableShape =
      fieldSchema.xml === "object" || fieldSchema.xml === "element";

    if (
      objSchema &&
      isExpandableShape &&
      childRef !== null &&
      !ancestorRefs.has(childRef)
    ) {
      // Resolve discriminator variant (handles known variants, unknown classes, and missing children).
      let resolvedSchema = objSchema;
      let resolvedRef = childRef;
      if (objSchema.discriminator) {
        const discAttrName = objSchema.discriminator.attribute;
        const classValue =
          nestedChild?.attributes?.find((a) => a.name === discAttrName)
            ?.value ?? "";
        const { schemaRef: variantRef, schema: variantSchema } =
          resolveObjectSchema(childRef, classValue, catalog);
        if (variantRef && variantSchema) {
          resolvedSchema = variantSchema;
          resolvedRef = variantRef;
        }
      }

      const childAncestors = new Set(ancestorRefs);
      childAncestors.add(resolvedRef);
      const childChain: FormSectionDefaults[] = [
        ...sectionChain,
        {
          path: fieldPath,
          defaultCollapsed: fieldSchema.defaultCollapsed,
          hasData: xmlChildHasData(nestedChild),
        },
      ];
      descriptors.push(
        ...buildNestedObjectDescriptors({
          objectSchema: resolvedSchema,
          objectSchemaRef: resolvedRef,
          objectChild: nestedChild,
          objectPath: fieldPath,
          catalog,
          ancestorRefs: childAncestors,
          sectionChain: childChain,
        }),
      );
    } else if (objSchema && isExpandableShape) {
      // Cycle detected - show as readonly object placeholder.
      descriptors.push({
        key,
        label: fieldSchema.label ?? fieldName,
        description: fieldSchema.description,
        control: "object",
        value: displayValueForUnsupportedChild(nestedChild),
        examples: [],
        required: fieldSchema.required,
        repeatable: fieldSchema.repeatable,
        xmlShape: fieldSchema.xml,
        fieldPath,
        nodeId: nestedChild?.nodeId ?? null,
        dirty: false,
        readonly: true,
        readOnlyReason: initI18n().t(
          "editor:formFieldControl.circularSchemaReferenceReason",
          "Circular schema reference.",
        ),
        diagnostics: [],
        sectionDefaults: sectionChain,
        xmlElementName:
          actualFieldName !== fieldName ? actualFieldName : undefined,
      });
    } else {
      const control = fieldSchemaToControl(fieldName, fieldSchema);
      const nestedBaseSchemaRef = fieldSchema.items?.schemaRef ?? null;
      const effectiveReference =
        fieldSchema.reference ??
        (fieldSchema.xml === "listOfLi" &&
        fieldSchema.items?.kind === "defReference"
          ? fieldSchema.items?.reference
          : undefined);
      const isEditableNestedObjectList =
        control === "objectList" && nestedBaseSchemaRef !== null;
      const isEditableNested =
        SCALAR_LIKE_CONTROLS.has(control) ||
        control === "list" ||
        control === "flags" ||
        control === "namedMap" ||
        isEditableNestedObjectList;
      const readonly = !isEditableNested;

      let readOnlyReason: string | undefined;

      const value: unknown = nestedChild
        ? control === "list"
          ? nestedChild.listItems
          : control === "flags"
            ? fieldSchema.xml === "flagsText"
              ? (nestedChild.textValue ?? "")
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean)
              : nestedChild.listItems
            : control === "namedMap"
              ? (nestedChild.children ?? []).map((c) => ({
                  key: c.name,
                  value: c.textValue ?? "",
                }))
              : fieldSchema.xml === "keyedObjectMap" && control === "objectList" && nestedBaseSchemaRef
              ? {
                  kind: "objectList" as const,
                  items: (nestedChild.liItems ?? []).map((item) =>
                    buildKeyedObjectMapItemValue(item, nestedBaseSchemaRef, catalog),
                  ),
                }
              : control === "objectList" && nestedBaseSchemaRef
                ? {
                    kind: "objectList" as const,
                    items: (nestedChild.liItems ?? []).map((item) =>
                      buildObjectListItemValue(
                        item,
                        nestedBaseSchemaRef,
                        catalog,
                      ),
                    ),
                  }
                : control === "objectList"
                  ? objectListSummary(nestedChild.liObjectItems?.length ?? 0)
                  : (nestedChild.textValue ?? "")
        : control === "list" || control === "flags"
          ? []
          : control === "namedMap"
            ? []
            : control === "objectList" && nestedBaseSchemaRef
              ? { kind: "objectList" as const, items: [] }
              : control === "objectList"
                ? objectListSummary(0)
                : "";

      descriptors.push({
        key,
        label: fieldSchema.label ?? fieldName,
        description: fieldSchema.description,
        control,
        value,
        defaultValue: fieldSchema.defaultValue,
        examples: fieldSchema.examples,
        required: fieldSchema.required,
        repeatable: fieldSchema.repeatable,
        xmlShape: fieldSchema.xml,
        fieldPath,
        nodeId: nestedChild?.nodeId ?? null,
        dirty: false,
        readonly,
        readOnlyReason,
        diagnostics: [],
        allowedValues: fieldSchema.validationHints?.allowedValues,
        validationHints: fieldSchema.validationHints,
        reference: effectiveReference,
        itemSchemaRef: isEditableNestedObjectList
          ? nestedBaseSchemaRef
          : undefined,
        sectionDefaults: sectionChain,
        xmlElementName:
          actualFieldName !== fieldName ? actualFieldName : undefined,
      });
    }
  }

  return descriptors;
}

export function buildFormDescriptors(
  def: DefEditorView,
  defSchema: DefTypeSchema | null,
  catalog: SchemaCatalog,
  visibleTopLevelFieldIds?: ReadonlySet<string> | null,
): FormFieldDescriptor[] {
  const descriptors: FormFieldDescriptor[] = [];
  const handledKeys = new Set<string>();

  // First: schema fields in schema-defined order (including inherited), present or absent.
  if (defSchema) {
    const childByName = new Map(def.children.map((c) => [c.name, c]));
    const attributeByName = new Map(def.attributes.map((a) => [a.name, a]));
    const allFields = getAllSchemaFields(defSchema, catalog);

    for (const [fieldName, fieldSchema] of allFields) {
      // `fieldName` here is already the canonical top-level Def schema field key (the same
      // key used by `DefTypeSchema.fields`/`fieldOrder`/`lookup_field`, and equal to what
      // becomes `descriptor.fieldPath[0]` below) - the exact TopLevelFieldId identity Form
      // Views (Plan.md section 7) filter on. Mark it (and any matched alias) handled before
      // the visibility check so a hidden field's XML still counts as "known" and never falls
      // through to `UnknownXmlFields` below.
      handledKeys.add(fieldName);
      const isAttribute = fieldSchema.xml === "attribute";
      let child = isAttribute ? undefined : childByName.get(fieldName);
      let actualChildName = fieldName;
      if (!child && !isAttribute) {
        for (const alias of fieldSchema.xmlAliases ?? []) {
          const aliasedChild = childByName.get(alias);
          if (aliasedChild) {
            child = aliasedChild;
            actualChildName = alias;
            handledKeys.add(alias);
            break;
          }
        }
      }
      const attribute = isAttribute
        ? attributeByName.get(fieldName)
        : undefined;

      // Form View visibility filter (issue 05, Plan.md section 7/10): skip this top-level
      // field entirely - before any nested-object-schema resolution, discriminator
      // resolution, `buildNestedObjectDescriptors` recursion, or object-list item value
      // construction (`buildObjectListItemValue` et al.) - so a hidden root never pays for
      // the expensive expansion it would otherwise trigger below. `undefined`/`null` means
      // "no filter", preserving today's full-form behavior exactly.
      if (visibleTopLevelFieldIds && !visibleTopLevelFieldIds.has(fieldName)) {
        continue;
      }

      // Schema-backed object field with an expandable xml shape - expand to nested descriptors.
      // listOfLi, namedChildrenMap, and keyedValueList containers are not expanded.
      const objSchema = getObjectSchemaForField(fieldSchema, catalog);
      const isExpandableShape =
        fieldSchema.xml === "object" || fieldSchema.xml === "element";
      if (objSchema && isExpandableShape) {
        const baseSchemaRef =
          fieldSchema.type.kind === "object"
            ? (fieldSchema.type.schemaRef ?? "")
            : "";

        // Resolve discriminator variant (handles known variants, unknown classes, and missing children).
        let resolvedSchemaRef = baseSchemaRef;
        let resolvedObjSchema = objSchema;
        if (objSchema.discriminator) {
          const discAttrName = objSchema.discriminator.attribute;
          const classValue =
            child?.attributes?.find((a) => a.name === discAttrName)?.value ??
            "";
          const { schemaRef: variantRef, schema: variantSchema } =
            resolveObjectSchema(baseSchemaRef, classValue, catalog);
          if (variantRef && variantSchema) {
            resolvedSchemaRef = variantRef;
            resolvedObjSchema = variantSchema;
          }
        }

        descriptors.push(
          ...buildNestedObjectDescriptors({
            objectSchema: resolvedObjSchema,
            objectSchemaRef: resolvedSchemaRef,
            objectChild: child,
            objectPath: [fieldName],
            catalog,
            ancestorRefs: new Set([resolvedSchemaRef]),
            sectionChain: [
              {
                path: [fieldName],
                defaultCollapsed: fieldSchema.defaultCollapsed,
                hasData: xmlChildHasData(child),
              },
            ],
          }),
        );
        continue;
      }

      const control = fieldSchemaToControl(fieldName, fieldSchema);
      const effectiveReference =
        fieldSchema.reference ??
        (fieldSchema.xml === "listOfLi" &&
        fieldSchema.items?.kind === "defReference"
          ? fieldSchema.items?.reference
          : undefined);
      const isReferenceList =
        fieldSchema.xml === "listOfLi" && !!effectiveReference;
      const baseSchemaRef = fieldSchema.items?.schemaRef ?? null;
      const isEditableObjectListField =
        control === "objectList" && baseSchemaRef !== null;
      const isKeyedObjectList = fieldSchema.xml === "keyedObjectList";
      const isEditableList =
        (fieldSchema.xml === "listOfLi" &&
          (control === "list" || control === "flags")) ||
        isKeyedObjectList ||
        control === "namedMap" ||
        control === "typedReferenceList" ||
        isEditableObjectListField;

      const value: unknown = child
        ? control === "list"
          ? child.listItems
          : control === "flags"
            ? fieldSchema.xml === "flagsText"
              ? (child.textValue ?? "")
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean)
              : child.listItems
            : control === "namedMap"
              ? (child.children ?? []).map((c) => ({
                  key: c.name,
                  value: c.textValue ?? "",
                }))
              : control === "typedReferenceList"
                ? {
                    kind: "typedReferenceList" as const,
                    items: (child.children ?? []).map(
                      (c): TypedReferenceItem => ({
                        nodeId: c.nodeId,
                        defType: c.name,
                        defName: c.textValue ?? "",
                      }),
                    ),
                  }
                : fieldSchema.xml === "keyedObjectMap" &&
                    control === "objectList" &&
                    baseSchemaRef
                  ? {
                      kind: "objectList" as const,
                      items: (child.liItems ?? []).map((item) =>
                        buildKeyedObjectMapItemValue(item, baseSchemaRef, catalog),
                      ),
                    }
                  : isKeyedObjectList &&
                    control === "objectList" &&
                    baseSchemaRef &&
                    fieldSchema.keyField
                  ? {
                      kind: "objectList" as const,
                      items: (child.children ?? []).map((c) =>
                        buildKeyedObjectListItemValue(
                          c,
                          fieldSchema.keyField!,
                          fieldSchema.defaultValueField,
                          baseSchemaRef,
                          catalog,
                        ),
                      ),
                    }
                  : control === "objectList" && baseSchemaRef
                    ? {
                        kind: "objectList" as const,
                        items: (child.liItems ?? []).map((item) =>
                          buildObjectListItemValue(
                            item,
                            baseSchemaRef,
                            catalog,
                          ),
                        ),
                      }
                    : control === "objectList"
                      ? objectListSummary(
                          child.liObjectItems?.length ??
                            child.children?.length ??
                            0,
                        )
                      : (child.textValue ??
                        (SCALAR_LIKE_CONTROLS.has(control) &&
                        child.xmlShape === "element"
                          ? ""
                          : displayValueForUnsupportedChild(child)))
        : attribute
          ? attribute.value
          : control === "list" || control === "flags"
            ? []
            : control === "namedMap"
              ? []
              : control === "typedReferenceList"
                ? { kind: "typedReferenceList" as const, items: [] }
                : control === "objectList" && baseSchemaRef
                  ? { kind: "objectList" as const, items: [] }
                  : control === "objectList"
                    ? objectListSummary(0)
                    : "";

      descriptors.push({
        key: fieldName,
        label: fieldSchema.label ?? fieldName,
        description: fieldSchema.description,
        control,
        value,
        defaultValue: fieldSchema.defaultValue,
        examples: fieldSchema.examples,
        required: fieldSchema.required,
        repeatable: fieldSchema.repeatable,
        xmlShape: fieldSchema.xml,
        fieldPath: [fieldName],
        nodeId: isAttribute ? def.nodeId : (child?.nodeId ?? null),
        dirty: false,
        readonly:
          !isEditableXmlShape(fieldSchema) &&
          !isReferenceList &&
          !isEditableList,
        diagnostics: [],
        allowedValues: fieldSchema.validationHints?.allowedValues,
        validationHints: fieldSchema.validationHints,
        reference: effectiveReference,
        keyReference: fieldSchema.keyReference,
        typedReference: fieldSchema.typedReference,
        itemSchemaRef: isEditableObjectListField
          ? (baseSchemaRef ?? undefined)
          : undefined,
        keyField: isKeyedObjectList
          ? (fieldSchema.keyField ?? undefined)
          : undefined,
        defaultValueField: isKeyedObjectList
          ? (fieldSchema.defaultValueField ?? undefined)
          : undefined,
        sectionDefaults: [],
        xmlElementName:
          actualChildName !== fieldName ? actualChildName : undefined,
      });
    }
  }

  // Second: XML children with no schema entry - rendered read-only.
  for (const child of def.children) {
    if (!handledKeys.has(child.name)) {
      descriptors.push({
        key: child.name,
        label: child.name,
        control: "readonlyUnknown",
        value: child.textValue ?? displayValueForUnsupportedChild(child),
        examples: [],
        required: false,
        repeatable: false,
        xmlShape: child.xmlShape,
        fieldPath: [child.name],
        nodeId: child.nodeId,
        dirty: false,
        readonly: true,
        diagnostics: [],
        sectionDefaults: [],
      });
    }
  }

  return descriptors;
}

function objectListSummary(count: number): string {
  return initI18n().t(
    "editor:objectListEditor.objectListItemCountSummary",
    `(object list: ${count} item${count === 1 ? "" : "s"})`,
    { count },
  );
}

function displayValueForUnsupportedChild(
  child: Pick<AnyXmlChild, "listItems" | "xmlShape"> | undefined,
): string {
  if (!child) return "";
  if (child.listItems.length > 0) {
    const count = child.listItems.length;
    return initI18n().t(
      "editor:objectListEditor.unsupportedChildSummaryWithCount",
      `(${child.xmlShape}, ${count} item${count === 1 ? "" : "s"})`,
      { xmlShape: child.xmlShape, count },
    );
  }
  return initI18n().t(
    "editor:objectListEditor.unsupportedChildSummary",
    `(${child.xmlShape})`,
    { xmlShape: child.xmlShape },
  );
}

export function buildFormFieldModels(
  def: DefEditorView,
  defSchema: DefTypeSchema | null,
  catalog: SchemaCatalog,
  visibleTopLevelFieldIds?: ReadonlySet<string> | null,
): FormFieldModel[] {
  return buildFormDescriptors(
    def,
    defSchema,
    catalog,
    visibleTopLevelFieldIds,
  ).map(
    (descriptor, index) => {
      const isUnknown = descriptor.control === "readonlyUnknown";
      const isAttribute = descriptor.xmlShape === "attribute";
      const isNested = descriptor.fieldPath.length > 1;
      const isReferenceList =
        descriptor.xmlShape === "listOfLi" && !!descriptor.reference;
      const isEditableObjectList =
        descriptor.control === "objectList" &&
        typeof descriptor.value === "object" &&
        descriptor.value !== null &&
        "kind" in (descriptor.value as object) &&
        (descriptor.value as { kind: string }).kind === "objectList";
      const isEditableList =
        descriptor.control === "list" ||
        descriptor.control === "flags" ||
        descriptor.control === "namedMap" ||
        descriptor.control === "typedReferenceList" ||
        isEditableObjectList;

      const isUnsupportedStructured =
        (!isReferenceList &&
          !isEditableList &&
          descriptor.xmlShape === "listOfLi") ||
        (descriptor.control !== "namedMap" &&
          (descriptor.xmlShape === "namedChildrenMap" ||
            descriptor.xmlShape === "keyedValueList")) ||
        descriptor.xmlShape === "object";

      let path: FormFieldPath;
      const lastSegment = descriptor.fieldPath[descriptor.fieldPath.length - 1];
      const objectPath = descriptor.fieldPath.slice(0, -1);
      // Use the actual XML element name (alias) when the field was matched via xmlAliases.
      const xmlLeafName = descriptor.xmlElementName ?? lastSegment;
      const xmlTopName = descriptor.xmlElementName ?? descriptor.key;

      if (descriptor.control === "typedReferenceList") {
        path = {
          kind: "typedReferenceList",
          objectPath: isNested ? objectPath : [],
          fieldName: lastSegment,
        };
      } else if (
        isNested &&
        (descriptor.control === "list" ||
          (descriptor.control === "flags" &&
            descriptor.xmlShape === "listOfLi"))
      ) {
        path = { kind: "nestedListItems", objectPath, fieldName: xmlLeafName };
      } else if (descriptor.control === "namedMap") {
        path = {
          kind: "namedMap",
          objectPath: isNested ? objectPath : [],
          mapName: lastSegment,
        };
      } else if (descriptor.control === "objectList") {
        path = {
          kind: "objectList",
          objectPath: isNested ? objectPath : [],
          fieldName: lastSegment,
        };
      } else if (isNested && isAttribute) {
        path = {
          kind: "nestedAttribute",
          objectPath,
          attributeName: lastSegment,
        };
      } else if (isNested) {
        path = {
          kind: "nestedObjectField",
          objectPath,
          fieldName: xmlLeafName,
        };
      } else if (isAttribute) {
        path = { kind: "attribute", attributeName: descriptor.key };
      } else if (isUnknown) {
        path = {
          kind: "unknownChild",
          childName: descriptor.key,
          nodeId: descriptor.nodeId ?? -1,
        };
      } else if (
        (descriptor.control === "list" || descriptor.control === "flags") &&
        descriptor.xmlShape === "listOfLi"
      ) {
        path = { kind: "listItems", childName: xmlTopName };
      } else {
        path = { kind: "childElement", childName: xmlTopName };
      }

      const readonly =
        descriptor.readonly ||
        descriptor.control === "object" ||
        isUnsupportedStructured;
      const readOnlyReason =
        descriptor.readOnlyReason ??
        (descriptor.control === "object" || isUnsupportedStructured
          ? initI18n().t(
              "editor:formFieldControl.structuredFieldReadOnlyReason",
              "Use Raw XML mode to edit this structured field.",
            )
          : descriptor.readonly
            ? initI18n().t(
                "editor:formFieldControl.noSchemaReadOnlyReason",
                "This field is not described by the active schema pack.",
              )
            : undefined);

      return {
        id: makeFormFieldId(
          def,
          descriptor.fieldPath,
          descriptor.control,
          descriptor.nodeId,
        ),
        key: descriptor.key,
        label: descriptor.label,
        description: descriptor.description,
        control: descriptor.control,
        defaultValue: descriptor.defaultValue,
        examples: descriptor.examples,
        required: descriptor.required,
        repeatable: descriptor.repeatable,
        xmlShape: descriptor.xmlShape,
        path,
        fieldPath: descriptor.fieldPath,
        sourceNodeId: descriptor.nodeId,
        defNodeId: def.nodeId,
        order: index,
        readonly,
        readOnlyReason,
        diagnostics: descriptor.diagnostics,
        allowedValues: descriptor.allowedValues,
        validationHints: descriptor.validationHints,
        reference: descriptor.reference,
        keyReference: descriptor.keyReference,
        typedReference: descriptor.typedReference,
        itemSchemaRef: descriptor.itemSchemaRef,
        keyField: descriptor.keyField,
        defaultValueField: descriptor.defaultValueField,
        sectionDefaults: descriptor.sectionDefaults,
      };
    },
  );
}

export function formValueFromModel(
  model: FormFieldModel,
  value: unknown,
): FormValue {
  if (model.control === "typedReferenceList") {
    if (
      value &&
      typeof value === "object" &&
      "kind" in (value as object) &&
      (value as { kind: string }).kind === "typedReferenceList"
    ) {
      return value as FormValue;
    }
    return { kind: "typedReferenceList", items: [] };
  }
  if (model.control === "objectList") {
    if (
      value &&
      typeof value === "object" &&
      "kind" in (value as object) &&
      (value as { kind: string }).kind === "objectList"
    ) {
      return value as FormValue;
    }
    return { kind: "objectList", items: [] };
  }
  if (model.control === "list") {
    return {
      kind: "list",
      items: Array.isArray(value) ? [...(value as string[])] : [],
    };
  }
  if (model.control === "flags") {
    const items = Array.isArray(value) ? [...(value as string[])] : [];
    const allowedValues = model.allowedValues ?? [];
    const selected = items.filter((v) => allowedValues.includes(v));
    const custom = items.filter((v) => !allowedValues.includes(v));
    return { kind: "flags", selected, custom };
  }
  if (model.control === "namedMap") {
    const entries = Array.isArray(value)
      ? (value as unknown[]).filter(
          (e): e is { key: string; value: string } =>
            !!e && typeof e === "object" && "key" in e && "value" in e,
        )
      : [];
    return {
      kind: "namedMap",
      entries: entries.map((e) => ({ key: e.key, value: e.value })),
    };
  }
  if (model.control === "checkbox") {
    const stringValue = typeof value === "string" ? value : String(value ?? "");
    return {
      kind: "boolean",
      value:
        stringValue === "true" || stringValue === "True" || stringValue === "1",
    };
  }
  if (model.control === "select") {
    return {
      kind: "enum",
      value: typeof value === "string" ? value : String(value ?? ""),
    };
  }
  if (model.readonly) {
    return {
      kind: "readonly",
      value: typeof value === "string" ? value : String(value ?? ""),
    };
  }
  return {
    kind: "scalar",
    value: typeof value === "string" ? value : String(value ?? ""),
  };
}

export function formValueFromDescriptor(
  descriptor: FormFieldDescriptor,
): FormValue {
  const isNested = descriptor.fieldPath.length > 1;
  const lastSegment = descriptor.fieldPath[descriptor.fieldPath.length - 1];
  const objectPath = descriptor.fieldPath.slice(0, -1);

  let path: FormFieldPath;
  if (
    isNested &&
    (descriptor.control === "list" || descriptor.control === "flags")
  ) {
    path = { kind: "nestedListItems", objectPath, fieldName: lastSegment };
  } else if (descriptor.control === "namedMap") {
    path = {
      kind: "namedMap",
      objectPath: isNested ? objectPath : [],
      mapName: lastSegment,
    };
  } else if (descriptor.control === "objectList") {
    path = {
      kind: "objectList",
      objectPath: isNested ? objectPath : [],
      fieldName: lastSegment,
    };
  } else if (isNested && descriptor.xmlShape === "attribute") {
    path = { kind: "nestedAttribute", objectPath, attributeName: lastSegment };
  } else if (isNested) {
    path = { kind: "nestedObjectField", objectPath, fieldName: lastSegment };
  } else if (descriptor.control === "list" || descriptor.control === "flags") {
    path = { kind: "listItems", childName: descriptor.key };
  } else {
    path = { kind: "childElement", childName: descriptor.key };
  }

  const model: FormFieldModel = {
    id: descriptor.key,
    key: descriptor.key,
    label: descriptor.label,
    description: descriptor.description,
    control: descriptor.control,
    defaultValue: descriptor.defaultValue,
    examples: descriptor.examples,
    required: descriptor.required,
    repeatable: descriptor.repeatable,
    xmlShape: descriptor.xmlShape,
    path,
    fieldPath: descriptor.fieldPath,
    sourceNodeId: descriptor.nodeId,
    defNodeId: 0,
    order: 0,
    readonly: descriptor.readonly,
    diagnostics: descriptor.diagnostics,
    allowedValues: descriptor.allowedValues,
    validationHints: undefined,
    reference: descriptor.reference,
    keyReference: descriptor.keyReference,
    defaultValueField: descriptor.defaultValueField,
    sectionDefaults: descriptor.sectionDefaults,
  };
  return formValueFromModel(model, descriptor.value);
}

function makeFormFieldId(
  def: DefEditorView,
  fieldPath: string[],
  control: FormControlKind,
  nodeId: number | null,
): string {
  const defIdentity = `${def.defType}:${def.defName ?? def.nodeId}`;
  const pathIdentity = fieldPath.join(".");
  const sourceIdentity =
    control === "readonlyUnknown" ? (nodeId ?? pathIdentity) : pathIdentity;
  return `${defIdentity}:${control}:${sourceIdentity}`;
}
