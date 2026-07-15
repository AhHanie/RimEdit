import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { PatchEditorPane } from "./PatchEditorPane";
import type { SchemaCatalog } from "../../../schema-catalog/types";
import type { PatchFile } from "../../types/patchFile";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function makeCatalog(): SchemaCatalog {
  return { formatVersion: 1, packs: [], defTypes: {}, objectTypes: {}, patchOperations: {} };
}

function initialPatchFile(): PatchFile {
  return {
    relativePath: "Patches/MyPatch.xml",
    xmlDeclaration: null,
    diagnostics: [],
    hadFatalParseError: false,
    operations: [
      {
        id: 0,
        className: "PatchOperationAdd",
        success: "normal",
        attributes: [],
        kind: {
          type: "add",
          data: { xpath: 'Defs/ThingDef[defName="Wall"]', valueXml: "<statBases/>", order: null },
        },
        span: null,
      },
      {
        id: 1,
        className: "PatchOperationSequence",
        success: "normal",
        attributes: [],
        kind: { type: "sequence", data: [] },
        span: null,
      },
    ],
  };
}

/** Fake "serialize" that just JSON-encodes the tree -- this test exercises PatchEditorPane's own
 * wiring (parse-on-load, edit -> reserialize -> propagate), not the real Rust XML serializer
 * (covered separately by `src-tauri/src/patches/tests`). */
function setupInvokeMock(initialFile: PatchFile) {
  invokeMock.mockImplementation((command, args) => {
    if (command === "parse_patch_operations") return Promise.resolve(initialFile);
    if (command === "serialize_patch_operations") {
      return Promise.resolve(JSON.stringify((args as { patchFile: PatchFile }).patchFile));
    }
    if (command === "complete_patch_operation_xpath") {
      return Promise.resolve({
        replaceFrom: 0,
        items: [],
        diagnostics: [],
        target: { kind: "unsupported" },
        resolvedField: null,
      });
    }
    return Promise.reject(new Error(`Unexpected command: ${String(command)}`));
  });
}

