import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { PatchPathInput } from "./PatchPathInput";
import type { XPathCompletionResult } from "../../types/xpathCompletion";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

function completionResult(overrides: Partial<XPathCompletionResult> = {}): XPathCompletionResult {
  return {
    replaceFrom: 0,
    items: [],
    diagnostics: [],
    target: { kind: "unsupported" },
    resolvedField: null,
    ...overrides,
  };
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("PatchPathInput", () => {
  it("fetches completions on focus and renders suggestions", async () => {
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: 5,
        items: [
          { insertText: "ThingDef", label: "ThingDef", detail: null, kind: "defType" },
          { insertText: "ThingDefStyleUnlockDef", label: "ThingDefStyleUnlockDef", detail: null, kind: "defType" },
        ],
      }),
    );

    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);

    fireEvent.focus(screen.getByRole("textbox"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("complete_patch_operation_xpath", {
        projectId: "proj1",
        xpath: "Defs/",
      });
    });

    expect(await screen.findByText("ThingDef")).toBeTruthy();
    expect(screen.getByText("ThingDefStyleUnlockDef")).toBeTruthy();
  });

  it("debounces completion requests while typing", async () => {
    invokeMock.mockResolvedValue(completionResult());

    const onChange = vi.fn();
    render(<PatchPathInput value="" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);

    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "D" } });
    fireEvent.change(input, { target: { value: "De" } });
    fireEvent.change(input, { target: { value: "Def" } });

    expect(onChange).toHaveBeenCalledTimes(3);
    expect(onChange).toHaveBeenLastCalledWith("Def");

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledTimes(1);
    });
    expect(invokeMock).toHaveBeenCalledWith("complete_patch_operation_xpath", {
      projectId: "proj1",
      xpath: "Def",
    });
  });

  it("discards a stale response that resolves while a newer request is still debouncing", async () => {
    let resolveFirst: (value: XPathCompletionResult) => void = () => {};
    const firstPromise = new Promise<XPathCompletionResult>((resolve) => {
      resolveFirst = resolve;
    });
    invokeMock.mockImplementationOnce(() => firstPromise);

    render(<PatchPathInput value="" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);
    const input = screen.getByRole("textbox");

    fireEvent.change(input, { target: { value: "De" } });
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    // A second edit starts a new debounce window before the first request has resolved. The fix
    // bumps the "current request" id synchronously when this second request is scheduled, not
    // only once its own debounce timer fires -- otherwise the stale response below would still
    // look current for this whole 180ms window.
    fireEvent.change(input, { target: { value: "Def" } });

    resolveFirst(
      completionResult({
        items: [{ insertText: "STALE", label: "STALE", detail: null, kind: "defType" }],
      }),
    );
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.queryByText("STALE")).toBeNull();
  });

  it("splices a selected suggestion's insertText at replaceFrom", async () => {
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: "Defs/ThingDef[defName=\"".length,
        items: [{ insertText: 'Wall"]', label: "Wall", detail: "My Mod", kind: "defName" }],
      }),
    );

    const onChange = vi.fn();
    render(
      <PatchPathInput
        value='Defs/ThingDef[defName="Wa'
        readOnly={false}
        label="XPath"
        projectId="proj1"
        onChange={onChange}
      />,
    );

    fireEvent.focus(screen.getByRole("textbox"));
    const suggestion = await screen.findByText("Wall");
    fireEvent.mouseDown(suggestion);

    expect(onChange).toHaveBeenLastCalledWith('Defs/ThingDef[defName="Wall"]');
  });

  it("splices by UTF-16 string index even when a non-ASCII prefix makes byte and character offsets diverge", async () => {
    // "é" is one UTF-16 code unit but two UTF-8 bytes -- a raw `replaceFrom` byte offset applied
    // directly as a JS string index would cut one character too early.
    const prefix = 'Defs/ThingDef[defName="Café"]/';
    const byteOffset = new TextEncoder().encode(prefix).length;
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: byteOffset,
        items: [{ insertText: "graphicData", label: "graphicData", detail: null, kind: "field" }],
      }),
    );

    const onChange = vi.fn();
    render(
      <PatchPathInput value={`${prefix}gra`} readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />,
    );

    fireEvent.focus(screen.getByRole("textbox"));
    const suggestion = await screen.findByText("graphicData");
    fireEvent.mouseDown(suggestion);

    expect(onChange).toHaveBeenLastCalledWith(`${prefix}graphicData`);
  });

  it("renders diagnostics returned by the completion result", async () => {
    invokeMock.mockResolvedValue(
      completionResult({
        diagnostics: [
          { severity: "warning", code: "xpath_autocomplete_inherited_field", message: "inherited field warning" },
        ],
      }),
    );

    render(
      <PatchPathInput value="Defs/ThingDef/statBases" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />,
    );

    fireEvent.focus(screen.getByRole("textbox"));

    expect(await screen.findByText("inherited field warning")).toBeTruthy();
  });

  it("does not fetch completions when readOnly", () => {
    invokeMock.mockResolvedValue(completionResult());

    render(<PatchPathInput value="Defs/ThingDef" readOnly label="XPath" projectId="proj1" onChange={vi.fn()} />);

    const input = screen.getByRole("textbox") as HTMLInputElement;
    expect(input.disabled).toBe(true);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("does not fetch completions when projectId is absent", async () => {
    invokeMock.mockResolvedValue(completionResult());

    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId={null} onChange={vi.fn()} />);

    fireEvent.focus(screen.getByRole("textbox"));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Defs/T" } });

    // Give the debounce window a chance to fire, then confirm it never called invoke.
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
