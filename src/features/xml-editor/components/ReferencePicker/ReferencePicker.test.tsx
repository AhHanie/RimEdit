import { fireEvent, screen, waitFor } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { invoke } from "@tauri-apps/api/core";
import { ReferencePicker } from "./ReferencePicker";
import type { DefReferenceSuggestion } from "../../../def-index/types";
import type { ReferenceMetadata } from "../../../schema-catalog";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

const reference: ReferenceMetadata = {
  defType: "ThingDef",
  allowAbstract: false,
  scope: "allSources",
};

function suggestion(overrides: Partial<DefReferenceSuggestion> = {}): DefReferenceSuggestion {
  return {
    defName: "Steel",
    defType: "ThingDef",
    label: "steel",
    relativePath: "Defs/Things/Steel.xml",
    nodeId: 42,
    line: 12,
    column: 5,
    locationId: "proj1",
    locationName: "My Mod",
    readOnly: false,
    rank: 1,
    ...overrides,
  };
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("ReferencePicker acceptedDefTypes", () => {
  it("passes acceptedDefTypes to the suggest API instead of defType", async () => {
    invokeMock.mockResolvedValue([]);

    const multiRef: ReferenceMetadata = {
      defType: "CreepJoinerBaseDef",
      allowAbstract: false,
      scope: "allSources",
      acceptedDefTypes: ["CreepJoinerBenefitDef", "CreepJoinerDownsideDef"],
    };

    render(
      <ReferencePicker
        value=""
        reference={multiRef}
        projectId="proj1"
        onChange={vi.fn()}
      />,
    );

    fireEvent.focus(screen.getByRole("textbox"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "suggest_def_references_cmd",
        expect.objectContaining({
          targetDefTypes: ["CreepJoinerBenefitDef", "CreepJoinerDownsideDef"],
        }),
      );
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "suggest_def_references_cmd",
      expect.objectContaining({ targetDefTypes: ["CreepJoinerBaseDef"] }),
    );
  });

  it("deduplicates acceptedDefTypes before passing to the suggest API", async () => {
    invokeMock.mockResolvedValue([]);

    const dupRef: ReferenceMetadata = {
      defType: "BaseDef",
      allowAbstract: false,
      scope: "allSources",
      acceptedDefTypes: ["ConcreteDefA", "ConcreteDefA", "ConcreteDefB"],
    };

    render(
      <ReferencePicker
        value=""
        reference={dupRef}
        projectId="proj1"
        onChange={vi.fn()}
      />,
    );

    fireEvent.focus(screen.getByRole("textbox"));

    await waitFor(() => {
      // After dedup: ["ConcreteDefA", "ConcreteDefB"] -- exactly once each, not three entries.
      expect(invokeMock).toHaveBeenCalledWith(
        "suggest_def_references_cmd",
        expect.objectContaining({
          targetDefTypes: ["ConcreteDefA", "ConcreteDefB"],
        }),
      );
    });
  });
});

describe("ReferencePicker actions", () => {
  it("uses suggestions for initial project values", async () => {
    const onNavigateDef = vi.fn();
    invokeMock.mockImplementation((command) => {
      if (command === "suggest_def_references_cmd") return Promise.resolve([suggestion()]);
      if (command === "resolve_def_reference_cmd") return Promise.resolve({ kind: "missing" });
      return Promise.resolve(null);
    });

    render(
      <ReferencePicker
        value="Steel"
        reference={reference}
        projectId="proj1"
        onChange={vi.fn()}
        onNavigateDef={onNavigateDef}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Go to ThingDef" }));

    await waitFor(() => {
      expect(onNavigateDef).toHaveBeenCalledWith(
        expect.objectContaining({
          locationId: "proj1",
          locationName: "My Mod",
          sourceKind: "project",
          readOnly: false,
          relativePath: "Defs/Things/Steel.xml",
        }),
        42,
      );
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "resolve_def_reference_cmd",
      expect.anything(),
    );
  });

  it("calls onNavigateDef for read-only source defs from the go-to action", async () => {
    const onNavigateDef = vi.fn();
    invokeMock.mockImplementation((command) => {
      if (command === "resolve_def_reference_cmd") return Promise.resolve({ kind: "missing" });
      if (command === "suggest_def_references_cmd") {
        return Promise.resolve([
          suggestion({
            relativePath: "Core/Things/Steel.xml",
            locationId: "core1",
            locationName: "Core",
            readOnly: true,
          }),
        ]);
      }
      return Promise.resolve(null);
    });

    render(
      <ReferencePicker
        value="Steel"
        reference={reference}
        projectId="proj1"
        onChange={vi.fn()}
        onNavigateDef={onNavigateDef}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Go to ThingDef" }));

    await waitFor(() => {
      expect(onNavigateDef).toHaveBeenCalledWith(
        expect.objectContaining({
          locationId: "core1",
          locationName: "Core",
          sourceKind: "source",
          readOnly: true,
          relativePath: "Core/Things/Steel.xml",
        }),
        42,
      );
    });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("matches initial values case-insensitively", async () => {
    const onNavigateDef = vi.fn();
    invokeMock.mockImplementation((command) => {
      if (command === "suggest_def_references_cmd") return Promise.resolve([suggestion()]);
      return Promise.resolve(null);
    });

    render(
      <ReferencePicker
        value="steel"
        reference={reference}
        projectId="proj1"
        onChange={vi.fn()}
        onNavigateDef={onNavigateDef}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Go to ThingDef" }));

    await waitFor(() => {
      expect(onNavigateDef).toHaveBeenCalledWith(
        expect.objectContaining({
          locationId: "proj1",
          relativePath: "Defs/Things/Steel.xml",
          readOnly: false,
        }),
        42,
      );
    });
  });
});