describe("PatchEditorPane", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("parses the raw XML buffer and renders the operation tree", async () => {
    setupInvokeMock(initialPatchFile());
    render(
      <PatchEditorPane
        relativePath="Patches/MyPatch.xml"
        rawXml="<Patch></Patch>"
        readOnly={false}
        catalog={makeCatalog()}
        projectId="test-project"
        onChangeRawXml={vi.fn()}
      />,
    );

    expect(await screen.findByText("PatchOperationAdd")).toBeTruthy();
    expect(screen.getByText("PatchOperationSequence")).toBeTruthy();
    expect(screen.getByDisplayValue('Defs/ThingDef[defName="Wall"]')).toBeTruthy();
  });

  it("edits an operation's XPath and propagates the reserialized XML", async () => {
    const onChangeRawXml = vi.fn();
    setupInvokeMock(initialPatchFile());
    render(
      <PatchEditorPane
        relativePath="Patches/MyPatch.xml"
        rawXml="<Patch></Patch>"
        readOnly={false}
        catalog={makeCatalog()}
        projectId="test-project"
        onChangeRawXml={onChangeRawXml}
      />,
    );

    const xpathInput = await screen.findByDisplayValue('Defs/ThingDef[defName="Wall"]');
    fireEvent.change(xpathInput, { target: { value: 'Defs/ThingDef[defName="Steel"]' } });

    await waitFor(() => {
      expect(onChangeRawXml).toHaveBeenCalled();
    });
    const lastCall = onChangeRawXml.mock.calls[onChangeRawXml.mock.calls.length - 1][0];
    expect(lastCall).toContain('Defs/ThingDef[defName=\\"Steel\\"]');
  });

  it("adds a new top-level built-in operation", async () => {
    const onChangeRawXml = vi.fn();
    setupInvokeMock(initialPatchFile());
    render(
      <PatchEditorPane
        relativePath="Patches/MyPatch.xml"
        rawXml="<Patch></Patch>"
        readOnly={false}
        catalog={makeCatalog()}
        projectId="test-project"
        onChangeRawXml={onChangeRawXml}
      />,
    );

    await screen.findByText("PatchOperationAdd");
    const addButtons = screen.getAllByRole("button", { name: /add operation/i });
    await userEvent.click(addButtons[addButtons.length - 1]);
    await userEvent.type(screen.getByPlaceholderText("Search operation type…"), "PatchOperationRemove");
    await userEvent.click(screen.getByRole("button", { name: /PatchOperationRemove/i }));

    await waitFor(() => {
      const allAddOps = screen.getAllByText("PatchOperationAdd");
      expect(allAddOps.length).toBeGreaterThan(0);
    });
    expect(screen.getByText("PatchOperationRemove")).toBeTruthy();
    await waitFor(() => {
      expect(onChangeRawXml).toHaveBeenCalled();
    });
    const lastCall = onChangeRawXml.mock.calls[onChangeRawXml.mock.calls.length - 1][0];
    expect(lastCall).toContain("PatchOperationRemove");
  });

  it("adds a nested operation inside an existing sequence", async () => {
    const onChangeRawXml = vi.fn();
    setupInvokeMock(initialPatchFile());
    render(
      <PatchEditorPane
        relativePath="Patches/MyPatch.xml"
        rawXml="<Patch></Patch>"
        readOnly={false}
        catalog={makeCatalog()}
        projectId="test-project"
        onChangeRawXml={onChangeRawXml}
      />,
    );

    await screen.findByText("PatchOperationSequence");
    const nestedAddButton = screen.getByRole("button", { name: /add sequence operation/i });
    await userEvent.click(nestedAddButton);
    await userEvent.type(screen.getByPlaceholderText("Search operation type…"), "PatchOperationSetName");
    await userEvent.click(screen.getByRole("button", { name: /PatchOperationSetName/i }));

    await waitFor(() => {
      expect(screen.getByText("PatchOperationSetName")).toBeTruthy();
    });
    await waitFor(() => {
      expect(onChangeRawXml).toHaveBeenCalled();
    });
    const lastCall = onChangeRawXml.mock.calls[onChangeRawXml.mock.calls.length - 1][0];
    const rebuilt = JSON.parse(lastCall) as PatchFile;
    const sequenceOp = rebuilt.operations.find((op) => op.className === "PatchOperationSequence");
    expect(sequenceOp?.kind.type).toBe("sequence");
    if (sequenceOp?.kind.type === "sequence") {
      expect(sequenceOp.kind.data.some((child) => child.className === "PatchOperationSetName")).toBe(true);
    }
  });

  it("surfaces an unrecognized operation class as a diagnostic alongside parser diagnostics", async () => {
    const fileWithBogusClass: PatchFile = {
      relativePath: "Patches/MyPatch.xml",
      xmlDeclaration: null,
      diagnostics: [],
      hadFatalParseError: false,
      operations: [
        {
          id: 0,
          className: "Totally.Bogus.Class",
          success: "normal",
          attributes: [],
          kind: { type: "unknown", data: { rawXml: '<Operation Class="Totally.Bogus.Class"></Operation>' } },
          span: null,
        },
      ],
    };
    setupInvokeMock(fileWithBogusClass);
    render(
      <PatchEditorPane
        relativePath="Patches/MyPatch.xml"
        rawXml="<Patch></Patch>"
        readOnly={false}
        catalog={makeCatalog()}
        projectId="test-project"
        onChangeRawXml={vi.fn()}
      />,
    );

    await userEvent.click(await screen.findByText(/1 issue/));
    expect(
      await screen.findByText(/not a recognized built-in patch operation class/),
    ).toBeTruthy();
  });

  it("tags a custom operation added inside a sequence as <li>, not <Operation>", async () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      defTypes: {},
      objectTypes: {},
      patchOperations: {
        "MyMod.PatchOperationCustom": {
          className: "MyMod.PatchOperationCustom",
          label: "Custom Op",
          fieldOrder: ["xpath"],
          fields: {
            xpath: {
              type: { kind: "string" },
              required: true,
              examples: [],
              repeatable: false,
              xml: "element",
              flags: false,
              role: "xpath",
            },
          },
          preview: { kind: "unsupported" },
        },
      },
    };

    const onChangeRawXml = vi.fn();
    invokeMock.mockImplementation((command, args) => {
      if (command === "parse_patch_operations") {
        const a = args as { relativePath: string; rawXml: string };
        // The real initial load always passes the real relativePath; the "wrap a fragment to
        // parse it" trick used by PatchAddOperationPanel always passes "" (see
        // lib/customOperationXml.ts's `wrapOperationForSlot`/PatchAddOperationPanel).
        if (a.relativePath === "") {
          expect(a.rawXml).toContain('<li Class="MyMod.PatchOperationCustom"');
          expect(a.rawXml).not.toContain("<Operation Class=\"MyMod.PatchOperationCustom\"");
          const customNode = {
            id: 0,
            className: "MyMod.PatchOperationCustom",
            success: "normal" as const,
            attributes: [],
            kind: { type: "unknown" as const, data: { rawXml: '<li Class="MyMod.PatchOperationCustom"><xpath>Defs/ThingDef</xpath></li>' } },
            span: null,
          };
          return Promise.resolve({
            relativePath: "",
            xmlDeclaration: null,
            diagnostics: [],
            hadFatalParseError: false,
            operations: [
              {
                id: 99,
                className: "PatchOperationSequence",
                success: "normal",
                attributes: [],
                kind: { type: "sequence", data: [customNode] },
                span: null,
              },
            ],
          } satisfies PatchFile);
        }
        return Promise.resolve(initialPatchFile());
      }
      if (command === "serialize_patch_operations") {
        return Promise.resolve(JSON.stringify((args as { patchFile: PatchFile }).patchFile));
      }
      if (command === "complete_patch_operation_xpath") {
        return Promise.resolve({
          replaceFrom: 0,
          items: [],
          diagnostics: [],
          target: { kind: "unsupported" },
          resolvedField: null,
        });
      }
      return Promise.reject(new Error(`Unexpected command: ${String(command)}`));
    });

    render(
      <PatchEditorPane
        relativePath="Patches/MyPatch.xml"
        rawXml="<Patch></Patch>"
        readOnly={false}
        catalog={catalog}
        projectId="test-project"
        onChangeRawXml={onChangeRawXml}
      />,
    );

    await screen.findByText("PatchOperationSequence");
    await userEvent.click(screen.getByRole("button", { name: /add sequence operation/i }));
    await userEvent.type(screen.getByPlaceholderText("Search operation type…"), "Custom Op");
    await userEvent.click(screen.getByRole("button", { name: /Custom Op/i }));
    await userEvent.click(await screen.findByRole("button", { name: "Create" }));

    await waitFor(() => {
      expect(onChangeRawXml).toHaveBeenCalled();
    });
    const lastCall = onChangeRawXml.mock.calls[onChangeRawXml.mock.calls.length - 1][0];
    const rebuilt = JSON.parse(lastCall) as PatchFile;
    const sequenceOp = rebuilt.operations.find((op) => op.className === "PatchOperationSequence");
    expect(sequenceOp?.kind.type).toBe("sequence");
    if (sequenceOp?.kind.type === "sequence") {
      const child = sequenceOp.kind.data[0];
      expect(child.kind.type).toBe("unknown");
      if (child.kind.type === "unknown") {
        expect(child.kind.data.rawXml).toContain("<li Class=");
      }
    }
  });
});
