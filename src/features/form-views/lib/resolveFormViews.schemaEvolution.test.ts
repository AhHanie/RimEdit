// Form Views (issue 10, Plan.md section 12/13): "schema view renamed/removed, fields
// removed/added, custom materialized snapshot remains usable, missing base provenance warning"
// as one explicit end-to-end SEQUENCE, not just each piece verified in isolation across
// different issues' test files. Walks a single custom view through a real schema-pack upgrade:
// a schema-defined view it was copied from disappears, one of its hidden fields is removed from
// the schema, and a brand-new field is introduced - then asserts the custom view survives,
// remains selectable, and its effective visibility is recomputed correctly against the new
// schema (Plan.md section 6: "later schema-view changes cannot silently rewrite a user's custom
// selection").
import {
  buildAvailableFormViews,
  computeEffectiveVisibility,
  isCustomViewBaseUnavailable,
  resolveSelectedFormView,
} from "./resolveFormViews";
import type { SchemaFormView } from "../../schema-catalog";
import type { CustomFormView } from "../types/formViews";

const DEF_TYPE = "ThingDef";

function makeSchemaV1(): { formViews: Record<string, SchemaFormView>; knownTopLevel: Set<string> } {
  // v1: fields a, b, c. A schema-defined "weapon" view hides a and b.
  return {
    formViews: {
      weapon: {
        id: "weapon",
        label: "Weapon",
        order: 20,
        recommended: true,
        hiddenFieldIds: ["a", "b"],
        declaredOnDefType: DEF_TYPE,
        source: { packId: "rimedit.core", packVersion: "1.6.0" },
      },
    },
    knownTopLevel: new Set(["a", "b", "c"]),
  };
}

function makeSchemaV2(): { formViews: Record<string, SchemaFormView>; knownTopLevel: Set<string> } {
  // v2 (a later pack upgrade): the "weapon" view is removed entirely, field "b" is removed from
  // the schema, and a brand-new field "d" is introduced. "a" and "c" survive unchanged.
  return {
    formViews: {},
    knownTopLevel: new Set(["a", "c", "d"]),
  };
}

describe("Form View schema-evolution sequence (issue 10, Plan.md section 12)", () => {
  it("a custom view copied from a schema view survives the view being renamed/removed, one of its hidden fields being removed, and a new field being introduced", () => {
    const v1 = makeSchemaV1();

    // Step 1: user duplicates the "weapon" schema view into a custom view. Per Plan.md section
    // 6, this materializes the hidden set as a snapshot, not a live reference.
    const customView: CustomFormView = {
      id: "custom-1",
      target: { gameVersion: "1.6", defType: DEF_TYPE },
      name: "My weapon view",
      description: null,
      hiddenFieldIds: [...v1.formViews.weapon.hiddenFieldIds],
      baseSchemaView: {
        viewId: "weapon",
        packId: "rimedit.core",
        packVersion: "1.6.0",
        declaredOnDefType: DEF_TYPE,
      },
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    };

    // Step 2: against v1, the custom view hides exactly what it was copied from, and its base
    // still resolves (no notice would be shown yet).
    const v1Available = buildAvailableFormViews(DEF_TYPE, v1.formViews, [customView]);
    const v1Selected = resolveSelectedFormView(v1Available, { origin: "custom", id: "custom-1" });
    expect(v1Selected.origin).toBe("custom");
    expect(isCustomViewBaseUnavailable(v1Selected, v1Available)).toBe(false);
    const v1Visibility = computeEffectiveVisibility({
      selected: v1Selected,
      override: null,
      knownTopLevel: v1.knownTopLevel,
    });
    expect(v1Visibility.visibleTopLevelFieldIds).toEqual(new Set(["c"]));

    // Step 3: the schema pack is upgraded to v2 - the "weapon" view this custom view was
    // derived from is gone, "b" no longer exists at all, and "d" is a brand-new field. The
    // on-disk custom view record itself is untouched (no store write happened) - exactly what
    // `form_views::store` round-trip/persistence tests already prove on the Rust side; this test
    // is the resolution-time half of the same contract.
    const v2 = makeSchemaV2();
    const v2Available = buildAvailableFormViews(DEF_TYPE, v2.formViews, [customView]);

    // The custom view record is still present and selectable - "custom materialized snapshot
    // remains usable" - even though its schema base and one of its hidden fields are both gone.
    expect(v2Available.some((v) => v.origin === "custom" && v.id === "custom-1")).toBe(true);
    // The formerly-recommended "weapon" schema view is gone; no schema view remains at all.
    expect(v2Available.some((v) => v.origin === "schema")).toBe(false);

    const v2Selected = resolveSelectedFormView(v2Available, { origin: "custom", id: "custom-1" });
    expect(v2Selected.origin).toBe("custom");
    expect(v2Selected.id).toBe("custom-1");
    // The raw stored hidden set is untouched - still exactly what was materialized in step 1.
    expect(v2Selected.hiddenFieldIds).toEqual(["a", "b"]);

    // Step 4: effective visibility against the NEW schema. "b" is silently dropped from the
    // effective hidden set (Plan.md section 7: `effectiveHidden = effectiveHidden ∩
    // knownTopLevel`) because it no longer exists - never a broken/crashing view. "a" is still
    // known and stays hidden. The new field "d" is visible by default (Plan.md section 4:
    // "hiddenFields is the sole base visibility semantics ... fields introduced in later schema
    // releases are visible by default") even though the custom view predates it entirely.
    const v2Visibility = computeEffectiveVisibility({
      selected: v2Selected,
      override: null,
      knownTopLevel: v2.knownTopLevel,
    });
    expect(v2Visibility.effectiveHidden).toEqual(new Set(["a"]));
    expect(v2Visibility.visibleTopLevelFieldIds).toEqual(new Set(["c", "d"]));

    // Step 5 ("missing base provenance"): `isCustomViewBaseUnavailable` -- the same function
    // `FormViewManagerDialog` calls to render its "derived from a view that's no longer
    // available" notice -- now reports true, even though (per step 3/4 above) the view remains
    // fully functional and its effective visibility is correctly recomputed either way. Plan.md
    // section 6: "a missing/renamed base becomes a nonblocking notice, not a broken view."
    expect(v2Selected.baseSchemaView?.viewId).toBe("weapon");
    expect(isCustomViewBaseUnavailable(v2Selected, v2Available)).toBe(true);
  });
});
