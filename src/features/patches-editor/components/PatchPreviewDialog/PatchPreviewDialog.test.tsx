import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { PatchPreviewDialog } from "./PatchPreviewDialog";
import type {
  PatchOperationKey,
  PatchPreviewOperationSummary,
  PatchPreviewResult,
  PatchPreviewTarget,
} from "../../types/patchPreview";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function target(overrides: Partial<PatchPreviewTarget> = {}): PatchPreviewTarget {
  return {
    locationId: "proj1",
    relativePath: "Defs/Things.xml",
    defType: "ThingDef",
    identity: "Wall",
    ordinal: 0,
    ...overrides,
  };
}

function key(operationId: number, overrides: Partial<PatchOperationKey> = {}): PatchOperationKey {
  return { locationId: "proj1", relativePath: "Patches/Patch.xml", operationId, ...overrides };
}

function operation(
  overrides: Partial<PatchPreviewOperationSummary> = {},
): PatchPreviewOperationSummary {
  return {
    key: key(0),
    className: "PatchOperationAdd",
    classification: "builtIn",
    previewSupport: { kind: "supported" },
    status: "applied",
    statusMessage: null,
    canReorder: true,
    defaultOrder: 0,
    fileOrder: 0,
    relativePath: "Patches/Patch.xml",
    locationId: "proj1",
    locationName: "My Mod",
    xpath: 'Defs/ThingDef[defName="Wall"]',
    target: { kind: "def", defType: "ThingDef", defName: "Wall" },
    ...overrides,
  };
}

