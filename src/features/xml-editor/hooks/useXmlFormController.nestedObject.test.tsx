import { act, renderHook } from "@testing-library/react";
import { useXmlFormController, scalarFormValue } from "./useXmlFormController";
import type { XmlEdit } from "../types/xmlDocument";
import type { XmlEditorSnapshot } from "../types/editorSession";
import { makeNestedVisualCatalog } from "../__fixtures__/graphicData";

// --- Fixture-backed draft form controller tests for nested object fields ---

describe("useXmlFormController – nested object draft editing", () => {
  it("reflects draft changes to imagePath and renderStyle immediately without flushing", async () => {
    const catalog = makeNestedVisualCatalog();
    const snapshot: XmlEditorSnapshot = {
      rawXml: "<Defs><TestDef><visualData><imagePath>Things/Fixture/Single/FixtureSingle</imagePath><renderStyle>Single</renderStyle></visualData></TestDef></Defs>",
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
            defType: "TestDef",
            defName: "FixtureVisualSingle",
            label: null,
            parentName: null,
            line: null,
            column: null,
            attributes: [],
            children: [
              {
                nodeId: 2,
                name: "visualData",
                textValue: null,
                listItems: [],
                xmlShape: "object",
                order: 0,
                known: true,
                line: null,
                column: null,
                children: [
                  {
                    nodeId: 3,
                    name: "imagePath",
                    textValue: "Things/Fixture/Single/FixtureSingle",
                    listItems: [],
                    xmlShape: "element",
                    order: 0,
                    line: null,
                    column: null,
                  },
                  {
                    nodeId: 4,
                    name: "renderStyle",
                    textValue: "Single",
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

    const imagePathField = result.current.snapshot!.fields.find((f) => f.model.key === "visualData.imagePath")!;
    const renderStyleField = result.current.snapshot!.fields.find((f) => f.model.key === "visualData.renderStyle")!;

    act(() => {
      result.current.setFieldValue(imagePathField.model.id, scalarFormValue("Things/Changed/NewPath"));
    });
    act(() => {
      result.current.setFieldValue(renderStyleField.model.id, scalarFormValue("Multi"));
    });

    const updatedImagePath = result.current.snapshot!.fields.find((f) => f.model.key === "visualData.imagePath")!;
    const updatedRenderStyle = result.current.snapshot!.fields.find((f) => f.model.key === "visualData.renderStyle")!;
    expect(updatedImagePath.value).toEqual(scalarFormValue("Things/Changed/NewPath"));
    expect(updatedRenderStyle.value).toEqual(scalarFormValue("Multi"));
    expect(result.current.hasDraftChanges).toBe(true);

    await act(async () => {
      await result.current.flushAll();
    });

    expect(edits.length).toBeGreaterThan(0);
    const allEdits = edits.flat();
    const imageEdit = allEdits.find(
      (e) => e.type === "setNestedElementText" && "fieldName" in e && e.fieldName === "imagePath",
    );
    const styleEdit = allEdits.find(
      (e) => e.type === "setNestedElementText" && "fieldName" in e && e.fieldName === "renderStyle",
    );
    expect(imageEdit).toBeDefined();
    expect(styleEdit).toBeDefined();
  });
});
