import { fireEvent, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { useLocale } from "../../../../i18n/LocaleProvider";
import { PatchPathInput } from "./PatchPathInput";
import type { XPathCompletionResult } from "../../types/xpathCompletion";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Mocks only `useLocale` (keeping the real `LocaleProvider`/`I18nextProvider` tree the other
// hooks in this component rely on) so tests can drive an app-wide locale switch without depending
// on `SUPPORTED_LOCALES` actually listing a second locale yet.
vi.mock("../../../../i18n/LocaleProvider", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../../i18n/LocaleProvider")>();
  return { ...actual, useLocale: vi.fn() };
});

const invokeMock = vi.mocked(invoke);
const mockUseLocale = vi.mocked(useLocale);

function completionResult(overrides: Partial<XPathCompletionResult> = {}): XPathCompletionResult {
  return {
    replaceFrom: 0,
    items: [],
    totalMatches: 0,
    isTruncated: false,
    diagnostics: [],
    target: { kind: "unsupported" },
    resolvedField: null,
    ...overrides,
  };
}

beforeEach(() => {
  invokeMock.mockReset();
  mockUseLocale.mockReturnValue({ locale: "en", direction: "ltr", changeLocale: vi.fn() });
});

describe("PatchPathInput", () => {
  it("fetches completions on mount and renders them once focused", async () => {
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

    // The shared completion result is needed by a sibling `PatchValueEditor` regardless of
    // whether this field is ever focused, so the request fires on mount, not only on focus.
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("complete_patch_operation_xpath", {
        projectId: "proj1",
        xpath: "Defs/",
        locale: "en",
      });
    });

    fireEvent.focus(screen.getByRole("textbox"));
    expect(await screen.findByText("ThingDef")).toBeTruthy();
    expect(screen.getByText("ThingDefStyleUnlockDef")).toBeTruthy();
  });

  it("updates the textbox immediately while typing without committing to the parent per keystroke", async () => {
    invokeMock.mockResolvedValue(completionResult());

    const onChange = vi.fn();
    render(<PatchPathInput value="" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);

    const input = screen.getByRole("textbox") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "D" } });
    fireEvent.change(input, { target: { value: "De" } });
    fireEvent.change(input, { target: { value: "Def" } });

    // The textbox reflects every keystroke immediately...
    expect(input.value).toBe("Def");
    // ...but none of them reached the parent tree mutation (Plan.md's per-character-serialize
    // fix): only a deliberate commit boundary (idle pause, blur, selection, flush) does.
    expect(onChange).not.toHaveBeenCalled();

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    expect(invokeMock).toHaveBeenCalledWith("complete_patch_operation_xpath", {
      projectId: "proj1",
      xpath: "Def",
      locale: "en",
    });
  });

  it("commits the draft once after an idle pause following a typing burst", async () => {
    vi.useFakeTimers();
    try {
      invokeMock.mockResolvedValue(completionResult());
      const onChange = vi.fn();
      render(<PatchPathInput value="" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);
      const input = screen.getByRole("textbox");

      fireEvent.change(input, { target: { value: "D" } });
      vi.advanceTimersByTime(100);
      fireEvent.change(input, { target: { value: "De" } });
      vi.advanceTimersByTime(100);
      fireEvent.change(input, { target: { value: "Def" } });

      expect(onChange).not.toHaveBeenCalled();

      vi.advanceTimersByTime(500);
      expect(onChange).toHaveBeenCalledTimes(1);
      expect(onChange).toHaveBeenLastCalledWith("Def");
    } finally {
      vi.useRealTimers();
    }
  });

  it("commits the draft immediately on blur", () => {
    invokeMock.mockResolvedValue(completionResult());
    const onChange = vi.fn();
    render(<PatchPathInput value="" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);
    const input = screen.getByRole("textbox");

    fireEvent.change(input, { target: { value: "Defs/ThingDef" } });
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.blur(input);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenLastCalledWith("Defs/ThingDef");
  });

  it("commits the pending draft exactly once when flushed via the draft-flush registry", () => {
    invokeMock.mockResolvedValue(completionResult());
    const onChange = vi.fn();
    let flush: (() => void) | undefined;
    const registerDraftFlush = (fn: () => void) => {
      flush = fn;
      return () => {
        flush = undefined;
      };
    };

    render(
      <PatchPathInput
        value=""
        readOnly={false}
        label="XPath"
        projectId="proj1"
        onChange={onChange}
        registerDraftFlush={registerDraftFlush}
      />,
    );
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "Defs/ThingDef" } });
    expect(onChange).not.toHaveBeenCalled();

    flush?.();
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenLastCalledWith("Defs/ThingDef");

    // Flushing again with nothing new to commit is a no-op, not a redundant call.
    flush?.();
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it("reports the completion result upward via onCompletionResult", async () => {
    const result = completionResult({
      target: { kind: "def", defType: "ThingDef", defName: "Wall" },
    });
    invokeMock.mockResolvedValue(result);
    const onCompletionResult = vi.fn();

    render(
      <PatchPathInput
        value='Defs/ThingDef[defName="Wall"]'
        readOnly={false}
        label="XPath"
        projectId="proj1"
        onChange={vi.fn()}
        onCompletionResult={onCompletionResult}
      />,
    );

    await waitFor(() => expect(onCompletionResult).toHaveBeenCalledWith(result));
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
    invokeMock.mockResolvedValue(completionResult());

    resolveFirst(
      completionResult({
        items: [{ insertText: "STALE", label: "STALE", detail: null, kind: "defType" }],
      }),
    );
    fireEvent.focus(input);
    await new Promise((resolve) => setTimeout(resolve, 250));

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

  it("renders and splices a structural 'li' completion for a listOfLi object field", async () => {
    // Proves PatchPathInput's rendering/splicing needs no depth-specific handling: a structural
    // `listItem` suggestion (offered several levels into a nested schema on the Rust side) is
    // spliced exactly like a `field`/`defType` one.
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: "Defs/ThingDef/comps/".length,
        items: [{ insertText: "li", label: "li", detail: null, kind: "listItem" }],
      }),
    );

    const onChange = vi.fn();
    render(
      <PatchPathInput
        value="Defs/ThingDef/comps/"
        readOnly={false}
        label="XPath"
        projectId="proj1"
        onChange={onChange}
      />,
    );

    fireEvent.focus(screen.getByRole("textbox"));
    const suggestion = await screen.findByText("li");
    fireEvent.mouseDown(suggestion);

    expect(onChange).toHaveBeenLastCalledWith("Defs/ThingDef/comps/li");
  });

  it("renders and splices a nested field completion several levels deep", async () => {
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: "Defs/ThingDef/graphicData/".length,
        items: [{ insertText: "texPath", label: "texPath", detail: null, kind: "field" }],
      }),
    );

    const onChange = vi.fn();
    render(
      <PatchPathInput
        value="Defs/ThingDef/graphicData/texP"
        readOnly={false}
        label="XPath"
        projectId="proj1"
        onChange={onChange}
      />,
    );

    fireEvent.focus(screen.getByRole("textbox"));
    const suggestion = await screen.findByText("texPath");
    fireEvent.mouseDown(suggestion);

    expect(onChange).toHaveBeenLastCalledWith("Defs/ThingDef/graphicData/texPath");
  });

  it("renders a truncated-results status when the completion result is capped", async () => {
    invokeMock.mockResolvedValue(
      completionResult({
        items: [{ insertText: "ThingDef", label: "ThingDef", detail: null, kind: "defType" }],
        totalMatches: 500,
        isTruncated: true,
      }),
    );

    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);
    fireEvent.focus(screen.getByRole("textbox"));

    expect(await screen.findByRole("status")).toBeTruthy();
  });

  it("renders diagnostics returned by the completion result", async () => {
    invokeMock.mockResolvedValue(
      completionResult({
        diagnostics: [
          // Uses a code with no catalog entry so this test exercises the generic
          // "render whatever diagnostics come back" path without coupling to
          // diagnostics.json's translated text for a real code (see renderDiagnostic's
          // message-fallback priority in src/i18n/diagnostics.ts).
          { severity: "warning", code: "xpath_autocomplete_test_only_code", message: "inherited field warning" },
        ],
      }),
    );

    render(
      <PatchPathInput value="Defs/ThingDef/statBases" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />,
    );

    expect(await screen.findByText("inherited field warning")).toBeTruthy();
  });

  it("refetches immediately when the locale changes, even with unchanged xpath text", async () => {
    invokeMock.mockResolvedValue(completionResult());

    const { rerender } = render(
      <PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />,
    );

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    expect(invokeMock).toHaveBeenLastCalledWith("complete_patch_operation_xpath", {
      projectId: "proj1",
      xpath: "Defs/",
      locale: "en",
    });

    // Refocusing with unchanged text and no locale change must not refetch (mere refocus doesn't
    // change any of the shared hook's reactive inputs).
    fireEvent.blur(screen.getByRole("textbox"));
    fireEvent.focus(screen.getByRole("textbox"));
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(invokeMock).toHaveBeenCalledTimes(1);

    // Simulate an app-wide locale switch (e.g. via the settings panel) -- the xpath text itself
    // never changes, but the shared hook treats locale as a reactive input and refetches.
    mockUseLocale.mockReturnValue({ locale: "fr", direction: "ltr", changeLocale: vi.fn() });
    rerender(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
    expect(invokeMock).toHaveBeenLastCalledWith("complete_patch_operation_xpath", {
      projectId: "proj1",
      xpath: "Defs/",
      locale: "fr",
    });
  });

  it("does not fetch completions or render an interactive dropdown when readOnly", async () => {
    invokeMock.mockResolvedValue(completionResult());

    render(<PatchPathInput value="Defs/ThingDef" readOnly label="XPath" projectId="proj1" onChange={vi.fn()} />);

    const input = screen.getByRole("textbox") as HTMLInputElement;
    expect(input.disabled).toBe(true);
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("forces dir=ltr on the input regardless of app locale direction", () => {
    // XPath is machine-readable syntax, not natural-language prose -- this must stay LTR even
    // once a future RTL locale flips `dir` on `<html>` (docs/i18n/issues/08-editor-and-patch-ui-
    // migration.md's "keep code editor/XML/XPath controls dir=ltr by semantic policy").
    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);
    expect(screen.getByRole("textbox").getAttribute("dir")).toBe("ltr");
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

  it("adopts an external value change when the field isn't focused", async () => {
    invokeMock.mockResolvedValue(completionResult());
    const { rerender } = render(
      <PatchPathInput value="Defs/ThingDef" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    // Simulate an external change (e.g. undo) while unfocused.
    rerender(
      <PatchPathInput value="Defs/ThingDef/statBases" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />,
    );

    expect((screen.getByRole("textbox") as HTMLInputElement).value).toBe("Defs/ThingDef/statBases");
  });

  it("does not clobber a focused in-progress draft with an unrelated external value change", async () => {
    invokeMock.mockResolvedValue(completionResult());
    const { rerender } = render(
      <PatchPathInput value="Defs/ThingDef" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    const input = screen.getByRole("textbox") as HTMLInputElement;
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "Defs/ThingDef/labe" } });

    // An external change to `value` arrives (e.g. a sibling field's flush reconciling first)
    // while this field is focused with its own uncommitted draft -- the draft must survive.
    rerender(
      <PatchPathInput value="Defs/ThingDef/statBases" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />,
    );

    expect(input.value).toBe("Defs/ThingDef/labe");
  });
});
