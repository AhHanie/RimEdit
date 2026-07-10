import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import { useXmlFormController, scalarFormValue } from "./useXmlFormController";
import { FormFieldStore } from "../lib/formFieldStore";
import type { XmlEditorSnapshot } from "../types/editorSession";
import type { FieldSchema, SchemaCatalog } from "../../schema-catalog";
import type { XmlEdit, XmlEditContext } from "../types/xmlDocument";

function makeSnapshot(): XmlEditorSnapshot {
  return {
    rawXml:
      "<Defs><ThingDef><defName>Steel</defName><description>Old</description></ThingDef></Defs>",
    parseDiagnostics: [],
    validationDiagnostics: [],
    selectedDefNodeId: 1,
    parsed: {
      nodeCount: 4,
      rootElement: "Defs",
      profile: "defs",
      about: null,
      defs: [
        {
          nodeId: 1,
          defType: "ThingDef",
          defName: "Steel",
          label: null,
          parentName: null,
          line: null,
          column: null,
          attributes: [],
          children: [
            {
              nodeId: 2,
              name: "defName",
              textValue: "Steel",
              listItems: [],
              xmlShape: "element",
              order: 0,
              known: false,
              line: null,
              column: null,
            },
            {
              nodeId: 3,
              name: "description",
              textValue: "Old",
              listItems: [],
              xmlShape: "element",
              order: 1,
              known: false,
              line: null,
              column: null,
            },
            {
              nodeId: 4,
              name: "ingredients",
              textValue: null,
              listItems: ["Wood"],
              xmlShape: "listOfLi",
              order: 2,
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

function makeCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    objectTypes: {},
    defTypes: {
      ThingDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: [],
        fields: {
          defName: {
            type: { kind: "string" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
          },
          description: {
            type: { kind: "string" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
          },
          ingredients: {
            type: { kind: "list" },
            required: false,
            repeatable: true,
            xml: "listOfLi",
            examples: [],
            flags: false,
            items: { kind: "object" },
          },
        },
      },
    },
  };
}

describe("useXmlFormController", () => {
  it("marks description dirty immediately and flushes without blur", async () => {
    const edits: XmlEdit[][] = [];
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot: makeSnapshot(),
        catalog: makeCatalog(),
        selectedDefNodeId: 1,
        commitEdits: async (nextEdits) => {
          edits.push(nextEdits);
          return "<xml>updated</xml>";
        },
        clearPreview: vi.fn(),
      }),
    );

    const description = result.current.snapshot!.fields.find(
      (field) => field.model.key === "description",
    )!;

    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("New description"),
      );
    });

    expect(result.current.hasDraftChanges).toBe(true);

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits).toEqual([
      [
        {
          type: "setChildElementText",
          parentNodeId: 1,
          childName: "description",
          value: "New description",
        },
      ],
    ]);
    expect(result.current.hasDraftChanges).toBe(false);
  });

  it("keeps unsupported structured list fields read-only", async () => {
    const edits: XmlEdit[][] = [];
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot: makeSnapshot(),
        catalog: makeCatalog(),
        selectedDefNodeId: 1,
        commitEdits: async (nextEdits) => {
          edits.push(nextEdits);
          return "<xml>updated</xml>";
        },
        clearPreview: vi.fn(),
      }),
    );

    const ingredients = result.current.snapshot!.fields.find(
      (field) => field.model.key === "ingredients",
    )!;

    expect(ingredients.model.readonly).toBe(true);
    expect(ingredients.model.control).toBe("objectList");

    act(() => {
      result.current.setFieldValue(
        ingredients.model.id,
        scalarFormValue("Steel"),
      );
    });

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits).toEqual([]);
  });

  it("editing graphicData.texPath emits setNestedElementText", async () => {
    const graphicDataField: FieldSchema = {
      type: { kind: "object", schemaRef: "GraphicData" },
      required: false,
      repeatable: false,
      xml: "object",
      examples: [],
      flags: false,
    };
    const texPathField: FieldSchema = {
      type: { kind: "string" },
      required: false,
      repeatable: false,
      xml: "element",
      examples: [],
      flags: false,
    };
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: { fieldOrder: [], fields: { texPath: texPathField } },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: { graphicData: graphicDataField },
        },
      },
    };
    const snapshot: XmlEditorSnapshot = {
      rawXml:
        "<Defs><ThingDef><graphicData><texPath>Things/Old</texPath></graphicData></ThingDef></Defs>",
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: 1,
      parsed: {
        nodeCount: 4,
        rootElement: "Defs",
        profile: "defs",
        about: null,
        defs: [
          {
            nodeId: 1,
            defType: "ThingDef",
            defName: "Test",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "graphicData",
                textValue: null,
                listItems: [],
                xmlShape: "object",
                order: 0,
                known: false,
                line: null,
                column: null,
                children: [
                  {
                    nodeId: 3,
                    name: "texPath",
                    textValue: "Things/Old",
                    listItems: [],
                    xmlShape: "element",
                    order: 0,
                    line: null,
                    column: null,
                  },
                ],
              },
            ],
          },
        ],
      },
    };

    const edits: XmlEdit[][] = [];
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot,
        catalog,
        selectedDefNodeId: 1,
        commitEdits: async (nextEdits) => {
          edits.push(nextEdits);
          return "<xml>updated</xml>";
        },
        clearPreview: vi.fn(),
      }),
    );

    const texPathField2 = result.current.snapshot!.fields.find(
      (f) => f.model.key === "graphicData.texPath",
    )!;

    act(() => {
      result.current.setFieldValue(
        texPathField2.model.id,
        scalarFormValue("Things/New"),
      );
    });

    expect(result.current.hasDraftChanges).toBe(true);

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits).toEqual([
      [
        {
          type: "setNestedElementText",
          parentNodeId: 1,
          objectPath: ["graphicData"],
          fieldName: "texPath",
          value: "Things/New",
        },
      ],
    ]);
    expect(result.current.hasDraftChanges).toBe(false);
  });

  it("deeper nested fields emit setNestedElementText", async () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: [],
          fields: {
            shadowData: {
              type: { kind: "object", schemaRef: "ShadowData" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
          },
        },
        ShadowData: {
          fieldOrder: [],
          fields: {
            volume: {
              type: { kind: "float" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
          },
        },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: {
            graphicData: {
              type: { kind: "object", schemaRef: "GraphicData" },
              required: false,
              repeatable: false,
              xml: "object",
              examples: [],
              flags: false,
            },
          },
        },
      },
    };
    const snapshot: XmlEditorSnapshot = {
      rawXml: "<Defs><ThingDef><graphicData /></ThingDef></Defs>",
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
            defType: "ThingDef",
            defName: "Test",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "graphicData",
                textValue: null,
                listItems: [],
                xmlShape: "object",
                order: 0,
                known: false,
                line: null,
                column: null,
                children: [],
              },
            ],
          },
        ],
      },
    };

    const edits: XmlEdit[][] = [];
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot,
        catalog,
        selectedDefNodeId: 1,
        commitEdits: async (nextEdits) => {
          edits.push(nextEdits);
          return "<xml>updated</xml>";
        },
        clearPreview: vi.fn(),
      }),
    );

    const volumeField = result.current.snapshot!.fields.find(
      (f) => f.model.key === "graphicData.shadowData.volume",
    )!;

    expect(volumeField.model.readonly).toBe(false);

    act(() => {
      result.current.setFieldValue(
        volumeField.model.id,
        scalarFormValue("0.5"),
      );
    });

    expect(result.current.hasDraftChanges).toBe(true);

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits).toEqual([
      [
        {
          type: "setNestedElementText",
          parentNodeId: 1,
          objectPath: ["graphicData", "shadowData"],
          fieldName: "volume",
          value: "0.5",
        },
      ],
    ]);
  });

  it("missing parent object field can be edited and flushed", async () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: ["texPath"],
          fields: {
            texPath: {
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
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["defName", "graphicData"],
          fields: {
            defName: {
              type: { kind: "string" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
            graphicData: {
              type: { kind: "object", schemaRef: "GraphicData" },
              required: false,
              repeatable: false,
              xml: "object",
              examples: [],
              flags: false,
            },
          },
        },
      },
    };
    const snapshot: XmlEditorSnapshot = {
      rawXml: "<Defs><ThingDef><defName>X</defName></ThingDef></Defs>",
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
            defType: "ThingDef",
            defName: "X",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "defName",
                textValue: "X",
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

    const edits: XmlEdit[][] = [];
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot,
        catalog,
        selectedDefNodeId: 1,
        commitEdits: async (nextEdits) => {
          edits.push(nextEdits);
          return "<xml>updated</xml>";
        },
        clearPreview: vi.fn(),
      }),
    );

    const texPathField = result.current.snapshot!.fields.find(
      (f) => f.model.key === "graphicData.texPath",
    )!;

    // Field should be editable even though graphicData is absent from XML
    expect(texPathField.model.readonly).toBe(false);
    expect(texPathField.value).toEqual({ kind: "scalar", value: "" });

    act(() => {
      result.current.setFieldValue(
        texPathField.model.id,
        scalarFormValue("Things/New"),
      );
    });

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits).toEqual([
      [
        {
          type: "setNestedElementText",
          parentNodeId: 1,
          objectPath: ["graphicData"],
          fieldName: "texPath",
          value: "Things/New",
        },
      ],
    ]);
  });

  it("commit sends nestedFieldOrders in editContext when catalog has object schemas", async () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: ["texPath", "graphicClass"],
          fields: {
            texPath: {
              type: { kind: "string" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
            graphicClass: {
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
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["defName", "graphicData"],
          fields: {
            defName: {
              type: { kind: "string" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
            graphicData: {
              type: { kind: "object", schemaRef: "GraphicData" },
              required: false,
              repeatable: false,
              xml: "object",
              examples: [],
              flags: false,
            },
          },
        },
      },
    };
    const snapshot: XmlEditorSnapshot = {
      rawXml:
        "<Defs><ThingDef><defName>X</defName><graphicData><texPath>Old</texPath></graphicData></ThingDef></Defs>",
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: 1,
      parsed: {
        nodeCount: 5,
        rootElement: "Defs",
        profile: "defs",
        about: null,
        defs: [
          {
            nodeId: 1,
            defType: "ThingDef",
            defName: "X",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "defName",
                textValue: "X",
                listItems: [],
                xmlShape: "element",
                order: 0,
                known: false,
                line: null,
                column: null,
              },
              {
                nodeId: 3,
                name: "graphicData",
                textValue: null,
                listItems: [],
                xmlShape: "object",
                order: 1,
                known: false,
                line: null,
                column: null,
                children: [
                  {
                    nodeId: 4,
                    name: "texPath",
                    textValue: "Old",
                    listItems: [],
                    xmlShape: "element",
                    order: 0,
                    line: null,
                    column: null,
                  },
                ],
              },
            ],
          },
        ],
      },
    };

    const capturedContexts: (XmlEditContext | undefined)[] = [];
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot,
        catalog,
        selectedDefNodeId: 1,
        commitEdits: async (_edits, ctx) => {
          capturedContexts.push(ctx);
          return "<xml>updated</xml>";
        },
        clearPreview: vi.fn(),
      }),
    );

    const texPath = result.current.snapshot!.fields.find(
      (f) => f.model.key === "graphicData.texPath",
    )!;
    act(() => {
      result.current.setFieldValue(texPath.model.id, scalarFormValue("New"));
    });
    await act(async () => {
      await result.current.flushAll();
    });

    expect(capturedContexts[0]?.nestedFieldOrders).toBeDefined();
    expect(capturedContexts[0]?.nestedFieldOrders?.["graphicData"]).toEqual([
      "texPath",
      "graphicClass",
    ]);
  });

  it("does not mark a newer draft committed when a flush resolves", async () => {
    const edits: XmlEdit[][] = [];
    let resolveCommit: (rawXml: string) => void = () => undefined;
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot: makeSnapshot(),
        catalog: makeCatalog(),
        selectedDefNodeId: 1,
        commitEdits: async (nextEdits) => {
          edits.push(nextEdits);
          return new Promise<string>((resolve) => {
            resolveCommit = resolve;
          });
        },
        clearPreview: vi.fn(),
      }),
    );

    const description = result.current.snapshot!.fields.find(
      (field) => field.model.key === "description",
    )!;

    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("First draft"),
      );
    });

    const flushPromise = result.current.flushAll();
    await waitFor(() => expect(edits).toHaveLength(1));

    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("Second draft"),
      );
      resolveCommit("<xml>first draft</xml>");
    });

    await expect(flushPromise).rejects.toThrow(
      "Form changed while edits were being applied",
    );

    const updatedDescription = result.current.snapshot!.fields.find(
      (field) => field.model.key === "description",
    )!;
    expect(updatedDescription.dirty).toBe(true);
    expect(edits).toEqual([
      [
        {
          type: "setChildElementText",
          parentNodeId: 1,
          childName: "description",
          value: "First draft",
        },
      ],
    ]);
  });

  it("snapshot immediately reflects new def after rerender with a different selectedDefNodeId", () => {
    const graphicDataField: FieldSchema = {
      type: { kind: "object", schemaRef: "GraphicData" },
      required: false,
      repeatable: false,
      xml: "object",
      examples: [],
      flags: false,
    };
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: [],
          fields: {
            texPath: {
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
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: {
            defName: {
              type: { kind: "string" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
            graphicData: graphicDataField,
          },
        },
        PawnKindDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: {
            defName: {
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
    const snapshot: XmlEditorSnapshot = {
      rawXml: `<Defs>
        <ThingDef><defName>Steel</defName><graphicData><texPath>p</texPath></graphicData></ThingDef>
        <PawnKindDef><defName>Worker</defName></PawnKindDef>
      </Defs>`,
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: 1,
      parsed: {
        nodeCount: 6,
        rootElement: "Defs",
        profile: "defs",
        about: null,
        defs: [
          {
            nodeId: 1,
            defType: "ThingDef",
            defName: "Steel",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "defName",
                textValue: "Steel",
                listItems: [],
                xmlShape: "element",
                order: 0,
                known: false,
                line: null,
                column: null,
              },
              {
                nodeId: 3,
                name: "graphicData",
                textValue: null,
                listItems: [],
                xmlShape: "object",
                order: 1,
                known: false,
                line: null,
                column: null,
                children: [
                  {
                    nodeId: 4,
                    name: "texPath",
                    textValue: "p",
                    listItems: [],
                    xmlShape: "element",
                    order: 0,
                    line: null,
                    column: null,
                  },
                ],
              },
            ],
          },
          {
            nodeId: 5,
            defType: "PawnKindDef",
            defName: "Worker",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 6,
                name: "defName",
                textValue: "Worker",
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

    let currentSelectedDefNodeId = 1;
    const { result, rerender } = renderHook(() =>
      useXmlFormController({
        snapshot,
        catalog,
        selectedDefNodeId: currentSelectedDefNodeId,
        commitEdits: async () => "<xml/>",
        clearPreview: vi.fn(),
      }),
    );

    // Def A (ThingDef) has graphicData.texPath
    expect(result.current.snapshot!.defNodeId).toBe(1);
    expect(
      result.current.snapshot!.fields.some(
        (f) => f.model.key === "graphicData.texPath",
      ),
    ).toBe(true);

    // Switch to def B (PawnKindDef) which has only defName
    currentSelectedDefNodeId = 5;
    rerender();

    // Immediately after rerender, the snapshot must reflect def B -- no stale graphicData fields
    expect(result.current.snapshot!.defNodeId).toBe(5);
    expect(
      result.current.snapshot!.fields.every(
        (f) => !f.model.key.startsWith("graphicData"),
      ),
    ).toBe(true);
  });

  it("Preview Save: clearing description emits '' and stays '' after re-parsed empty element", async () => {
    const afterCommitSnapshot: XmlEditorSnapshot = {
      rawXml:
        "<Defs><ThingDef><defName>Steel</defName><description/></ThingDef></Defs>",
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: 1,
      parsed: {
        nodeCount: 4,
        rootElement: "Defs",
        profile: "defs",
        about: null,
        defs: [
          {
            nodeId: 1,
            defType: "ThingDef",
            defName: "Steel",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "defName",
                textValue: "Steel",
                listItems: [],
                xmlShape: "element",
                order: 0,
                known: false,
                line: null,
                column: null,
              },
              {
                nodeId: 3,
                name: "description",
                textValue: null,
                listItems: [],
                xmlShape: "element",
                order: 1,
                known: false,
                line: null,
                column: null,
              },
              {
                nodeId: 4,
                name: "ingredients",
                textValue: null,
                listItems: ["Wood"],
                xmlShape: "listOfLi",
                order: 2,
                known: false,
                line: null,
                column: null,
              },
            ],
          },
        ],
      },
    };

    const edits: XmlEdit[][] = [];
    type Props = Parameters<typeof useXmlFormController>[0];
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async (nextEdits) => {
        edits.push(nextEdits);
        return afterCommitSnapshot.rawXml;
      },
      clearPreview: vi.fn(),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;

    act(() => {
      result.current.setFieldValue(description.model.id, scalarFormValue(""));
    });
    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits[0]).toEqual([
      {
        type: "setChildElementText",
        parentNodeId: 1,
        childName: "description",
        value: "",
      },
    ]);

    act(() => {
      rerender({ ...initialProps, snapshot: afterCommitSnapshot });
    });

    const updated = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    expect(updated.value).toEqual({ kind: "scalar", value: "" });
  });

  it("rebuilds form state when object-type schema changes without changing def fields", async () => {
    const graphicDataField: FieldSchema = {
      type: { kind: "object", schemaRef: "GraphicData" },
      required: false,
      repeatable: false,
      xml: "object",
      examples: [],
      flags: false,
    };
    const baseSnapshot: XmlEditorSnapshot = {
      rawXml:
        "<Defs><ThingDef><graphicData><texPath>x</texPath></graphicData></ThingDef></Defs>",
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: 1,
      parsed: {
        nodeCount: 4,
        rootElement: "Defs",
        profile: "defs",
        about: null,
        defs: [
          {
            nodeId: 1,
            defType: "ThingDef",
            defName: "Test",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "graphicData",
                textValue: null,
                listItems: [],
                xmlShape: "object",
                order: 0,
                known: false,
                line: null,
                column: null,
                children: [
                  {
                    nodeId: 3,
                    name: "texPath",
                    textValue: "x",
                    listItems: [],
                    xmlShape: "element",
                    order: 0,
                    line: null,
                    column: null,
                  },
                ],
              },
            ],
          },
        ],
      },
    };
    const catalogV1: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: [],
          fields: {
            texPath: {
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
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: { graphicData: graphicDataField },
        },
      },
    };
    const catalogV2: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: [],
          fields: {
            texPath: {
              type: { kind: "string" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
            graphicClass: {
              type: { kind: "enum" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
              validationHints: {
                allowedValues: ["Graphic_Single", "Graphic_Multi"],
              },
            },
          },
        },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: { graphicData: graphicDataField },
        },
      },
    };

    let currentCatalog = catalogV1;
    const { result, rerender } = renderHook(() =>
      useXmlFormController({
        snapshot: baseSnapshot,
        catalog: currentCatalog,
        selectedDefNodeId: 1,
        commitEdits: async () => "<xml/>",
        clearPreview: vi.fn(),
      }),
    );

    expect(
      result.current.snapshot!.fields.find(
        (f) => f.model.key === "graphicData.graphicClass",
      ),
    ).toBeUndefined();

    currentCatalog = catalogV2;
    rerender();

    expect(
      result.current.snapshot!.fields.find(
        (f) => f.model.key === "graphicData.graphicClass",
      ),
    ).toBeDefined();
  });
});

