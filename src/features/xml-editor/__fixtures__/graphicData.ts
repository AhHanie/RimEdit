import type { SchemaCatalog, FieldSchema } from "../../schema-catalog";
import type { DefEditorView, XmlChildView } from "../types/xmlDocument";
import type { FormFieldState, FormFieldPath, FormValue } from "../types/editorForm";

import type { GraphicPreviewAssetResult, GraphicPreviewVariant } from "../types/graphicPreview";

function makeFieldSchema(overrides: Partial<FieldSchema> & Pick<FieldSchema, "type" | "xml">): FieldSchema {
  return {
    required: false,
    repeatable: false,
    examples: [],
    flags: false,
    ...overrides,
  };
}

export function makeNestedVisualCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {
      TestDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: ["defName", "visualData"],
        fields: {
          defName: makeFieldSchema({ type: { kind: "string" }, xml: "element" }),
          visualData: makeFieldSchema({ type: { kind: "object", schemaRef: "NestedVisual" }, xml: "object" }),
        },
      },
    },
    objectTypes: {
      NestedVisual: {
        fieldOrder: ["imagePath", "renderStyle", "shadowSection", "damageSection", "linkMode", "linkFlags", "shaderParams", "attachments", "attachPoints"],
        fields: {
          imagePath: makeFieldSchema({ type: { kind: "string" }, xml: "element" }),
          renderStyle: makeFieldSchema({
            type: { kind: "enum" },
            xml: "element",
            validationHints: {
              allowedValues: [
                "Single", "Multi", "Random", "StackCount",
                "RandomWithAge", "Linked", "LinkedAsymmetric",
              ],
            },
          }),
          shadowSection: makeFieldSchema({ type: { kind: "object", schemaRef: "ShadowSection" }, xml: "object" }),
          damageSection: makeFieldSchema({ type: { kind: "object", schemaRef: "DamageSection" }, xml: "object" }),
          linkMode: makeFieldSchema({
            type: { kind: "enum" },
            xml: "element",
            validationHints: {
              allowedValues: ["None", "Basic", "CornerFiller", "CornerOverlay", "Transmitter", "TransmitterOverlay", "Asymmetric"],
            },
          }),
          linkFlags: makeFieldSchema({
            type: { kind: "list" },
            xml: "listOfLi",
            items: { kind: "enum" },
            flags: true,
            validationHints: { allowedValues: ["None", "Wall", "Rock", "Conduit"] },
          }),
          shaderParams: makeFieldSchema({ type: { kind: "object" }, xml: "namedChildrenMap" }),
          attachments: makeFieldSchema({
            type: { kind: "list" },
            xml: "listOfLi",
            items: { kind: "object", schemaRef: "NestedVisual" },
          }),
          attachPoints: makeFieldSchema({
            type: { kind: "list" },
            xml: "listOfLi",
            items: { kind: "object", schemaRef: "AttachPoint" },
          }),
        },
      },
      ShadowSection: {
        fieldOrder: ["volume", "offset"],
        fields: {
          volume: makeFieldSchema({ type: { kind: "vector3" }, xml: "element" }),
          offset: makeFieldSchema({ type: { kind: "vector3" }, xml: "element" }),
        },
      },
      DamageSection: {
        fieldOrder: ["enabled", "scratches"],
        fields: {
          enabled: makeFieldSchema({ type: { kind: "boolean" }, xml: "element" }),
          scratches: makeFieldSchema({ type: { kind: "list" }, xml: "listOfLi", items: { kind: "string" } }),
        },
      },
      AttachPoint: {
        fieldOrder: ["offset", "type"],
        fields: {
          offset: makeFieldSchema({ type: { kind: "vector3" }, xml: "element" }),
          type: makeFieldSchema({
            type: { kind: "enum" },
            xml: "element",
            validationHints: { allowedValues: ["RootNone", "Connection0", "Restraint0", "Restraint3", "Exhaust"] },
          }),
        },
      },
    },
  };
}

function makeXmlChild(
  name: string,
  nodeId: number,
  textValue: string | null = null,
  overrides: Partial<XmlChildView> = {},
): XmlChildView {
  return {
    nodeId,
    name,
    textValue,
    listItems: [],
    xmlShape: "element",
    order: nodeId,
    known: true,
    line: null,
    column: null,
    ...overrides,
  };
}

export function makeNestedVisualDefView(overrides: Partial<DefEditorView> = {}): DefEditorView {
  return {
    nodeId: 1,
    defType: "TestDef",
    defName: "FixtureVisualSingle",
    label: null,
    parentName: null,
    line: null,
    column: null,
    attributes: [],
    children: [
      makeXmlChild("defName", 2, "FixtureVisualSingle"),
      {
        nodeId: 3,
        name: "visualData",
        textValue: null,
        listItems: [],
        xmlShape: "object",
        order: 3,
        known: true,
        line: null,
        column: null,
        children: [
          {
            nodeId: 4,
            name: "imagePath",
            textValue: "Things/Fixture/Single/FixtureSingle",
            listItems: [],
            xmlShape: "element",
            order: 4,
            line: null,
            column: null,
          },
          {
            nodeId: 5,
            name: "renderStyle",
            textValue: "Single",
            listItems: [],
            xmlShape: "element",
            order: 5,
            line: null,
            column: null,
          },
        ],
      },
    ],
    ...overrides,
  };
}

// The following functions are used by graphic preview and form editor tests
// that are not in scope for renaming. They remain under their original names.

function makeScalarValue(value: string): FormValue {
  return { kind: "scalar", value };
}

export function makeGraphicDataFormState(overrides: Partial<FormFieldState> = {}): FormFieldState {
  const path: FormFieldPath = { kind: "nestedObjectField", objectPath: ["graphicData"], fieldName: "texPath" };
  const value = makeScalarValue("Things/Fixture/Single/FixtureSingle");
  return {
    model: {
      id: "graphicData.texPath",
      key: "graphicData.texPath",
      label: "Tex Path",
      control: "text",
      path,
      fieldPath: ["graphicData", "texPath"],
      defNodeId: 1,
      sourceNodeId: 4,
      order: 0,
      readonly: false,
      required: false,
      repeatable: false,
      xmlShape: "element",
      examples: [],
      diagnostics: [],
      sectionDefaults: [],
    },
    value,
    initialValue: value,
    dirty: false,
    touched: false,
    focused: false,
    pending: false,
    error: null,
    validationErrors: [],
    clearRequested: false,
    ...overrides,
  };
}

export function makeGraphicPreviewVariant(overrides: Partial<GraphicPreviewVariant> = {}): GraphicPreviewVariant {
  return {
    id: "v1",
    label: { kind: "single" },
    role: "single",
    sourceLocationId: "fixture-proj",
    sourceLocationName: "Project Mod",
    relativeTexturePath: "Things/Fixture/Single/FixtureSingle.png",
    assetUrl: "rimedit-asset://localhost/fixture-token-1",
    ...overrides,
  };
}

export function makeGraphicPreviewResult(overrides: Partial<GraphicPreviewAssetResult> = {}): GraphicPreviewAssetResult {
  return {
    texPath: "Things/Fixture/Single/FixtureSingle",
    graphicClass: "Graphic_Single",
    variants: [makeGraphicPreviewVariant()],
    warnings: [],
    ...overrides,
  };
}
