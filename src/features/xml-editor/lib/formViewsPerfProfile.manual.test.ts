// Form Views (issue 10, Plan.md section 10): the "documented manual large-form profile" the
// issue requires -- real, instrumented measurements (not a description of the memoization
// mechanism) of form-open, view-switch, and typing cost against the large synthetic
// ThingDef-shaped fixture (`../__fixtures__/largeThingDef.ts`, 135 top-level fields, 210
// descriptors, one order of magnitude below the real `rimworld-core` ThingDef.json's ~196
// fields but the same shape).
//
// Uses the project's own `measure()` timer (`src/instrumentation/timer.ts`) directly against the
// real production `buildFormFieldModels`/`useXmlFormController` code paths -- not a hand-rolled
// substitute, and not a narrative in place of numbers. Imports the raw (ungated) timer rather
// than the `src/instrumentation/index.ts` wrapper so results don't depend on this test runner's
// `import.meta.env.DEV`/`VITE_RIMEDIT_INSTRUMENTATION` detection; the gated wrapper (already used
// in production in `useXmlEditorSession.ts`'s `previewSave` path) is a zero-cost, env-gated call
// to this exact same function when enabled.
//
// This file deliberately does NOT assert wall-clock thresholds or compare two timings against
// each other (per Plan.md's explicit "avoid fragile timing assertions" guidance -- even a
// same-run relative comparison between two sub-millisecond operations is exactly the kind of
// flaky assertion that guidance warns against). The structural/count-based proof that typing
// stays O(1) and a view switch rebuilds at most once already lives in
// `useXmlFormController.largeForm.test.tsx` (dirty-field-count and `store.reset` call-count
// assertions). This file's only job is to PRINT real measured numbers -- via the timer's
// `console.debug` events and the `console.info` summary below -- for the issue's acceptance-notes
// record. Its own assertions are loose sanity bounds (durations are non-negative finite numbers,
// field counts match the fixture) that could never meaningfully fail from ordinary timing noise.
import { act, renderHook } from "@testing-library/react";
import { measure } from "../../../instrumentation/timer";
import { buildFormFieldModels } from "./formDescriptors";
import { useXmlFormController } from "../hooks/useXmlFormController";
import type { XmlEditorSnapshot } from "../types/editorSession";
import {
  allTopLevelFieldIds,
  buildLargeThingDefCatalog,
  buildLargeThingDefEditorView,
  scalarFieldId,
  scalarsOnlyVisibleSet,
} from "../__fixtures__/largeThingDef";

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0 ? (sorted[mid - 1] + sorted[mid]) / 2 : sorted[mid];
}

function timeIterations(name: string, iterations: number, run: () => void): number[] {
  const durations: number[] = [];
  for (let i = 0; i < iterations; i++) {
    measure(name, () => {
      const start = performance.now();
      run();
      durations.push(performance.now() - start);
    });
  }
  return durations;
}

function makeLargeSnapshot(): XmlEditorSnapshot {
  const def = buildLargeThingDefEditorView();
  return {
    rawXml: "<Defs><ThingDef>(large synthetic fixture)</ThingDef></Defs>",
    parseDiagnostics: [],
    validationDiagnostics: [],
    selectedDefNodeId: def.nodeId,
    parsed: {
      nodeCount: def.children.length + 1,
      rootElement: "Defs",
      profile: "defs",
      about: null,
      defs: [def],
    },
  };
}

