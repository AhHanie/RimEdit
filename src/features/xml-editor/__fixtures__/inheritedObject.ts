import type { SchemaCatalog } from "../../schema-catalog";
import type { XmlEditorSnapshot } from "../types/editorSession";

export function makeInheritedObjectCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {
      DerivedDef: {
        inherits: ["BaseDef"],
        abstractType: false,
        fieldOrder: [],
        fields: {},
      },
      BaseDef: {
        inherits: [],
        abstractType: true,
        fieldOrder: [
          "layerMode",
          "blockedLayers",
          "difficultyConfig",
          "variantItems",
          "iconVariants",
        ],
        fields: {
          layerMode: {
            type: { kind: "enum" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
            validationHints: { allowedValues: ["High", "Medium", "Low"] },
          },
          blockedLayers: {
            type: { kind: "list" },
            required: false,
            repeatable: false,
            xml: "listOfLi",
            examples: [],
            flags: false,
            items: { kind: "enum" },
            validationHints: { allowedValues: ["High", "Medium"] },
          },
          difficultyConfig: {
            type: { kind: "object", schemaRef: "DifficultyConfig" },
            required: false,
            repeatable: false,
            xml: "object",
            examples: [],
            flags: false,
          },
          variantItems: {
            type: { kind: "list" },
            required: false,
            repeatable: false,
            xml: "listOfLi",
            examples: [],
            flags: false,
            items: { kind: "object", schemaRef: "VariantItem" },
          },
          iconVariants: {
            type: { kind: "list" },
            required: false,
            repeatable: false,
            xml: "listOfLi",
            examples: [],
            flags: false,
            items: { kind: "object", schemaRef: "IconVariant" },
          },
        },
      },
    },
    objectTypes: {
      DifficultyConfig: {
        fieldOrder: ["difficultyVar", "costList", "costStuffCount", "invert"],
        fields: {
          difficultyVar: {
            type: { kind: "string" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
          },
          costList: {
            type: { kind: "object" },
            required: false,
            repeatable: false,
            xml: "namedChildrenMap",
            examples: [],
            flags: false,
          },
          costStuffCount: {
            type: { kind: "integer" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
          },
          invert: {
            type: { kind: "boolean" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
          },
        },
      },
      VariantItem: {
        fieldOrder: ["source", "color"],
        fields: {
          source: {
            type: { kind: "defReference" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
            reference: { defType: "ThingDef", allowAbstract: false, scope: "allSources" },
          },
          color: {
            type: { kind: "string" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
          },
        },
      },
      IconVariant: {
        fieldOrder: ["appearance", "iconPath"],
        fields: {
          appearance: {
            type: { kind: "defReference" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
            reference: { defType: "AppearanceDef", allowAbstract: false, scope: "allSources" },
          },
          iconPath: {
            type: { kind: "string" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
          },
        },
      },
    },
  };
}

export function makeInheritedObjectSnapshot(): XmlEditorSnapshot {
  return {
    rawXml: "<Defs><DerivedDef><layerMode>High</layerMode></DerivedDef></Defs>",
    parseDiagnostics: [],
    validationDiagnostics: [],
    selectedDefNodeId: 1,
    parsed: {
      nodeCount: 3,
      rootElement: "Defs",
      profile: "defs",
      about: null,
      defs: [
        {
          nodeId: 1,
          defType: "DerivedDef",
          defName: "TestInstance",
          label: null,
          parentName: null,
          line: null,
          column: null,
          attributes: [],
          children: [
            {
              nodeId: 2,
              name: "layerMode",
              textValue: "High",
              listItems: [],
              xmlShape: "element",
              order: 0,
              known: false,
              line: null,
              column: null,
            },
          ],
        },
      ],
    },
  };
}
