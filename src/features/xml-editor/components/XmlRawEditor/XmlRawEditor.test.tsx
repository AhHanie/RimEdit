import { createRef } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { XmlRawEditor, type XmlRawEditorHandle } from "./XmlRawEditor";

// CodeMirror uses ResizeObserver for viewport tracking; polyfill for JSDOM.
beforeAll(() => {
  if (!globalThis.ResizeObserver) {
    globalThis.ResizeObserver = class ResizeObserver {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
});

describe("XmlRawEditor", () => {
  it("renders an accessible raw XML editor", () => {
    render(<XmlRawEditor value="<Defs/>" onChange={vi.fn()} />);
    expect(screen.getByLabelText("Raw XML editor")).toBeTruthy();
  });

  it("renders the initial value as content", () => {
    render(<XmlRawEditor value="<Defs/>" onChange={vi.fn()} />);
    expect(screen.getByLabelText("Raw XML editor").textContent).toContain(
      "<Defs/>",
    );
  });

  it("updates content when the value prop changes", async () => {
    const { rerender } = render(
      <XmlRawEditor value="<Defs/>" onChange={vi.fn()} />,
    );
    rerender(<XmlRawEditor value="<NewDefs/>" onChange={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByLabelText("Raw XML editor").textContent).toContain(
        "<NewDefs/>",
      );
    });
  });

  it("sets contenteditable=false when readOnly", () => {
    render(<XmlRawEditor value="<Defs/>" onChange={vi.fn()} readOnly />);
    expect(
      screen.getByLabelText("Raw XML editor").getAttribute("contenteditable"),
    ).toBe("false");
  });

  it("restores editability when readOnly is toggled off", async () => {
    const { rerender } = render(
      <XmlRawEditor value="<Defs/>" onChange={vi.fn()} readOnly />,
    );
    rerender(
      <XmlRawEditor value="<Defs/>" onChange={vi.fn()} readOnly={false} />,
    );
    await waitFor(() => {
      expect(
        screen.getByLabelText("Raw XML editor").getAttribute("contenteditable"),
      ).toBe("true");
    });
  });

  it("calls onChange when the user edits content", async () => {
    const handleRef = createRef<XmlRawEditorHandle>();
    const onChange = vi.fn();
    render(
      <XmlRawEditor ref={handleRef} value="<Defs/>" onChange={onChange} />,
    );

    const view = await waitFor(() => {
      const v = handleRef.current?.view;
      if (!v) throw new Error("view not ready");
      return v;
    });

    // Simulate a user keystroke via dispatch - no syncAnnotation → listener fires.
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: "<EditedDefs/>" },
    });

    expect(onChange).toHaveBeenCalledWith("<EditedDefs/>");
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  describe("keyboard shortcut interception (onShortcut)", () => {
    async function setupWithShortcut() {
      const onShortcut = vi.fn().mockReturnValue(true);
      const onChange = vi.fn();
      const handleRef = createRef<XmlRawEditorHandle>();
      render(
        <XmlRawEditor
          ref={handleRef}
          value="<Defs/>"
          onChange={onChange}
          onShortcut={onShortcut}
        />,
      );
      const view = await waitFor(() => {
        const v = handleRef.current?.view;
        if (!v) throw new Error("view not ready");
        return v;
      });
      return { onShortcut, onChange, view };
    }

    it("calls onShortcut('undo') for Ctrl+Z", async () => {
      const { onShortcut, view } = await setupWithShortcut();
      fireEvent.keyDown(view.contentDOM, { key: "z", ctrlKey: true });
      expect(onShortcut).toHaveBeenCalledWith("undo");
    });

    it("calls onShortcut('redo') for Ctrl+Y", async () => {
      const { onShortcut, view } = await setupWithShortcut();
      fireEvent.keyDown(view.contentDOM, { key: "y", ctrlKey: true });
      expect(onShortcut).toHaveBeenCalledWith("redo");
    });

    it("calls onShortcut('redo') for Ctrl+Shift+Z", async () => {
      const { onShortcut, view } = await setupWithShortcut();
      // keyCode:90 is required so CodeMirror's base-keycode lookup finds "z" and
      // matches the "Mod-Shift-z" binding via the modifier-prefix path.
      fireEvent.keyDown(view.contentDOM, {
        key: "Z",
        code: "KeyZ",
        keyCode: 90,
        ctrlKey: true,
        shiftKey: true,
      });
      expect(onShortcut).toHaveBeenCalledWith("redo");
    });

    it("calls onShortcut('save') for Ctrl+S", async () => {
      const { onShortcut, view } = await setupWithShortcut();
      fireEvent.keyDown(view.contentDOM, { key: "s", ctrlKey: true });
      expect(onShortcut).toHaveBeenCalledWith("save");
    });

    it("calls onShortcut('close') for Ctrl+W", async () => {
      const { onShortcut, view } = await setupWithShortcut();
      fireEvent.keyDown(view.contentDOM, { key: "w", ctrlKey: true });
      expect(onShortcut).toHaveBeenCalledWith("close");
    });

    it("does not call onChange when onShortcut handles Ctrl+Z (prevents CodeMirror internal undo)", async () => {
      const { onShortcut, onChange, view } = await setupWithShortcut();

      // Simulate a user edit so CodeMirror has history to undo.
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "<Edited/>" },
      });
      expect(onChange).toHaveBeenCalledWith("<Edited/>");
      onChange.mockClear();

      // Ctrl+Z - our Prec.highest keymap should intercept before basicSetup's undo runs.
      fireEvent.keyDown(view.contentDOM, { key: "z", ctrlKey: true });

      expect(onShortcut).toHaveBeenCalledWith("undo");
      // CodeMirror internal undo must NOT have run - no onChange call with reverted content.
      expect(onChange).not.toHaveBeenCalled();
      // Editor content stays at the post-edit value.
      expect(view.state.doc.toString()).toBe("<Edited/>");
    });
  });

  it("does not call onChange when the value prop is updated externally (e.g. toolbar undo)", async () => {
    const handleRef = createRef<XmlRawEditorHandle>();
    const onChange = vi.fn();
    const { rerender } = render(
      <XmlRawEditor ref={handleRef} value="<Before/>" onChange={onChange} />,
    );

    // Wait for the view to mount so the sync effect has somewhere to dispatch to.
    await waitFor(() => {
      if (!handleRef.current?.view) throw new Error("view not ready");
    });

    // Simulate an external controlled update (undo, redo, form edit flush, etc.)
    rerender(
      <XmlRawEditor ref={handleRef} value="<After/>" onChange={onChange} />,
    );

    // Content should update in the editor...
    await waitFor(() => {
      expect(screen.getByLabelText("Raw XML editor").textContent).toContain(
        "<After/>",
      );
    });

    // ...but onChange must NOT have been called (syncAnnotation suppresses it).
    expect(onChange).not.toHaveBeenCalled();
  });
});
