import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { PatchAddOperationPanel } from "./PatchAddOperationPanel";
import type { SchemaCatalog } from "../../../schema-catalog/types";
import type { PatchFile, PatchOperationNode } from "../../types/patchFile";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function catalogWithCustomOp(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {},
    objectTypes: {},
    patchOperations: {
      "MyMod.PatchOperationFoo": {
        className: "MyMod.PatchOperationFoo",
        label: "Foo Operation",
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
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("PatchAddOperationPanel", () => {
  it("renders as a trigger button when closed", () => {
    render(<PatchAddOperationPanel catalog={null} generateId={() => 0} onAdd={vi.fn()} />);
    expect(screen.getByText("Add operation")).toBeTruthy();
  });

  it("uses a custom trigger label when provided", () => {
    render(
      <PatchAddOperationPanel
        catalog={null}
        generateId={() => 0}
        onAdd={vi.fn()}
        triggerLabel="Add sequence operation"
      />,
    );
    expect(screen.getByText("Add sequence operation")).toBeTruthy();
  });

  it("adds a built-in operation immediately with a blank default shape", async () => {
    const onAdd = vi.fn();
    render(<PatchAddOperationPanel catalog={null} generateId={() => 7} onAdd={onAdd} />);

    await userEvent.click(screen.getByText("Add operation"));
    await userEvent.type(screen.getByPlaceholderText("Search operation type…"), "PatchOperationRemove");
    await userEvent.click(screen.getByRole("button", { name: /PatchOperationRemove/i }));

    expect(onAdd).toHaveBeenCalledTimes(1);
    const node = onAdd.mock.calls[0][0] as PatchOperationNode;
    expect(node.id).toBe(7);
    expect(node.className).toBe("PatchOperationRemove");
    expect(node.kind).toEqual({ type: "remove", data: { xpath: null } });
    // The panel resets back to the trigger button after a built-in add, with no invoke needed.
    expect(screen.getByText("Add operation")).toBeTruthy();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("shows a metadata-driven field form for a custom operation class", async () => {
    render(<PatchAddOperationPanel catalog={catalogWithCustomOp()} generateId={() => 0} onAdd={vi.fn()} />);

    await userEvent.click(screen.getByText("Add operation"));
    await userEvent.click(screen.getByRole("button", { name: /Foo Operation/i }));

    expect(screen.getByText("Foo Operation")).toBeTruthy();
    expect(screen.getByText("Create")).toBeTruthy();
  });

  it("builds and parses custom operation XML, then adds the parsed node", async () => {
    const onAdd = vi.fn();
    invokeMock.mockImplementation((command, args) => {
      if (command === "parse_patch_operations") {
        const a = args as { relativePath: string; rawXml: string };
        expect(a.rawXml).toContain('Class="MyMod.PatchOperationFoo"');
        expect(a.rawXml).toContain("<xpath>Defs/ThingDef</xpath>");
        const parsedFile: PatchFile = {
          relativePath: "",
          xmlDeclaration: null,
          diagnostics: [],
          hadFatalParseError: false,
          operations: [
            {
              id: 0,
              className: "MyMod.PatchOperationFoo",
              success: "normal",
              attributes: [],
              kind: { type: "unknown", data: { rawXml: a.rawXml } },
              span: null,
            },
          ],
        };
        return Promise.resolve(parsedFile);
      }
      return Promise.reject(new Error(`Unexpected command: ${String(command)}`));
    });

    render(<PatchAddOperationPanel catalog={catalogWithCustomOp()} generateId={() => 42} onAdd={onAdd} />);

    await userEvent.click(screen.getByText("Add operation"));
    await userEvent.click(screen.getByRole("button", { name: /Foo Operation/i }));
    await userEvent.type(screen.getByRole("textbox"), "Defs/ThingDef");
    await userEvent.click(screen.getByText("Create"));

    expect(onAdd).toHaveBeenCalledTimes(1);
    const node = onAdd.mock.calls[0][0] as PatchOperationNode;
    expect(node.id).toBe(42);
    expect(node.className).toBe("MyMod.PatchOperationFoo");
    // Back to the trigger button after a successful add.
    expect(await screen.findByText("Add operation")).toBeTruthy();
  });

  it("shows an error and stays open when parsing the built XML fails", async () => {
    invokeMock.mockRejectedValue(new Error("boom"));
    render(<PatchAddOperationPanel catalog={catalogWithCustomOp()} generateId={() => 0} onAdd={vi.fn()} />);

    await userEvent.click(screen.getByText("Add operation"));
    await userEvent.click(screen.getByRole("button", { name: /Foo Operation/i }));
    await userEvent.click(screen.getByText("Create"));

    expect(await screen.findByText("boom")).toBeTruthy();
  });

  it("cancel resets state back to the trigger button", async () => {
    render(<PatchAddOperationPanel catalog={catalogWithCustomOp()} generateId={() => 0} onAdd={vi.fn()} />);

    await userEvent.click(screen.getByText("Add operation"));
    expect(screen.getByPlaceholderText("Search operation type…")).toBeTruthy();
    await userEvent.click(screen.getByText("Cancel"));

    expect(screen.getByText("Add operation")).toBeTruthy();
    expect(screen.queryByPlaceholderText("Search operation type…")).toBeNull();
  });
});