// --- Step 4: skip the whole-form rebuild on a form-originated commit ---

describe("useXmlFormController – skip-rebuild on form commit (Step 4)", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  // A single-ThingDef snapshot with a `description` child, parameterised so a test can
  // simulate the re-parsed document that arrives after a commit (its rawXml, the parsed
  // description text, and the node count).
  function makeDescSnapshot(
    description: string,
    rawXml: string,
    nodeCount: number,
  ): XmlEditorSnapshot {
    return {
      rawXml,
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: 1,
      parsed: {
        nodeCount,
        rootElement: "Defs",
        profile: "defs",
        about: null,
        defs: [
          {
            nodeId: 1,
            defType: "ThingDef",
            defName: "Steel",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "defName",
                textValue: "Steel",
                listItems: [],
                xmlShape: "element",
                order: 0,
                known: false,
                line: null,
                column: null,
              },
              {
                nodeId: 3,
                name: "description",
                textValue: description,
                listItems: [],
                xmlShape: "element",
                order: 1,
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

  function descCatalog(): SchemaCatalog {
    return {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: {
            defName: {
              type: { kind: "string" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
            description: {
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

  type Props = Parameters<typeof useXmlFormController>[0];

  /** Edit `description`, flush, then deliver the committed snapshot - returns the reset spy. */
  async function commitAndDeliver(committedSnapshot: XmlEditorSnapshot) {
    const resetSpy = vi.spyOn(FormFieldStore.prototype, "reset");
    const catalog = descCatalog();
    const commitEdits = vi.fn(async () => committedSnapshot.rawXml);
    const initialProps: Props = {
      snapshot: makeDescSnapshot("Old", "<raw1>", 4),
      catalog,
      selectedDefNodeId: 1,
      commitEdits,
      clearPreview: vi.fn(),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    // Reset is never called on mount (the store is `initialize`d, not `reset`).
    expect(resetSpy).not.toHaveBeenCalled();

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("New"),
      );
    });
    await act(async () => {
      await result.current.flushAll();
    });

    // The re-parsed document for the form's own commit flows back in.
    act(() => {
      rerender({ ...initialProps, snapshot: committedSnapshot });
    });

    return resetSpy;
  }

  it("skips the rebuild when the commit is value-only and the re-parsed values match", async () => {
    const resetSpy = await commitAndDeliver(
      makeDescSnapshot("New", "<raw2>", 4),
    );
    expect(resetSpy).not.toHaveBeenCalled();
  });

  it("rebuilds when the committed change altered document structure (node count changed)", async () => {
    // Same rawXml marker, but the re-parsed doc has a different node count - node ids may have
    // shifted, so the optimistic skip is unsafe and a rebuild must happen.
    const resetSpy = await commitAndDeliver(
      makeDescSnapshot("New", "<raw2>", 5),
    );
    expect(resetSpy).toHaveBeenCalledTimes(1);
  });

  it("rebuilds when the backend canonicalized a field value (optimistic state diverged)", async () => {
    // Non-structural, count-preserving, but the re-parsed value differs from what the form
    // committed - the value-diff guard must fall back to a rebuild rather than show stale state.
    const resetSpy = await commitAndDeliver(
      makeDescSnapshot("CANONICALIZED", "<raw2>", 4),
    );
    expect(resetSpy).toHaveBeenCalledTimes(1);
  });
});

describe("clearField", () => {
  function makeControllerWithClear() {
    const edits: XmlEdit[][] = [];
    const clearPreview = vi.fn();
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot: makeSnapshot(),
        catalog: makeCatalog(),
        selectedDefNodeId: 1,
        commitEdits: async (nextEdits) => {
          edits.push(nextEdits);
          return "<xml>updated</xml>";
        },
        clearPreview,
      }),
    );
    return { result, edits, clearPreview };
  }

  it("clearField on a top-level scalar emits removeChildElement", async () => {
    const { result, edits } = makeControllerWithClear();

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;

    act(() => {
      result.current.clearField(description.model.id);
    });

    expect(result.current.hasDraftChanges).toBe(true);

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits).toEqual([
      [
        {
          type: "removeChildElement",
          parentNodeId: 1,
          childName: "description",
        },
      ],
    ]);
  });

  it("clearField on a nested scalar emits removeNestedElement", async () => {
    const graphicDataField = {
      type: { kind: "object" as const, schemaRef: "GraphicData" },
      required: false,
      repeatable: false,
      xml: "object" as const,
      examples: [],
      flags: false,
    };
    const texPathField = {
      type: { kind: "string" as const },
      required: false,
      repeatable: false,
      xml: "element" as const,
      examples: [],
      flags: false,
    };
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: { fieldOrder: [], fields: { texPath: texPathField } },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: { graphicData: graphicDataField },
        },
      },
    };
    const snapshot: import("../types/editorSession").XmlEditorSnapshot = {
      rawXml:
        "<Defs><ThingDef><graphicData><texPath>Things/Old</texPath></graphicData></ThingDef></Defs>",
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: 1,
      parsed: {
        nodeCount: 4,
        rootElement: "Defs",
        profile: "defs",
        about: null,
        defs: [
          {
            nodeId: 1,
            defType: "ThingDef",
            defName: "Test",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "graphicData",
                textValue: null,
                listItems: [],
                xmlShape: "object",
                order: 0,
                known: false,
                line: null,
                column: null,
                children: [
                  {
                    nodeId: 3,
                    name: "texPath",
                    textValue: "Things/Old",
                    listItems: [],
                    xmlShape: "element",
                    order: 0,
                    line: null,
                    column: null,
                  },
                ],
              },
            ],
          },
        ],
      },
    };

    const edits: XmlEdit[][] = [];
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot,
        catalog,
        selectedDefNodeId: 1,
        commitEdits: async (nextEdits) => {
          edits.push(nextEdits);
          return "<xml>updated</xml>";
        },
        clearPreview: vi.fn(),
      }),
    );

    const texPathFieldState = result.current.snapshot!.fields.find(
      (f) => f.model.key === "graphicData.texPath",
    )!;

    act(() => {
      result.current.clearField(texPathFieldState.model.id);
    });

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits).toEqual([
      [
        {
          type: "removeNestedElement",
          parentNodeId: 1,
          objectPath: ["graphicData"],
          fieldName: "texPath",
          pruneEmptyAncestors: true,
        },
      ],
    ]);
  });

  it("clearing two fields in the same section emits two removeNestedElement edits with pruneEmptyAncestors", async () => {
    const graphicDataField = {
      type: { kind: "object" as const, schemaRef: "GraphicData" },
      required: false,
      repeatable: false,
      xml: "object" as const,
      examples: [],
      flags: false,
    };
    const scalarField = {
      type: { kind: "string" as const },
      required: false,
      repeatable: false,
      xml: "element" as const,
      examples: [],
      flags: false,
    };
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: [],
          fields: { texPath: scalarField, graphicClass: scalarField },
        },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: { graphicData: graphicDataField },
        },
      },
    };
    const snapshot: import("../types/editorSession").XmlEditorSnapshot = {
      rawXml:
        "<Defs><ThingDef><graphicData><texPath>Things/Old</texPath><graphicClass>Graphic_Single</graphicClass></graphicData></ThingDef></Defs>",
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: 1,
      parsed: {
        nodeCount: 5,
        rootElement: "Defs",
        profile: "defs",
        about: null,
        defs: [
          {
            nodeId: 1,
            defType: "ThingDef",
            defName: "Test",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "graphicData",
                textValue: null,
                listItems: [],
                xmlShape: "object",
                order: 0,
                known: false,
                line: null,
                column: null,
                children: [
                  {
                    nodeId: 3,
                    name: "texPath",
                    textValue: "Things/Old",
                    listItems: [],
                    xmlShape: "element",
                    order: 0,
                    line: null,
                    column: null,
                  },
                  {
                    nodeId: 4,
                    name: "graphicClass",
                    textValue: "Graphic_Single",
                    listItems: [],
                    xmlShape: "element",
                    order: 1,
                    line: null,
                    column: null,
                  },
                ],
              },
            ],
          },
        ],
      },
    };

    const edits: XmlEdit[][] = [];
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot,
        catalog,
        selectedDefNodeId: 1,
        commitEdits: async (nextEdits) => {
          edits.push(nextEdits);
          return "<xml>updated</xml>";
        },
        clearPreview: vi.fn(),
      }),
    );

    const texPathFieldState = result.current.snapshot!.fields.find(
      (f) => f.model.key === "graphicData.texPath",
    )!;
    const graphicClassFieldState = result.current.snapshot!.fields.find(
      (f) => f.model.key === "graphicData.graphicClass",
    )!;

    act(() => {
      result.current.clearField(texPathFieldState.model.id);
      result.current.clearField(graphicClassFieldState.model.id);
    });

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits).toEqual([
      [
        {
          type: "removeNestedElement",
          parentNodeId: 1,
          objectPath: ["graphicData"],
          fieldName: "texPath",
          pruneEmptyAncestors: true,
        },
        {
          type: "removeNestedElement",
          parentNodeId: 1,
          objectPath: ["graphicData"],
          fieldName: "graphicClass",
          pruneEmptyAncestors: true,
        },
      ],
    ]);
  });

  it("clearField on a list emits removeChildElement, not setListItems with []", async () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: {
            ingredients: {
              type: { kind: "list" },
              required: false,
              repeatable: true,
              xml: "listOfLi",
              examples: [],
              flags: false,
            },
          },
        },
      },
    };
    const snapshot: import("../types/editorSession").XmlEditorSnapshot = {
      rawXml:
        "<Defs><ThingDef><ingredients><li>Wood</li></ingredients></ThingDef></Defs>",
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: 1,
      parsed: {
        nodeCount: 4,
        rootElement: "Defs",
        profile: "defs",
        about: null,
        defs: [
          {
            nodeId: 1,
            defType: "ThingDef",
            defName: "Test",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "ingredients",
                textValue: null,
                listItems: ["Wood"],
                xmlShape: "listOfLi",
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

    const edits: XmlEdit[][] = [];
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot,
        catalog,
        selectedDefNodeId: 1,
        commitEdits: async (nextEdits) => {
          edits.push(nextEdits);
          return "<xml>updated</xml>";
        },
        clearPreview: vi.fn(),
      }),
    );

    const ingredientsField = result.current.snapshot!.fields.find(
      (f) => f.model.key === "ingredients",
    )!;

    act(() => {
      result.current.clearField(ingredientsField.model.id);
    });

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits).toEqual([
      [
        {
          type: "removeChildElement",
          parentNodeId: 1,
          childName: "ingredients",
        },
      ],
    ]);
  });

  it("reset after clear restores the committed value and removes clearRequested", () => {
    const { result } = makeControllerWithClear();

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;

    act(() => {
      result.current.clearField(description.model.id);
    });

    const clearedField = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    expect(clearedField.clearRequested).toBe(true);
    expect(clearedField.value).toEqual({ kind: "scalar", value: "" });

    act(() => {
      result.current.resetField(description.model.id);
    });

    const resetField = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    expect(resetField.clearRequested).toBe(false);
    expect(resetField.value).toEqual({ kind: "scalar", value: "Old" });
    expect(resetField.dirty).toBe(false);
  });

  it("normal empty-string edit still emits setChildElementText, not removeChildElement", async () => {
    const { result, edits } = makeControllerWithClear();

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;

    act(() => {
      result.current.setFieldValue(description.model.id, scalarFormValue(""));
    });

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits).toEqual([
      [
        {
          type: "setChildElementText",
          parentNodeId: 1,
          childName: "description",
          value: "",
        },
      ],
    ]);
  });
});