describe("Form Views manual large-form performance profile (issue 10, Plan.md section 10)", () => {
  it("records real measure() timings for form-open, view-switch, and typing against the large fixture", () => {
    const catalog = buildLargeThingDefCatalog();
    const def = buildLargeThingDefEditorView();
    const schema = catalog.defTypes[def.defType];
    const ITERATIONS = 25;
    const minimalVisible = scalarsOnlyVisibleSet();
    const fullVisible = new Set(allTopLevelFieldIds());

    // 1. "Form open" proxy: descriptor+model construction for the full (unfiltered) large form --
    // exactly what `useXmlFormController` runs once on mount.
    const openDurations = timeIterations(
      "perf.largeForm.buildFormFieldModels.full",
      ITERATIONS,
      () => {
        buildFormFieldModels(def, schema, catalog);
      },
    );

    // 2. "View switch to a minimal view" proxy: same construction, filtered to hide the bulk of
    // the field surface (every object/list root) -- the expensive-root-skip path.
    const switchToMinimalDurations = timeIterations(
      "perf.largeForm.buildFormFieldModels.viewSwitchToMinimal",
      ITERATIONS,
      () => {
        buildFormFieldModels(def, schema, catalog, minimalVisible);
      },
    );

    // 3. "View switch back to full" proxy: the reverse direction (restoring every hidden field).
    const switchToFullDurations = timeIterations(
      "perf.largeForm.buildFormFieldModels.viewSwitchToFull",
      ITERATIONS,
      () => {
        buildFormFieldModels(def, schema, catalog, fullVisible);
      },
    );

    // 4. Real controller mount ("form open" end-to-end, including `FormFieldStore`
    // construction) against the full large fixture -- timed around the actual `renderHook` call,
    // since the hook's `useMemo`/lazy store-init work runs synchronously inside it.
    let mountDurationMs = 0;
    const hookResult = measure("perf.largeForm.controller.mount", () => {
      const start = performance.now();
      const r = renderHook(
        (props: { visible: ReadonlySet<string> | undefined }) =>
          useXmlFormController({
            snapshot: makeLargeSnapshot(),
            catalog,
            selectedDefNodeId: def.nodeId,
            commitEdits: async () => "<xml/>",
            clearPreview: () => {},
            visibleTopLevelFieldIds: props.visible,
          }),
        { initialProps: { visible: fullVisible as ReadonlySet<string> | undefined } },
      );
      mountDurationMs = performance.now() - start;
      return r;
    });
    const { result, rerender } = hookResult;
    const initialFieldCount = result.current.snapshot!.fields.length;

    // 5. Real controller view-switch rebuild (the actual `useLayoutEffect` + `store.reset` path
    // a live Form View switch takes), hiding the bulk of the field surface in one rerender.
    let controllerSwitchDurationMs = 0;
    measure("perf.largeForm.controller.viewSwitchRerender", () => {
      const start = performance.now();
      act(() => {
        rerender({ visible: minimalVisible });
      });
      controllerSwitchDurationMs = performance.now() - start;
    });
    const afterSwitchFieldCount = result.current.snapshot!.fields.length;

    // 6. "Typing" proxy: cost of ONE `setFieldValue` call on a single remaining-visible scalar
    // field after the switch above. `useXmlFormController.largeForm.test.tsx` already proves
    // this is structurally O(1) (only that field's store entry changes, verified via a dirty-id
    // count assertion, not timing) -- this measurement just records the real number alongside it.
    const targetId = result.current.snapshot!.fields.find(
      (f) => f.model.key === scalarFieldId(0),
    )!.model.id;
    let typingDurationMs = 0;
    measure("perf.largeForm.controller.singleFieldTypingUpdate", () => {
      const start = performance.now();
      act(() => {
        result.current.setFieldValue(targetId, { kind: "scalar", value: "typed-value" });
      });
      typingDurationMs = performance.now() - start;
    });

    const summary = {
      fixture: { topLevelFields: 135, descriptorsFull: 210, objectListItemsPerRoot: 8 },
      iterationsPerMeasurement: ITERATIONS,
      buildFormFieldModels_full_ms: {
        median: median(openDurations),
        min: Math.min(...openDurations),
        max: Math.max(...openDurations),
      },
      buildFormFieldModels_viewSwitchToMinimal_ms: {
        median: median(switchToMinimalDurations),
        min: Math.min(...switchToMinimalDurations),
        max: Math.max(...switchToMinimalDurations),
      },
      buildFormFieldModels_viewSwitchToFull_ms: {
        median: median(switchToFullDurations),
        min: Math.min(...switchToFullDurations),
        max: Math.max(...switchToFullDurations),
      },
      controller_mount_ms: mountDurationMs,
      controller_initialFieldCount: initialFieldCount,
      controller_viewSwitchRerender_ms: controllerSwitchDurationMs,
      controller_afterSwitchFieldCount: afterSwitchFieldCount,
      controller_singleFieldTypingUpdate_ms: typingDurationMs,
    };
    // eslint-disable-next-line no-console
    console.info("[rimedit:formViewsPerfProfile]", JSON.stringify(summary, null, 2));

    // Loose sanity bounds only -- not a CI performance gate, and no comparison between two
    // timings (Plan.md's explicit "avoid fragile timing assertions" guidance).
    for (const d of [...openDurations, ...switchToMinimalDurations, ...switchToFullDurations]) {
      expect(Number.isFinite(d)).toBe(true);
      expect(d).toBeGreaterThanOrEqual(0);
    }
    expect(Number.isFinite(mountDurationMs)).toBe(true);
    expect(Number.isFinite(controllerSwitchDurationMs)).toBe(true);
    expect(Number.isFinite(typingDurationMs)).toBe(true);
    expect(initialFieldCount).toBe(210);
    expect(afterSwitchFieldCount).toBe(100);
  });
});
