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

// --- Issue 05: Form View top-level visibility filtering plumbing ---
//
// No real caller wires `visibleTopLevelFieldIds` in yet (issue 06 owns that); these tests
// exercise the mechanism directly via a small test harness, per the issue's own testing
// requirement. `makeSnapshot`/`makeCatalog` (defined above) describe a ThingDef with
// `defName`, `description`, and `ingredients` top-level fields.
describe("useXmlFormController – Form View visibility filtering (issue 05)", () => {
  type Props = Parameters<typeof useXmlFormController>[0];

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("omitting visibleTopLevelFieldIds matches today's unfiltered form", () => {
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot: makeSnapshot(),
        catalog: makeCatalog(),
        selectedDefNodeId: 1,
        commitEdits: async () => "<xml/>",
        clearPreview: vi.fn(),
      }),
    );
    const keys = result.current
      .snapshot!.fields.map((f) => f.model.key)
      .sort();
    expect(keys).toEqual(["defName", "description", "ingredients"]);
  });

  it("hides a field not in visibleTopLevelFieldIds and restores its value when shown again", () => {
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    expect(
      result.current.snapshot!.fields.some(
        (f) => f.model.key === "description",
      ),
    ).toBe(true);

    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName"]),
      });
    });

    expect(
      result.current.snapshot!.fields.some(
        (f) => f.model.key === "description",
      ),
    ).toBe(false);
    // Untouched sibling fields stay mounted.
    expect(
      result.current.snapshot!.fields.some((f) => f.model.key === "defName"),
    ).toBe(true);

    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set([
          "defName",
          "description",
          "ingredients",
        ]),
      });
    });

    const restored = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    );
    expect(restored).toBeDefined();
    expect(restored!.value).toEqual({ kind: "scalar", value: "Old" });
    expect(restored!.dirty).toBe(false);
  });

  it("rebuilds the store exactly once per visibility change, not once per hidden field", () => {
    const resetSpy = vi.spyOn(FormFieldStore.prototype, "reset");
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { rerender } = renderHook((p: Props) => useXmlFormController(p), {
      initialProps,
    });
    expect(resetSpy).not.toHaveBeenCalled();

    act(() => {
      // Hides two fields (description, ingredients) in a single prop change.
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName"]),
      });
    });

    expect(resetSpy).toHaveBeenCalledTimes(1);
  });

  it("does not rebuild when rerendered with a content-equal but different Set instance", () => {
    // A future caller may not memoize the Set across renders; equal *content* must not
    // force extra rebuilds (mirrors the existing catalog content-signature caching).
    const resetSpy = vi.spyOn(FormFieldStore.prototype, "reset");
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { rerender } = renderHook((p: Props) => useXmlFormController(p), {
      initialProps,
    });

    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set([
          "ingredients",
          "defName",
          "description",
        ]),
      });
    });

    expect(resetSpy).not.toHaveBeenCalled();
  });

  it("calls onFocusedFieldHidden when the currently focused field's root is hidden", () => {
    const onFocusedFieldHidden = vi.fn();
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
      onFocusedFieldHidden,
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    act(() => {
      result.current.focusField(description.model.id);
    });

    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName"]),
      });
    });

    expect(onFocusedFieldHidden).toHaveBeenCalledTimes(1);
    expect(onFocusedFieldHidden).toHaveBeenCalledWith(description.model.id);
  });

  it("does not call onFocusedFieldHidden when the focused field stays visible", () => {
    const onFocusedFieldHidden = vi.fn();
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
      onFocusedFieldHidden,
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    const defNameField = result.current.snapshot!.fields.find(
      (f) => f.model.key === "defName",
    )!;
    act(() => {
      result.current.focusField(defNameField.model.id);
    });

    act(() => {
      // Hides `description`, but the focused field (`defName`) remains visible.
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName", "ingredients"]),
      });
    });

    expect(onFocusedFieldHidden).not.toHaveBeenCalled();
  });

  it("never invokes commitEdits, applyFormEdits-equivalent, or dirties the form on a visibility change alone", () => {
    const commitEdits = vi.fn(async () => "<xml/>");
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits,
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName"]),
      });
    });

    expect(commitEdits).not.toHaveBeenCalled();
    expect(result.current.hasDraftChanges).toBe(false);
    expect(result.current.hasPendingCommits).toBe(false);
  });
});

