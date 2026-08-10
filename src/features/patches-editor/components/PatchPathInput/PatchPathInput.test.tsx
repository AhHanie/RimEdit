import { createEvent, fireEvent, screen, waitFor } from "@testing-library/react";
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
    replaceTo: 0,
    items: [],
    totalMatches: 0,
    isTruncated: false,
    diagnostics: [],
    target: { kind: "unsupported" },
    resolvedField: null,
    ...overrides,
  };
}

function textarea(): HTMLTextAreaElement {
  return screen.getByRole("textbox") as HTMLTextAreaElement;
}

/** Byte length of `text` as UTF-8 -- the same conversion the component applies before sending a
 * caret position over IPC. */
function byteLength(text: string): number {
  return new TextEncoder().encode(text).length;
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
        replaceTo: 5,
        items: [
          { insertText: "ThingDef", label: "ThingDef", detail: null, kind: "defType" },
          { insertText: "ThingDefStyleUnlockDef", label: "ThingDefStyleUnlockDef", detail: null, kind: "defType" },
        ],
      }),
    );

    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);

    // The shared completion result is needed by a sibling `PatchValueEditor` regardless of
    // whether this field is ever focused, so the request fires on mount, not only on focus. With
    // no explicit caret yet, the initial caret defaults to the end of the initial value.
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("complete_patch_operation_xpath", {
        projectId: "proj1",
        xpath: "Defs/",
        locale: "en",
        cursorByteOffset: 5,
      });
    });

    fireEvent.focus(textarea());
    expect(await screen.findByText("ThingDef")).toBeTruthy();
    expect(screen.getByText("ThingDefStyleUnlockDef")).toBeTruthy();
  });

  it("updates the textbox immediately while typing without committing to the parent per keystroke", async () => {
    invokeMock.mockResolvedValue(completionResult());

    const onChange = vi.fn();
    render(<PatchPathInput value="" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);

    const input = textarea();
    fireEvent.change(input, { target: { value: "D" } });
    fireEvent.change(input, { target: { value: "De" } });
    fireEvent.change(input, { target: { value: "Def" } });

    // The textbox reflects every keystroke immediately...
    expect(input.value).toBe("Def");
    // ...but none of them reached the parent tree mutation (Plan.md's per-character-serialize
    // fix): only a deliberate commit boundary (idle pause, blur, selection, flush) does.
    expect(onChange).not.toHaveBeenCalled();

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    // Setting `.value` programmatically (as `fireEvent.change` does) moves the caret to the end
    // of the new value, so the last request's caret offset is the end of "Def".
    expect(invokeMock).toHaveBeenCalledWith("complete_patch_operation_xpath", {
      projectId: "proj1",
      xpath: "Def",
      locale: "en",
      cursorByteOffset: 3,
    });
  });

  it("commits the draft once after an idle pause following a typing burst", async () => {
    vi.useFakeTimers();
    try {
      invokeMock.mockResolvedValue(completionResult());
      const onChange = vi.fn();
      render(<PatchPathInput value="" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);
      const input = textarea();

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
    const input = textarea();

    fireEvent.change(input, { target: { value: "Defs/ThingDef" } });
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.blur(input);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenLastCalledWith("Defs/ThingDef");
  });

  it("preserves an embedded newline in the local draft and the committed value", () => {
    invokeMock.mockResolvedValue(completionResult());
    const onChange = vi.fn();
    render(<PatchPathInput value="" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);
    const input = textarea();

    const multiline = 'Defs/\n  ThingDef[\n    defName = "Wall"\n  ]/\n  comps';
    fireEvent.change(input, { target: { value: multiline } });
    // Hard line breaks are stored as XPath text and never joined/normalized -- see Plan.md.
    expect(input.value).toBe(multiline);

    fireEvent.blur(input);
    expect(onChange).toHaveBeenLastCalledWith(multiline);
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
    const input = textarea();
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
    const input = textarea();

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

  it("clears a previous result immediately when the caret moves without a text change", async () => {
    invokeMock.mockResolvedValueOnce(
      completionResult({
        items: [{ insertText: "comps", label: "comps", detail: null, kind: "field" }],
      }),
    );
    render(<PatchPathInput value="Defs/ThingDef/" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);
    fireEvent.focus(textarea());
    await screen.findByText("comps");

    invokeMock.mockResolvedValueOnce(completionResult());
    const input = textarea();
    input.setSelectionRange(3, 3);
    fireEvent.select(input);

    // The stale "comps" suggestion (computed for the old caret position) must disappear
    // immediately -- not linger until the new, still-debouncing request resolves 180ms later.
    expect(screen.queryByText("comps")).toBeNull();
  });

  it("moving the caret alone never commits the draft to the parent", async () => {
    invokeMock.mockResolvedValue(completionResult());
    const onChange = vi.fn();
    render(<PatchPathInput value="Defs/ThingDef" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);
    const input = textarea();

    input.setSelectionRange(3, 3);
    fireEvent.select(input);
    await new Promise((resolve) => setTimeout(resolve, 250));

    expect(onChange).not.toHaveBeenCalled();
  });

  it("sends the caret's UTF-8 byte offset (converted from the DOM's UTF-16 selection index) and reissues on caret movement", async () => {
    invokeMock.mockResolvedValue(completionResult());
    const value = 'Defs/ThingDef[defName="Café"]/comps'; // "é" is 1 UTF-16 unit but 2 UTF-8 bytes
    render(<PatchPathInput value={value} readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    invokeMock.mockClear();
    const input = textarea();
    const stringIndex = value.indexOf("/comps");
    input.setSelectionRange(stringIndex, stringIndex);
    fireEvent.select(input);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenLastCalledWith("complete_patch_operation_xpath", {
        projectId: "proj1",
        xpath: value,
        locale: "en",
        cursorByteOffset: byteLength(value.slice(0, stringIndex)),
      }),
    );
  });

  it("converts a UTF-16 astral-character (surrogate-pair) selection index to the correct UTF-8 byte offset", async () => {
    invokeMock.mockResolvedValue(completionResult());
    const value = 'Defs/ThingDef[defName="\u{1F642}Wall"]/comps'; // U+1F642: 2 UTF-16 units, 4 UTF-8 bytes
    render(<PatchPathInput value={value} readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    invokeMock.mockClear();
    const input = textarea();
    const stringIndex = value.indexOf("/comps");
    input.setSelectionRange(stringIndex, stringIndex);
    fireEvent.select(input);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenLastCalledWith("complete_patch_operation_xpath", {
        projectId: "proj1",
        xpath: value,
        locale: "en",
        cursorByteOffset: byteLength(value.slice(0, stringIndex)),
      }),
    );
  });

  it("splices a selected suggestion's insertText at [replaceFrom, replaceTo)", async () => {
    const value = 'Defs/ThingDef[defName="Wa';
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: 'Defs/ThingDef[defName="'.length,
        replaceTo: value.length,
        items: [{ insertText: 'Wall"]', label: "Wall", detail: "My Mod", kind: "defName" }],
      }),
    );

    const onChange = vi.fn();
    render(<PatchPathInput value={value} readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);

    fireEvent.focus(textarea());
    const suggestion = await screen.findByText("Wall");
    fireEvent.mouseDown(suggestion);

    expect(onChange).toHaveBeenLastCalledWith('Defs/ThingDef[defName="Wall"]');
  });

  it("a chosen suggestion replaces only [replaceFrom, replaceTo), preserving a multiline suffix", async () => {
    const value = 'Defs/ThingDef[defName="Wa"]/\ncomps';
    const replaceFrom = value.indexOf("Wa");
    const replaceTo = replaceFrom + "Wa".length;
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom,
        replaceTo,
        items: [{ insertText: "Wall", label: "Wall", detail: null, kind: "defName" }],
      }),
    );

    const onChange = vi.fn();
    render(<PatchPathInput value={value} readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);

    fireEvent.focus(textarea());
    const suggestion = await screen.findByText("Wall");
    fireEvent.mouseDown(suggestion);

    expect(onChange).toHaveBeenLastCalledWith('Defs/ThingDef[defName="Wall"]/\ncomps');
  });

  it("splices by UTF-16 string index even when a non-ASCII prefix makes byte and character offsets diverge", async () => {
    // "é" is one UTF-16 code unit but two UTF-8 bytes -- a raw `replaceFrom` byte offset applied
    // directly as a JS string index would cut one character too early.
    const prefix = 'Defs/ThingDef[defName="Café"]/';
    const value = `${prefix}gra`;
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: byteLength(prefix),
        replaceTo: byteLength(value),
        items: [{ insertText: "graphicData", label: "graphicData", detail: null, kind: "field" }],
      }),
    );

    const onChange = vi.fn();
    render(<PatchPathInput value={value} readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);

    fireEvent.focus(textarea());
    const suggestion = await screen.findByText("graphicData");
    fireEvent.mouseDown(suggestion);

    expect(onChange).toHaveBeenLastCalledWith(`${prefix}graphicData`);
  });

  it("renders and splices a structural 'li' completion for a listOfLi object field", async () => {
    // Proves PatchPathInput's rendering/splicing needs no depth-specific handling: a structural
    // `listItem` suggestion (offered several levels into a nested schema on the Rust side) is
    // spliced exactly like a `field`/`defType` one.
    const value = "Defs/ThingDef/comps/";
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: value.length,
        replaceTo: value.length,
        items: [{ insertText: "li", label: "li", detail: null, kind: "listItem" }],
      }),
    );

    const onChange = vi.fn();
    render(<PatchPathInput value={value} readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);

    fireEvent.focus(textarea());
    const suggestion = await screen.findByText("li");
    fireEvent.mouseDown(suggestion);

    expect(onChange).toHaveBeenLastCalledWith("Defs/ThingDef/comps/li");
  });

  it("renders and splices a nested field completion several levels deep", async () => {
    const value = "Defs/ThingDef/graphicData/texP";
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: "Defs/ThingDef/graphicData/".length,
        replaceTo: value.length,
        items: [{ insertText: "texPath", label: "texPath", detail: null, kind: "field" }],
      }),
    );

    const onChange = vi.fn();
    render(<PatchPathInput value={value} readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);

    fireEvent.focus(textarea());
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
    fireEvent.focus(textarea());

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
      cursorByteOffset: 5,
    });

    // Refocusing with unchanged text and no locale change must not refetch (mere refocus doesn't
    // change any of the shared hook's reactive inputs).
    fireEvent.blur(textarea());
    fireEvent.focus(textarea());
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
      cursorByteOffset: 5,
    });
  });

  it("does not fetch completions, render an interactive dropdown, or disable (vs. read-only) the field when readOnly", async () => {
    invokeMock.mockResolvedValue(completionResult());

    render(<PatchPathInput value="Defs/ThingDef" readOnly label="XPath" projectId="proj1" onChange={vi.fn()} />);

    const input = textarea();
    // `readOnly`, not `disabled`: a read-only/source-location file's XPath must remain selectable,
    // copyable plain text rather than an inert control (Plan.md's contract).
    expect(input.readOnly).toBe(true);
    expect(input.disabled).toBe(false);
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("forces dir=ltr on the input regardless of app locale direction", () => {
    // XPath is machine-readable syntax, not natural-language prose -- this must stay LTR even
    // once a future RTL locale flips `dir` on `<html>` (docs/i18n/issues/08-editor-and-patch-ui-
    // migration.md's "keep code editor/XML/XPath controls dir=ltr by semantic policy").
    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);
    expect(textarea().getAttribute("dir")).toBe("ltr");
  });

  it("does not fetch completions when projectId is absent", async () => {
    invokeMock.mockResolvedValue(completionResult());

    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId={null} onChange={vi.fn()} />);

    fireEvent.focus(textarea());
    fireEvent.change(textarea(), { target: { value: "Defs/T" } });

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

    expect(textarea().value).toBe("Defs/ThingDef/statBases");
  });

  it("does not clobber a focused in-progress draft with an unrelated external value change", async () => {
    invokeMock.mockResolvedValue(completionResult());
    const { rerender } = render(
      <PatchPathInput value="Defs/ThingDef" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    const input = textarea();
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "Defs/ThingDef/labe" } });

    // An external change to `value` arrives (e.g. a sibling field's flush reconciling first)
    // while this field is focused with its own uncommitted draft -- the draft must survive.
    rerender(
      <PatchPathInput value="Defs/ThingDef/statBases" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />,
    );

    expect(input.value).toBe("Defs/ThingDef/labe");
  });

  it("Enter does not accept a highlighted suggestion (it is left to insert a newline by default)", async () => {
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: 5,
        replaceTo: 5,
        items: [{ insertText: "ThingDef", label: "ThingDef", detail: null, kind: "defType" }],
      }),
    );
    const onChange = vi.fn();
    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);
    const input = textarea();
    fireEvent.focus(input);
    await screen.findByText("ThingDef");
    fireEvent.keyDown(input, { key: "ArrowDown" }); // highlight the suggestion first

    const enterEvent = createEvent.keyDown(input, { key: "Enter" });
    fireEvent(input, enterEvent);

    expect(enterEvent.defaultPrevented).toBe(false);
    expect(onChange).not.toHaveBeenCalled();
  });

  it("ArrowDown highlights a suggestion and Tab accepts it, blocking focus traversal", async () => {
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: 5,
        replaceTo: 5,
        items: [{ insertText: "ThingDef", label: "ThingDef", detail: null, kind: "defType" }],
      }),
    );
    const onChange = vi.fn();
    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);
    const input = textarea();
    fireEvent.focus(input);
    await screen.findByText("ThingDef");

    fireEvent.keyDown(input, { key: "ArrowDown" });
    const tabEvent = createEvent.keyDown(input, { key: "Tab" });
    fireEvent(input, tabEvent);

    expect(tabEvent.defaultPrevented).toBe(true);
    expect(onChange).toHaveBeenLastCalledWith("Defs/ThingDef");
  });

  it("Tab does not intercept focus traversal when no suggestion is highlighted", async () => {
    invokeMock.mockResolvedValue(
      completionResult({ items: [{ insertText: "ThingDef", label: "ThingDef", detail: null, kind: "defType" }] }),
    );
    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);
    const input = textarea();
    fireEvent.focus(input);
    await screen.findByText("ThingDef");

    const tabEvent = createEvent.keyDown(input, { key: "Tab" }); // no ArrowDown first -- activeIndex is -1
    fireEvent(input, tabEvent);

    expect(tabEvent.defaultPrevented).toBe(false);
  });

  it("Escape closes the dropdown", async () => {
    invokeMock.mockResolvedValue(
      completionResult({ items: [{ insertText: "ThingDef", label: "ThingDef", detail: null, kind: "defType" }] }),
    );
    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={vi.fn()} />);
    const input = textarea();
    fireEvent.focus(input);
    await screen.findByText("ThingDef");

    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByText("ThingDef")).toBeNull();
  });

  it("a mouse selection also accepts a suggestion", async () => {
    invokeMock.mockResolvedValue(
      completionResult({
        replaceFrom: 5,
        replaceTo: 5,
        items: [{ insertText: "ThingDef", label: "ThingDef", detail: null, kind: "defType" }],
      }),
    );
    const onChange = vi.fn();
    render(<PatchPathInput value="Defs/" readOnly={false} label="XPath" projectId="proj1" onChange={onChange} />);
    fireEvent.focus(textarea());
    const suggestion = await screen.findByText("ThingDef");
    fireEvent.mouseDown(suggestion);

    expect(onChange).toHaveBeenLastCalledWith("Defs/ThingDef");
  });
});
