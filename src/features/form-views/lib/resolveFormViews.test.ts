import {
  buildAvailableFormViews,
  buildDefaultFormView,
  computeEffectiveVisibility,
  formViewsStateKey,
  isCustomViewBaseUnavailable,
  isHiddenSetDirty,
  resolveSelectedFormView,
  toggleHiddenFieldId,
} from "./resolveFormViews";
import type { SchemaFormView } from "../../schema-catalog";
import type { CustomFormView } from "../types/formViews";

function schemaView(overrides: Partial<SchemaFormView> = {}): SchemaFormView {
  return {
    id: "weapon",
    label: "Weapon",
    order: 20,
    recommended: false,
    hiddenFieldIds: ["apparel"],
    declaredOnDefType: "ThingDef",
    ...overrides,
  };
}

function customView(overrides: Partial<CustomFormView> = {}): CustomFormView {
  return {
    id: "custom-1",
    target: { gameVersion: "1.6", defType: "ThingDef" },
    name: "My custom view",
    description: null,
    hiddenFieldIds: ["plant"],
    baseSchemaView: null,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("buildAvailableFormViews", () => {
  it("always puts Default first even with no schema/custom views", () => {
    const available = buildAvailableFormViews("ThingDef", undefined, []);
    expect(available).toHaveLength(1);
    expect(available[0]).toEqual(buildDefaultFormView("ThingDef"));
  });

  it("orders schema views by `order`, then puts Default first and custom views last", () => {
    const schemaViews = {
      minimal: schemaView({ id: "minimal", label: "Minimal", order: 30 }),
      weapon: schemaView({ id: "weapon", label: "Weapon", order: 20 }),
    };
    const available = buildAvailableFormViews("ThingDef", schemaViews, [
      customView({ id: "c1", name: "Custom" }),
    ]);
    expect(available.map((v) => `${v.origin}:${v.id}`)).toEqual([
      "default:default",
      "schema:weapon",
      "schema:minimal",
      "custom:c1",
    ]);
  });

  it("breaks a schema-view order tie by recommended-first, then label", () => {
    const schemaViews = {
      b: schemaView({ id: "b", label: "B View", order: 10, recommended: false }),
      a: schemaView({ id: "a", label: "A View", order: 10, recommended: true }),
    };
    const available = buildAvailableFormViews("ThingDef", schemaViews, []);
    expect(available.map((v) => v.id)).toEqual(["default", "a", "b"]);
  });

  it("orders custom views by creation time", () => {
    const customViews = [
      customView({ id: "later", createdAt: "2026-02-01T00:00:00Z" }),
      customView({ id: "earlier", createdAt: "2026-01-01T00:00:00Z" }),
    ];
    const available = buildAvailableFormViews("ThingDef", undefined, customViews);
    expect(available.map((v) => v.id)).toEqual(["default", "earlier", "later"]);
  });

  it("carries schema provenance and custom baseSchemaView through", () => {
    const schemaViews = {
      weapon: schemaView({
        source: { packId: "rimedit.core", packVersion: "1.6.0" },
      }),
    };
    const custom = customView({
      baseSchemaView: {
        viewId: "weapon",
        packId: "rimedit.core",
        packVersion: "1.6.0",
        declaredOnDefType: "ThingDef",
      },
    });
    const available = buildAvailableFormViews("ThingDef", schemaViews, [custom]);
    const resolvedSchema = available.find((v) => v.origin === "schema");
    const resolvedCustom = available.find((v) => v.origin === "custom");
    expect(resolvedSchema?.source).toEqual({ packId: "rimedit.core", packVersion: "1.6.0" });
    expect(resolvedCustom?.baseSchemaView?.viewId).toBe("weapon");
  });
});

describe("resolveSelectedFormView", () => {
  it("returns Default when nothing else is available", () => {
    const available = buildAvailableFormViews("ThingDef", undefined, []);
    expect(resolveSelectedFormView(available, null).origin).toBe("default");
  });

  it("returns the persisted selection when it still resolves", () => {
    const available = buildAvailableFormViews(
      "ThingDef",
      { weapon: schemaView() },
      [customView()],
    );
    const resolved = resolveSelectedFormView(available, { origin: "custom", id: "custom-1" });
    expect(resolved.origin).toBe("custom");
    expect(resolved.id).toBe("custom-1");
  });

  it("falls back to the recommended schema view when the persisted reference is gone", () => {
    const available = buildAvailableFormViews("ThingDef", {
      weapon: schemaView({ recommended: true }),
    }, []);
    const resolved = resolveSelectedFormView(available, { origin: "custom", id: "deleted" });
    expect(resolved.origin).toBe("schema");
    expect(resolved.id).toBe("weapon");
  });

  it("falls back to Default when no schema view is recommended and the persisted ref is gone", () => {
    const available = buildAvailableFormViews("ThingDef", { weapon: schemaView() }, []);
    const resolved = resolveSelectedFormView(available, { origin: "schema", id: "gone" });
    expect(resolved.origin).toBe("default");
  });

  it("falls back to the recommended schema view when there is no persisted selection at all", () => {
    const available = buildAvailableFormViews("ThingDef", {
      weapon: schemaView({ recommended: true }),
    }, []);
    expect(resolveSelectedFormView(available, null).id).toBe("weapon");
  });
});

describe("computeEffectiveVisibility", () => {
  const defaultView = buildDefaultFormView("ThingDef");
  const knownTopLevel = new Set(["defName", "apparel", "plant", "graphicData"]);

  it("returns a null visible-filter (no filter) for Default View with no override", () => {
    const { effectiveHidden, visibleTopLevelFieldIds } = computeEffectiveVisibility({
      selected: defaultView,
      override: null,
      knownTopLevel,
    });
    expect(effectiveHidden.size).toBe(0);
    expect(visibleTopLevelFieldIds).toBeNull();
  });

  it("computes visible = known - hidden for a selected view with a hidden set", () => {
    const selected = { ...defaultView, hiddenFieldIds: ["apparel", "plant"] };
    const { effectiveHidden, visibleTopLevelFieldIds } = computeEffectiveVisibility({
      selected,
      override: null,
      knownTopLevel,
    });
    expect([...effectiveHidden].sort()).toEqual(["apparel", "plant"]);
    expect([...(visibleTopLevelFieldIds ?? [])].sort()).toEqual(["defName", "graphicData"]);
  });

  it("prefers the override's hidden set over the selected view's when an override exists", () => {
    const selected = { ...defaultView, hiddenFieldIds: ["apparel"] };
    const { effectiveHidden } = computeEffectiveVisibility({
      selected,
      override: { hiddenFieldIds: new Set(["plant"]), isDirty: true },
      knownTopLevel,
    });
    expect([...effectiveHidden]).toEqual(["plant"]);
  });

  it("intersects with knownTopLevel so a stale/removed field id never leaks into the hidden set", () => {
    const selected = { ...defaultView, hiddenFieldIds: ["apparel", "noLongerAField"] };
    const { effectiveHidden, visibleTopLevelFieldIds } = computeEffectiveVisibility({
      selected,
      override: null,
      knownTopLevel,
    });
    expect([...effectiveHidden]).toEqual(["apparel"]);
    expect([...(visibleTopLevelFieldIds ?? [])].sort()).toEqual([
      "defName",
      "graphicData",
      "plant",
    ]);
  });
});

describe("isHiddenSetDirty", () => {
  const selected = { ...buildDefaultFormView("ThingDef"), hiddenFieldIds: ["apparel", "plant"] };

  it("is false when the hidden set exactly matches the selected view's", () => {
    expect(isHiddenSetDirty(new Set(["apparel", "plant"]), selected)).toBe(false);
  });

  it("is true when a field was added", () => {
    expect(isHiddenSetDirty(new Set(["apparel", "plant", "graphicData"]), selected)).toBe(true);
  });

  it("is true when a field was removed", () => {
    expect(isHiddenSetDirty(new Set(["apparel"]), selected)).toBe(true);
  });
});

describe("toggleHiddenFieldId (issue 07: shared toggle primitive)", () => {
  it("adds the id when it is not currently hidden", () => {
    const result = toggleHiddenFieldId(new Set(["apparel"]), "plant");
    expect([...result].sort()).toEqual(["apparel", "plant"]);
  });

  it("removes the id when it is currently hidden", () => {
    const result = toggleHiddenFieldId(new Set(["apparel", "plant"]), "plant");
    expect([...result]).toEqual(["apparel"]);
  });

  it("never mutates the input set", () => {
    const input = new Set(["apparel"]);
    toggleHiddenFieldId(input, "plant");
    expect([...input]).toEqual(["apparel"]);
  });
});

describe("formViewsStateKey", () => {
  it("disambiguates two Defs of the same type by ordinal", () => {
    expect(formViewsStateKey("proj1", "1.6", "ThingDef", 0)).not.toBe(
      formViewsStateKey("proj1", "1.6", "ThingDef", 1),
    );
  });

  it("disambiguates the same Def/ordinal across different game versions", () => {
    expect(formViewsStateKey("proj1", "1.6", "ThingDef", 0)).not.toBe(
      formViewsStateKey("proj1", "1.5", "ThingDef", 0),
    );
  });

  it("disambiguates the same Def/ordinal/version across different projects", () => {
    expect(formViewsStateKey("proj1", "1.6", "ThingDef", 0)).not.toBe(
      formViewsStateKey("proj2", "1.6", "ThingDef", 0),
    );
  });
});

describe("isCustomViewBaseUnavailable (Plan.md section 6/12: unavailable-base notice)", () => {
  it("is false when the recorded base view id still resolves as a schema view", () => {
    const available = buildAvailableFormViews("ThingDef", { weapon: schemaView() }, [
      customView({
        baseSchemaView: {
          viewId: "weapon",
          packId: "rimedit.core",
          packVersion: "1.6.0",
          declaredOnDefType: "ThingDef",
        },
      }),
    ]);
    const custom = available.find((v) => v.origin === "custom")!;
    expect(isCustomViewBaseUnavailable(custom, available)).toBe(false);
  });

  it("is true when the recorded base view id no longer resolves (renamed or removed)", () => {
    // The schema pack no longer declares a "weapon" view at all -- only the custom view's own
    // stored provenance still names it.
    const available = buildAvailableFormViews("ThingDef", {}, [
      customView({
        baseSchemaView: {
          viewId: "weapon",
          packId: "rimedit.core",
          packVersion: "1.6.0",
          declaredOnDefType: "ThingDef",
        },
      }),
    ]);
    const custom = available.find((v) => v.origin === "custom")!;
    expect(isCustomViewBaseUnavailable(custom, available)).toBe(true);
  });

  it("is false for a custom view with no recorded base at all", () => {
    const available = buildAvailableFormViews("ThingDef", {}, [
      customView({ baseSchemaView: null }),
    ]);
    const custom = available.find((v) => v.origin === "custom")!;
    expect(isCustomViewBaseUnavailable(custom, available)).toBe(false);
  });

  it("is false for Default and schema-origin views regardless of baseSchemaView", () => {
    const available = buildAvailableFormViews("ThingDef", { weapon: schemaView() }, []);
    const defaultView = available.find((v) => v.origin === "default")!;
    const schema = available.find((v) => v.origin === "schema")!;
    expect(isCustomViewBaseUnavailable(defaultView, available)).toBe(false);
    expect(isCustomViewBaseUnavailable(schema, available)).toBe(false);
  });
});
