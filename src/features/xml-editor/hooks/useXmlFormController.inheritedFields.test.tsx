import { renderHook } from "@testing-library/react";
import { useXmlFormController } from "./useXmlFormController";
import { makeInheritedObjectCatalog, makeInheritedObjectSnapshot } from "../__fixtures__/inheritedObject";

// --- Inherited field form controller tests ---

describe("useXmlFormController – inherited fields from BaseDef", () => {
  it("blockedLayers renders as editable list field with listItems path", () => {
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot: makeInheritedObjectSnapshot(),
        catalog: makeInheritedObjectCatalog(),
        selectedDefNodeId: 1,
        commitEdits: async () => "<xml/>",
        clearPreview: vi.fn(),
      }),
    );

    const field = result.current.snapshot!.fields.find((f) => f.model.key === "blockedLayers")!;
    expect(field).toBeDefined();
    expect(field.model.control).toBe("list");
    expect(field.model.readonly).toBe(false);
    expect(field.model.path).toEqual({ kind: "listItems", childName: "blockedLayers" });
  });

  it("difficultyConfig.costList renders as editable namedMap nested inside difficultyConfig", () => {
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot: makeInheritedObjectSnapshot(),
        catalog: makeInheritedObjectCatalog(),
        selectedDefNodeId: 1,
        commitEdits: async () => "<xml/>",
        clearPreview: vi.fn(),
      }),
    );

    const field = result.current.snapshot!.fields.find(
      (f) => f.model.key === "difficultyConfig.costList",
    )!;
    expect(field).toBeDefined();
    expect(field.model.control).toBe("namedMap");
    expect(field.model.readonly).toBe(false);
    expect(field.model.path).toEqual({
      kind: "namedMap",
      objectPath: ["difficultyConfig"],
      mapName: "costList",
    });
  });

  it("variantItems renders as editable objectList with VariantItem item schema", () => {
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot: makeInheritedObjectSnapshot(),
        catalog: makeInheritedObjectCatalog(),
        selectedDefNodeId: 1,
        commitEdits: async () => "<xml/>",
        clearPreview: vi.fn(),
      }),
    );

    const field = result.current.snapshot!.fields.find((f) => f.model.key === "variantItems")!;
    expect(field).toBeDefined();
    expect(field.model.control).toBe("objectList");
    expect(field.model.readonly).toBe(false);
    expect(field.model.itemSchemaRef).toBe("VariantItem");
  });

  it("iconVariants renders as editable objectList with IconVariant item schema", () => {
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot: makeInheritedObjectSnapshot(),
        catalog: makeInheritedObjectCatalog(),
        selectedDefNodeId: 1,
        commitEdits: async () => "<xml/>",
        clearPreview: vi.fn(),
      }),
    );

    const field = result.current.snapshot!.fields.find((f) => f.model.key === "iconVariants")!;
    expect(field).toBeDefined();
    expect(field.model.control).toBe("objectList");
    expect(field.model.readonly).toBe(false);
    expect(field.model.itemSchemaRef).toBe("IconVariant");
  });
});
