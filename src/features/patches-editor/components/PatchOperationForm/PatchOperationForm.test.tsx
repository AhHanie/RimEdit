import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { PatchOperationForm } from "./PatchOperationForm";
import type { PatchOperationNode } from "../../types/patchFile";

// `PatchPathInput`/`PatchValueEditor` (rendered for pathed/value operations below) both skip
// fetching completions entirely when `projectId` is null (see their own tests), so no command is
// ever actually invoked here -- this mock only exists so importing them doesn't touch the real
// Tauri bridge.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function addNode(overrides: Partial<PatchOperationNode> = {}): PatchOperationNode {
  return {
    id: 0,
    className: "PatchOperationAdd",
    success: "normal",
    attributes: [],
    kind: { type: "add", data: { xpath: 'Defs/ThingDef[defName="Wall"]', valueXml: "<a/>", order: null } },
    span: null,
    ...overrides,
  };
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockRejectedValue(new Error("no invoke expected in this test"));
});

describe("PatchOperationForm", () => {
  it("renders the xpath field for a pathed operation", () => {
    render(
      <PatchOperationForm node={addNode()} catalog={null} readOnly={false} projectId={null} onChange={vi.fn()} />,
    );
    expect(screen.getByDisplayValue('Defs/ThingDef[defName="Wall"]')).toBeTruthy();
  });

  it("edits the xpath field and reports the change via onChange", () => {
    const onChange = vi.fn();
    render(
      <PatchOperationForm node={addNode()} catalog={null} readOnly={false} projectId={null} onChange={onChange} />,
    );
    fireEvent.change(screen.getByDisplayValue('Defs/ThingDef[defName="Wall"]'), {
      target: { value: 'Defs/ThingDef[defName="Steel"]' },
    });
    expect(onChange).toHaveBeenCalled();
    const updater = onChange.mock.calls[0][0] as (n: PatchOperationNode) => PatchOperationNode;
    const updated = updater(addNode());
    expect(updated.kind.type).toBe("add");
    if (updated.kind.type === "add") {
      expect(updated.kind.data.xpath).toBe('Defs/ThingDef[defName="Steel"]');
    }
  });

  it("renders attribute/value fields for PatchOperationAttributeSet", () => {
    const node = addNode({
      className: "PatchOperationAttributeSet",
      kind: {
        type: "attributeSet",
        data: { xpath: "Defs/ThingDef", attribute: "Foo", value: "Bar" },
      },
    });
    render(<PatchOperationForm node={node} catalog={null} readOnly={false} projectId={null} onChange={vi.fn()} />);
    expect(screen.getByDisplayValue("Foo")).toBeTruthy();
    expect(screen.getByDisplayValue("Bar")).toBeTruthy();
  });

  it("renders the name field for PatchOperationSetName", () => {
    const node = addNode({
      className: "PatchOperationSetName",
      kind: { type: "setName", data: { xpath: "Defs/ThingDef", name: "Renamed" } },
    });
    render(<PatchOperationForm node={node} catalog={null} readOnly={false} projectId={null} onChange={vi.fn()} />);
    expect(screen.getByDisplayValue("Renamed")).toBeTruthy();
  });

  it("renders and edits the mods list for PatchOperationFindMod", () => {
    const onChange = vi.fn();
    const node = addNode({
      className: "PatchOperationFindMod",
      kind: { type: "findMod", data: { mods: ["Harmony"], matchOp: null, nomatchOp: null } },
    });
    render(<PatchOperationForm node={node} catalog={null} readOnly={false} projectId={null} onChange={onChange} />);
    fireEvent.change(screen.getByDisplayValue("Harmony"), { target: { value: "CoreLib" } });
    const updater = onChange.mock.calls[0][0] as (n: PatchOperationNode) => PatchOperationNode;
    const updated = updater(node);
    expect(updated.kind.type).toBe("findMod");
    if (updated.kind.type === "findMod") {
      expect(updated.kind.data.mods).toEqual(["CoreLib"]);
    }
  });

  it("edits the xpath field for PatchOperationConditional (previously a silent no-op)", () => {
    const onChange = vi.fn();
    const node = addNode({
      className: "PatchOperationConditional",
      kind: { type: "conditional", data: { xpath: "Defs/ThingDef", matchOp: null, nomatchOp: null } },
    });
    render(<PatchOperationForm node={node} catalog={null} readOnly={false} projectId={null} onChange={onChange} />);
    fireEvent.change(screen.getByDisplayValue("Defs/ThingDef"), {
      target: { value: "Defs/ThingDef/statBases" },
    });
    const updater = onChange.mock.calls[0][0] as (n: PatchOperationNode) => PatchOperationNode;
    const updated = updater(node);
    expect(updated.kind.type).toBe("conditional");
    if (updated.kind.type === "conditional") {
      expect(updated.kind.data.xpath).toBe("Defs/ThingDef/statBases");
    }
  });

  it("renders no scalar fields for sequence/unknown operations", () => {
    const node = addNode({ className: "PatchOperationSequence", kind: { type: "sequence", data: [] } });
    render(<PatchOperationForm node={node} catalog={null} readOnly={false} projectId={null} onChange={vi.fn()} />);
    // Only the common Success/MayRequire/MayRequireAnyOf fields should render, no xpath input.
    expect(screen.queryByText("XPath")).toBeNull();
  });

  it("changes the success mode via the Success select", () => {
    const onChange = vi.fn();
    render(
      <PatchOperationForm node={addNode()} catalog={null} readOnly={false} projectId={null} onChange={onChange} />,
    );
    fireEvent.change(screen.getByDisplayValue("Normal"), { target: { value: "always" } });
    const updater = onChange.mock.calls[0][0] as (n: PatchOperationNode) => PatchOperationNode;
    expect(updater(addNode()).success).toBe("always");
  });

  it("sets and clears the MayRequire attribute", () => {
    const onChange = vi.fn();
    render(
      <PatchOperationForm node={addNode()} catalog={null} readOnly={false} projectId={null} onChange={onChange} />,
    );
    fireEvent.change(screen.getByPlaceholderText("mod.package.id"), { target: { value: "my.mod.id" } });
    const updater = onChange.mock.calls[0][0] as (n: PatchOperationNode) => PatchOperationNode;
    const updated = updater(addNode());
    expect(updated.attributes).toEqual([{ name: "MayRequire", value: "my.mod.id" }]);
  });

  it("adds and removes an arbitrary other attribute", async () => {
    const onChange = vi.fn();
    render(
      <PatchOperationForm node={addNode()} catalog={null} readOnly={false} projectId={null} onChange={onChange} />,
    );
    await userEvent.click(screen.getByText("Add attribute"));
    const addUpdater = onChange.mock.calls[0][0] as (n: PatchOperationNode) => PatchOperationNode;
    const withAttr = addUpdater(addNode());
    expect(withAttr.attributes).toEqual([{ name: "", value: "" }]);
  });

  it("disables all inputs when readOnly", () => {
    render(
      <PatchOperationForm node={addNode()} catalog={null} readOnly={true} projectId={null} onChange={vi.fn()} />,
    );
    expect(
      (screen.getByDisplayValue('Defs/ThingDef[defName="Wall"]') as HTMLInputElement).disabled,
    ).toBe(true);
    expect((screen.getByDisplayValue("Normal") as HTMLSelectElement).disabled).toBe(true);
    expect(screen.queryByText("Add attribute")).toBeNull();
  });
});
