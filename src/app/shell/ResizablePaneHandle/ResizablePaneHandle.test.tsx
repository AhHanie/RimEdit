import { render, screen, fireEvent } from "@testing-library/react";
import { ResizablePaneHandle } from "./ResizablePaneHandle";

function defaultProps(
  overrides: Partial<Parameters<typeof ResizablePaneHandle>[0]> = {},
) {
  return {
    width: 300,
    minWidth: 220,
    maxWidth: 520,
    defaultWidth: 280,
    onChange: vi.fn(),
    ...overrides,
  };
}

function getHandle() {
  return screen.getByRole("separator");
}

describe("ResizablePaneHandle", () => {
  it("renders with correct ARIA attributes", () => {
    render(
      <ResizablePaneHandle
        {...defaultProps({ width: 300, minWidth: 220, maxWidth: 520 })}
      />,
    );
    const handle = getHandle();
    expect(handle.getAttribute("aria-orientation")).toBe("vertical");
    expect(handle.getAttribute("aria-valuemin")).toBe("220");
    expect(handle.getAttribute("aria-valuemax")).toBe("520");
    expect(handle.getAttribute("aria-valuenow")).toBe("300");
    expect(handle.getAttribute("tabindex")).toBe("0");
  });

  it("ArrowRight increases width by 16", () => {
    const onChange = vi.fn();
    render(<ResizablePaneHandle {...defaultProps({ width: 300, onChange })} />);
    fireEvent.keyDown(getHandle(), { key: "ArrowRight" });
    expect(onChange).toHaveBeenCalledWith(316);
  });

  it("ArrowLeft decreases width by 16", () => {
    const onChange = vi.fn();
    render(<ResizablePaneHandle {...defaultProps({ width: 300, onChange })} />);
    fireEvent.keyDown(getHandle(), { key: "ArrowLeft" });
    expect(onChange).toHaveBeenCalledWith(284);
  });

  it("Shift+ArrowRight increases width by 48", () => {
    const onChange = vi.fn();
    render(<ResizablePaneHandle {...defaultProps({ width: 300, onChange })} />);
    fireEvent.keyDown(getHandle(), { key: "ArrowRight", shiftKey: true });
    expect(onChange).toHaveBeenCalledWith(348);
  });

  it("Shift+ArrowLeft decreases width by 48", () => {
    const onChange = vi.fn();
    render(<ResizablePaneHandle {...defaultProps({ width: 300, onChange })} />);
    fireEvent.keyDown(getHandle(), { key: "ArrowLeft", shiftKey: true });
    expect(onChange).toHaveBeenCalledWith(252);
  });

  it("Home sets width to minWidth", () => {
    const onChange = vi.fn();
    render(
      <ResizablePaneHandle
        {...defaultProps({ width: 300, minWidth: 220, onChange })}
      />,
    );
    fireEvent.keyDown(getHandle(), { key: "Home" });
    expect(onChange).toHaveBeenCalledWith(220);
  });

  it("End sets width to maxWidth", () => {
    const onChange = vi.fn();
    render(
      <ResizablePaneHandle
        {...defaultProps({ width: 300, maxWidth: 520, onChange })}
      />,
    );
    fireEvent.keyDown(getHandle(), { key: "End" });
    expect(onChange).toHaveBeenCalledWith(520);
  });

  it("Enter resets width to defaultWidth", () => {
    const onChange = vi.fn();
    render(
      <ResizablePaneHandle
        {...defaultProps({ width: 300, defaultWidth: 280, onChange })}
      />,
    );
    fireEvent.keyDown(getHandle(), { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith(280);
  });

  it("double-click resets width to defaultWidth", () => {
    const onChange = vi.fn();
    render(
      <ResizablePaneHandle
        {...defaultProps({ width: 300, defaultWidth: 280, onChange })}
      />,
    );
    fireEvent.doubleClick(getHandle());
    expect(onChange).toHaveBeenCalledWith(280);
  });

  it("ArrowRight clamps at maxWidth", () => {
    const onChange = vi.fn();
    render(
      <ResizablePaneHandle
        {...defaultProps({ width: 515, maxWidth: 520, onChange })}
      />,
    );
    fireEvent.keyDown(getHandle(), { key: "ArrowRight" });
    expect(onChange).toHaveBeenCalledWith(520);
  });

  it("ArrowLeft clamps at minWidth", () => {
    const onChange = vi.fn();
    render(
      <ResizablePaneHandle
        {...defaultProps({ width: 225, minWidth: 220, onChange })}
      />,
    );
    fireEvent.keyDown(getHandle(), { key: "ArrowLeft" });
    expect(onChange).toHaveBeenCalledWith(220);
  });

  it("pointercancel restores body styles", () => {
    render(<ResizablePaneHandle {...defaultProps()} />);
    const handle = getHandle();
    // jsdom does not implement pointer capture - stub the methods
    handle.setPointerCapture = vi.fn();
    handle.releasePointerCapture = vi.fn();
    fireEvent.pointerDown(handle, { clientX: 300, pointerId: 1 });
    expect(document.body.style.cursor).toBe("col-resize");
    fireEvent.pointerCancel(handle);
    expect(document.body.style.cursor).toBe("");
    expect(document.body.style.userSelect).toBe("");
  });
});