// --- Issue 05 review fix: uncommitted drafts must survive a pure visibility rebuild ---
//
// Plan.md section 7/9's "no value is discarded" guarantee covers the CURRENT in-memory form
// value (including an uncommitted/dirty draft the user hasn't flushed yet), not merely the
// last-committed XML value. A visibility-only rebuild must never silently revert a dirty
// field to its last-saved value, whether that field's own visibility changed or a sibling's
// did (a whole-store rebuild happens either way).
describe("useXmlFormController – uncommitted drafts survive a pure visibility rebuild (issue 05)", () => {
  type Props = Parameters<typeof useXmlFormController>[0];

  it("preserves an unrelated field's dirty draft when a DIFFERENT field's visibility changes and changes back", () => {
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    // Dirty an uncommitted edit on `defName` - never flushed.
    const defName = result.current.snapshot!.fields.find(
      (f) => f.model.key === "defName",
    )!;
    act(() => {
      result.current.setFieldValue(
        defName.model.id,
        scalarFormValue("Uncommitted Steel"),
      );
    });
    expect(
      result.current.snapshot!.fields.find((f) => f.model.key === "defName")!
        .dirty,
    ).toBe(true);

    // Hide a DIFFERENT field (`description`) - forces a full store rebuild that does not
    // itself touch `defName`'s visibility.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName", "ingredients"]),
      });
    });

    const afterHide = result.current.snapshot!.fields.find(
      (f) => f.model.key === "defName",
    )!;
    expect(afterHide.dirty).toBe(true);
    expect(afterHide.value).toEqual({
      kind: "scalar",
      value: "Uncommitted Steel",
    });

    // Show `description` again - another rebuild.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set([
          "defName",
          "description",
          "ingredients",
        ]),
      });
    });

    const afterReshow = result.current.snapshot!.fields.find(
      (f) => f.model.key === "defName",
    )!;
    expect(afterReshow.dirty).toBe(true);
    expect(afterReshow.value).toEqual({
      kind: "scalar",
      value: "Uncommitted Steel",
    });
  });

  it("preserves a field's OWN dirty draft across being hidden and shown again", () => {
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("Uncommitted description"),
      );
    });
    expect(
      result.current.snapshot!.fields.find(
        (f) => f.model.key === "description",
      )!.dirty,
    ).toBe(true);

    // Hide `description` itself while it is dirty.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName", "ingredients"]),
      });
    });
    expect(
      result.current.snapshot!.fields.some(
        (f) => f.model.key === "description",
      ),
    ).toBe(false);

    // Show it again - the draft must reappear exactly as it was, not the last-saved "Old"
    // value from the raw XML snapshot.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set([
          "defName",
          "description",
          "ingredients",
        ]),
      });
    });

    const restored = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    expect(restored.dirty).toBe(true);
    expect(restored.value).toEqual({
      kind: "scalar",
      value: "Uncommitted description",
    });
  });

  it("discards a stashed hidden-field draft when a real document change happens while it is hidden", () => {
    // Draft preservation is specific to a PURE visibility change. A real document change
    // (e.g. a reparsed/edited document) arriving while a field is hidden must not resurrect
    // a stale stashed draft once that field becomes visible again - the fresh document value
    // wins, exactly as it already did before this feature existed.
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("Uncommitted description"),
      );
    });

    // Hide `description` while it is dirty.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName", "ingredients"]),
      });
    });

    // A real document change (different rawXml/parsed value) arrives while `description` is
    // hidden - e.g. an external raw edit, undo/redo, or reload. Visibility content is
    // unchanged (new Set instance, same members), isolating this as a pure document change.
    const base = makeSnapshot();
    const changedSnapshot: XmlEditorSnapshot = {
      ...base,
      rawXml: base.rawXml.replace("Old", "Reparsed"),
      parsed: {
        ...base.parsed!,
        defs: [
          {
            ...base.parsed!.defs[0],
            children: base.parsed!.defs[0].children.map((c) =>
              c.name === "description" ? { ...c, textValue: "Reparsed" } : c,
            ),
          },
        ],
      },
    };
    act(() => {
      rerender({
        ...initialProps,
        snapshot: changedSnapshot,
        visibleTopLevelFieldIds: new Set(["defName", "ingredients"]),
      });
    });

    // Show `description` again - it must reflect the new document value, not the stale draft.
    act(() => {
      rerender({
        ...initialProps,
        snapshot: changedSnapshot,
        visibleTopLevelFieldIds: new Set([
          "defName",
          "description",
          "ingredients",
        ]),
      });
    });

    const restored = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    expect(restored.dirty).toBe(false);
    expect(restored.value).toEqual({ kind: "scalar", value: "Reparsed" });
  });
});