function previewResult(overrides: Partial<PatchPreviewResult> = {}): PatchPreviewResult {
  return {
    xml: "<ThingDef><defName>Wall</defName></ThingDef>",
    defFound: true,
    isPartial: false,
    visibleOperations: [],
    operationTrace: [],
    applyDiagnostics: [],
    inheritanceDiagnostics: [],
    conflictDiagnostics: [],
    impactSummary: {
      visibleOperationCount: 0,
      reorderableOperationCount: 0,
      unsupportedOperationCount: 0,
      conflictCount: 0,
    },
    ...overrides,
  };
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("PatchPreviewDialog", () => {
  it("previews the selected Def by identity on mount", async () => {
    invokeMock.mockResolvedValue(previewResult());
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("preview_def_patches", {
        projectId: "proj1",
        target: target(),
        request: { disabled: [], order: [] },
      });
    });
  });

  it("shows a complete-preview banner when the result is not partial", async () => {
    invokeMock.mockResolvedValue(previewResult({ isPartial: false }));
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );
    expect(await screen.findByText("Complete preview.")).toBeTruthy();
  });

  it("shows a partial-preview banner distinct from a complete one", async () => {
    invokeMock.mockResolvedValue(previewResult({ isPartial: true }));
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );
    expect(
      await screen.findByText("Partial preview - some operations could not be fully previewed."),
    ).toBeTruthy();
  });

  it("lists only operations affecting the Def, split into normal and unknown-impact groups", async () => {
    invokeMock.mockResolvedValue(
      previewResult({
        visibleOperations: [
          operation({ key: key(0), className: "PatchOperationAdd" }),
          operation({
            key: key(1),
            className: "PatchOperationReplace",
            canReorder: false,
            target: { kind: "unsupported" },
          }),
        ],
      }),
    );
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );

    expect(await screen.findByText("Patch operations (1)")).toBeTruthy();
    expect(screen.getByText("Unknown impact (1)")).toBeTruthy();
    expect(screen.getByText("PatchOperationAdd")).toBeTruthy();
    expect(screen.getByText("PatchOperationReplace")).toBeTruthy();
  });

  it("disabling an operation updates the preview request and refetches", async () => {
    invokeMock.mockResolvedValue(
      previewResult({ visibleOperations: [operation({ key: key(0) })] }),
    );
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    await userEvent.click(screen.getByLabelText("Enable PatchOperationAdd"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenLastCalledWith("preview_def_patches", {
        projectId: "proj1",
        target: target(),
        request: { disabled: [key(0)], order: [] },
      });
    });
  });

  it("reordering a top-level operation updates the preview request order", async () => {
    invokeMock.mockResolvedValue(
      previewResult({
        visibleOperations: [
          operation({ key: key(0), className: "PatchOperationAdd" }),
          operation({ key: key(1), className: "PatchOperationReplace" }),
        ],
      }),
    );
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    await userEvent.click(screen.getByLabelText("Move PatchOperationAdd down"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenLastCalledWith("preview_def_patches", {
        projectId: "proj1",
        target: target(),
        request: { disabled: [], order: [key(1), key(0)] },
      });
    });
  });

  it("resetting the order restores the default (empty order override) state", async () => {
    invokeMock.mockResolvedValue(
      previewResult({
        visibleOperations: [
          operation({ key: key(0), className: "PatchOperationAdd" }),
          operation({ key: key(1), className: "PatchOperationReplace" }),
        ],
      }),
    );
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    await userEvent.click(screen.getByLabelText("Move PatchOperationAdd down"));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));

    await userEvent.click(screen.getByText("Reset order"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenLastCalledWith("preview_def_patches", {
        projectId: "proj1",
        target: target(),
        request: { disabled: [], order: [] },
      });
    });
  });

  it("renders a conflict diagnostic's code as its badge", async () => {
    invokeMock.mockResolvedValue(
      previewResult({
        conflictDiagnostics: [
          {
            code: "patch_conflict_duplicate_add_child",
            key: key(0),
            message: "2 Add operations add a <label> child at \"Defs/ThingDef\"",
          },
        ],
      }),
    );
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );

    expect(await screen.findByText("patch_conflict_duplicate_add_child")).toBeTruthy();
  });

  it("renders a status code/args pair through the shared diagnostic catalog, not the raw backend message", async () => {
    invokeMock.mockResolvedValue(
      previewResult({
        visibleOperations: [
          operation({
            key: key(0),
            status: "skipped",
            // The raw compatibility message intentionally differs from the catalog text below --
            // this proves the row renders the translated `statusCode`/`statusArgs` lookup, not a
            // pass-through of backend English (see `renderDiagnostic`'s priority order).
            statusMessage: 'Requires mod "Power++" to be active',
            statusCode: "patch_find_mod_dependency_not_active",
            statusArgs: { mods: ["Power++"] },
          }),
        ],
      }),
    );
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );

    expect(
      await screen.findByText(
        "Requires mod Power++ to be active, which is not registered as a location in this project.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText('Requires mod "Power++" to be active')).toBeFalsy();
  });

  it("falls back to the raw status message when no status code is present (pre-migration compatibility)", async () => {
    invokeMock.mockResolvedValue(
      previewResult({
        visibleOperations: [
          operation({
            key: key(0),
            status: "skipped",
            statusMessage: 'Requires mod "Power++" to be active',
          }),
        ],
      }),
    );
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );

    expect(
      await screen.findByText('Requires mod "Power++" to be active'),
    ).toBeTruthy();
  });

  it("describes an OR-chained defName match's reason text", async () => {
    invokeMock.mockResolvedValue(
      previewResult({
        visibleOperations: [
          operation({
            key: key(0),
            target: { kind: "defs", defType: "ThingDef", defNames: ["Wall", "Door"] },
          }),
        ],
      }),
    );
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={vi.fn()} />,
    );

    expect(
      await screen.findByText("Targets 2 Defs directly by defName"),
    ).toBeTruthy();
  });

  it("calls onClose when Close is clicked", async () => {
    invokeMock.mockResolvedValue(previewResult());
    const onClose = vi.fn();
    render(
      <PatchPreviewDialog projectId="proj1" target={target()} onClose={onClose} />,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    await userEvent.click(screen.getByText("Close"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