// --- Issue 05 review fix: visibility-set signature must not collide on delimiter reuse ---
describe("useXmlFormController – visibility signature collision safety (issue 05)", () => {
  type Props = Parameters<typeof useXmlFormController>[0];

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("rebuilds when switching between a field literally named 'a,b' and the two fields 'a'/'b'", () => {
    // A naive `[...set].sort().join(",")` signature would serialize both `new Set(["a,b"])`
    // and `new Set(["a", "b"])` to the same string "a,b", silently colliding two genuinely
    // different visibility sets onto the same id and skipping the rebuild.
    const resetSpy = vi.spyOn(FormFieldStore.prototype, "reset");
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set(["a,b"]),
    };
    const { rerender } = renderHook((p: Props) => useXmlFormController(p), {
      initialProps,
    });
    expect(resetSpy).not.toHaveBeenCalled();

    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["a", "b"]),
      });
    });

    expect(resetSpy).toHaveBeenCalledTimes(1);
  });

  it("does not rebuild for a content-equal Set even when a member name contains a comma", () => {
    const resetSpy = vi.spyOn(FormFieldStore.prototype, "reset");
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set(["a,b"]),
    };
    const { rerender } = renderHook((p: Props) => useXmlFormController(p), {
      initialProps,
    });

    act(() => {
      rerender({
        ...initialProps,
        // New Set instance, same single member "a,b" - content-equal, must not rebuild.
        visibleTopLevelFieldIds: new Set(["a,b"]),
      });
    });

    expect(resetSpy).not.toHaveBeenCalled();
  });
});

// --- Issue 05 review round 2, finding 1: a pure visibility change must not invalidate an
// in-flight flush's commit-resolution guard (`draftVersionRef`). Before this fix, ANY store
// rebuild - including one caused purely by hiding/showing a field - reset that guard to 0,
// so a flush that started before the toggle would see a version mismatch once its commit
// resolved and incorrectly throw "Form changed while edits were being applied", even though
// no XML edit actually happened besides the flush's own.
describe("useXmlFormController – visibility change does not invalidate an in-flight flush (issue 05 review finding 1)", () => {
  type Props = Parameters<typeof useXmlFormController>[0];

  it("resolves a flush cleanly even when a pure visibility change (hiding a DIFFERENT field) happens while it is in flight", async () => {
    let resolveCommit: (rawXml: string) => void = () => undefined;
    let commitCalls = 0;
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => {
        commitCalls += 1;
        return new Promise<string>((resolve) => {
          resolveCommit = resolve;
        });
      },
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("New description"),
      );
    });

    const flushPromise = result.current.flushAll();
    // Wait for `commitEdits` to actually be invoked (and `resolveCommit` reassigned to the
    // real resolver) before proceeding - otherwise the hide/resolve below could race ahead
    // of the async `.then()` chain that calls it.
    await waitFor(() => expect(commitCalls).toBe(1));

    // Hide a DIFFERENT field while `description`'s commit is in flight.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName", "description"]),
      });
    });

    act(() => {
      resolveCommit("<xml>committed</xml>");
    });

    await expect(flushPromise).resolves.toBe("<xml>committed</xml>");
    expect(result.current.formError).toBeNull();
    expect(result.current.hasDraftChanges).toBe(false);
  });

  it("resolves a flush cleanly even when the field's OWN visibility changes while its commit is in flight", async () => {
    let resolveCommit: (rawXml: string) => void = () => undefined;
    let commitCalls = 0;
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => {
        commitCalls += 1;
        return new Promise<string>((resolve) => {
          resolveCommit = resolve;
        });
      },
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("New description"),
      );
    });

    const flushPromise = result.current.flushAll();
    await waitFor(() => expect(commitCalls).toBe(1));

    // Hide `description` itself while its own commit is in flight.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName", "ingredients"]),
      });
    });

    act(() => {
      resolveCommit("<xml>committed</xml>");
    });

    await expect(flushPromise).resolves.toBe("<xml>committed</xml>");
    expect(result.current.formError).toBeNull();
  });
});

// --- Issue 05 review round 2, finding 2: the hidden-draft cache (`draftOverridesRef`) must
// never resurrect a value that was already committed or explicitly discarded.
describe("useXmlFormController – hidden-field drafts never resurrect stale/discarded values (issue 05 review finding 2)", () => {
  type Props = Parameters<typeof useXmlFormController>[0];

  it("(a) shows the committed value, not the stale pre-commit draft, when a field hidden during its own in-flight commit is shown again", async () => {
    let resolveCommit: (rawXml: string) => void = () => undefined;
    let commitCalls = 0;
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => {
        commitCalls += 1;
        return new Promise<string>((resolve) => {
          resolveCommit = resolve;
        });
      },
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("Committed description"),
      );
    });

    const flushPromise = result.current.flushAll();
    await waitFor(() => expect(commitCalls).toBe(1));

    // Hide `description` itself WHILE its own commit is in flight.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName", "ingredients"]),
      });
    });
    expect(
      result.current.snapshot!.fields.some(
        (f) => f.model.key === "description",
      ),
    ).toBe(false);

    // The backend accepts the edit while `description` is still hidden.
    act(() => {
      resolveCommit("<xml>committed</xml>");
    });
    await flushPromise;

    // Show `description` again.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set([
          "defName",
          "description",
          "ingredients",
        ]),
      });
    });

    const restored = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    expect(restored.value).toEqual({
      kind: "scalar",
      value: "Committed description",
    });
    expect(restored.dirty).toBe(false);
  });

  it("(b) does not resurrect a stale hidden-field draft after discardDrafts()", () => {
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set([
        "defName",
        "description",
        "ingredients",
      ]),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("Uncommitted description"),
      );
    });

    // Hide `description` while it is dirty - stashes the draft.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName", "ingredients"]),
      });
    });

    // Explicitly discard all drafts while `description` is hidden.
    act(() => {
      result.current.discardDrafts();
    });

    // Show `description` again - it must show the clean original value, not the discarded
    // draft.
    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set([
          "defName",
          "description",
          "ingredients",
        ]),
      });
    });

    const restored = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    expect(restored.value).toEqual({ kind: "scalar", value: "Old" });
    expect(restored.dirty).toBe(false);
  });
});

// --- Issue 05 review round 3: `draftVersionRef` must be a genuinely monotonic generation
// counter, not "reset to a fixed value on rebuild, increment per edit". The reset-to-0
// scheme predates Form Views (present in the initial commit before this feature touched the
// file), but a real rebuild happening while a flush is in flight, followed by exactly one
// further genuine edit, could realign the counter back to the same value the stale flush
// had already captured - silently accepting a commit that should have been rejected.
describe("useXmlFormController – monotonic generation counter rejects a stale flush after a real rebuild (issue 05 review round 3)", () => {
  type Props = Parameters<typeof useXmlFormController>[0];

  it("rejects an in-flight flush as stale after a real document rebuild, even when exactly one further edit would have realigned a reset-to-0 counter", async () => {
    let resolveCommit: (rawXml: string) => void = () => undefined;
    let commitCalls = 0;
    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => {
        commitCalls += 1;
        return new Promise<string>((resolve) => {
          resolveCommit = resolve;
        });
      },
      clearPreview: vi.fn(),
    };
    const { result, rerender } = renderHook(
      (p: Props) => useXmlFormController(p),
      { initialProps },
    );

    // Step 1: edit `description` and start a flush that will resolve asynchronously.
    const description = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    act(() => {
      result.current.setFieldValue(
        description.model.id,
        scalarFormValue("First edit (will go stale)"),
      );
    });
    const flushPromise = result.current.flushAll();
    await waitFor(() => expect(commitCalls).toBe(1));

    // Step 2: WHILE that flush is pending, a real document change arrives from outside the
    // form (e.g. undo, or an external raw XML edit) - unrelated to the pending flush's own
    // (not-yet-resolved) commit. This is a genuine rebuild: a different rawXml/snapshot.
    const base = initialProps.snapshot!;
    const externallyChangedSnapshot: XmlEditorSnapshot = {
      ...base,
      rawXml:
        "<Defs><ThingDef><defName>Steel</defName><description>External change</description></ThingDef></Defs>",
      parsed: {
        ...base.parsed!,
        defs: [
          {
            ...base.parsed!.defs[0],
            children: base.parsed!.defs[0].children.map((c) =>
              c.name === "description"
                ? { ...c, textValue: "External change" }
                : c,
            ),
          },
        ],
      },
    };
    act(() => {
      rerender({ ...initialProps, snapshot: externallyChangedSnapshot });
    });

    // Step 3: exactly ONE further genuine edit after the rebuild - under the old reset-to-0
    // scheme (rebuild resets the counter to 0, then this one edit bumps it back to 1) this
    // would realign the per-edit counter with the stale flush's captured value (1).
    const defName = result.current.snapshot!.fields.find(
      (f) => f.model.key === "defName",
    )!;
    act(() => {
      result.current.setFieldValue(
        defName.model.id,
        scalarFormValue("NewSteelName"),
      );
    });

    // Step 4: the ORIGINAL (now-stale) flush finally resolves.
    act(() => {
      resolveCommit("<xml>stale-commit-result</xml>");
    });
    await expect(flushPromise).rejects.toThrow(
      "Form changed while edits were being applied",
    );

    // The stale commit must NOT have been applied: `description` still reflects the real
    // rebuild's value (not the stale flush's committed "First edit" value), and the genuine
    // `defName` edit remains an intact dirty draft rather than being silently marked clean.
    const restoredDescription = result.current.snapshot!.fields.find(
      (f) => f.model.key === "description",
    )!;
    expect(restoredDescription.value).toEqual({
      kind: "scalar",
      value: "External change",
    });
    expect(restoredDescription.dirty).toBe(false);

    const restoredDefName = result.current.snapshot!.fields.find(
      (f) => f.model.key === "defName",
    )!;
    expect(restoredDefName.value).toEqual({
      kind: "scalar",
      value: "NewSteelName",
    });
    expect(restoredDefName.dirty).toBe(true);
  });
});
