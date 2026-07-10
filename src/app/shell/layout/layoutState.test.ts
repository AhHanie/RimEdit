import { describe, it, expect } from "vitest";
import {
  clampExplorerWidth,
  parseLayoutState,
  normalizeLayoutState,
  serializeLayoutState,
  LAYOUT_MIN_EXPLORER_WIDTH,
  LAYOUT_MAX_EXPLORER_WIDTH,
  LAYOUT_MAX_EXPLORER_RATIO,
  LAYOUT_DEFAULTS,
} from "./layoutState";

describe("clampExplorerWidth", () => {
  it("clamps below min to min", () => {
    expect(clampExplorerWidth(100, 1440)).toBe(LAYOUT_MIN_EXPLORER_WIDTH);
  });

  it("clamps above absolute max to absolute max", () => {
    expect(clampExplorerWidth(9999, 9999)).toBe(LAYOUT_MAX_EXPLORER_WIDTH);
  });

  it("clamps above ratio max to ratio max", () => {
    const workspaceWidth = 800;
    const ratioMax = Math.floor(workspaceWidth * LAYOUT_MAX_EXPLORER_RATIO);
    expect(clampExplorerWidth(LAYOUT_MAX_EXPLORER_WIDTH, workspaceWidth)).toBe(
      ratioMax,
    );
  });

  it("uses absolute max when ratio max exceeds it", () => {
    const workspaceWidth = 9999;
    expect(clampExplorerWidth(LAYOUT_MAX_EXPLORER_WIDTH, workspaceWidth)).toBe(
      LAYOUT_MAX_EXPLORER_WIDTH,
    );
  });

  it("passes through valid width unchanged", () => {
    expect(clampExplorerWidth(300, 1440)).toBe(300);
  });

  it("ignores ratio cap when workspaceWidth is 0", () => {
    expect(clampExplorerWidth(LAYOUT_MAX_EXPLORER_WIDTH, 0)).toBe(
      LAYOUT_MAX_EXPLORER_WIDTH,
    );
  });

  it("ignores ratio cap when workspaceWidth is negative", () => {
    expect(clampExplorerWidth(300, -1)).toBe(300);
  });

  it("protects editor minimum: explorer cannot exceed 45% of workspace", () => {
    const workspaceWidth = 600;
    const result = clampExplorerWidth(400, workspaceWidth);
    expect(result).toBeLessThanOrEqual(
      Math.floor(workspaceWidth * LAYOUT_MAX_EXPLORER_RATIO),
    );
  });

  it("on very narrow windows, ratio cap wins over stated minimum", () => {
    // 45% of 400 = 180, which is below LAYOUT_MIN_EXPLORER_WIDTH (220)
    const workspaceWidth = 400;
    const ratioMax = Math.floor(workspaceWidth * LAYOUT_MAX_EXPLORER_RATIO); // 180
    expect(clampExplorerWidth(300, workspaceWidth)).toBe(ratioMax);
    expect(clampExplorerWidth(100, workspaceWidth)).toBe(ratioMax);
    expect(ratioMax).toBeLessThan(LAYOUT_MIN_EXPLORER_WIDTH);
  });
});

describe("parseLayoutState", () => {
  it("returns null for null input", () => {
    expect(parseLayoutState(null)).toBeNull();
  });

  it("returns null for invalid JSON", () => {
    expect(parseLayoutState("not-json")).toBeNull();
  });

  it("returns null for non-object JSON", () => {
    expect(parseLayoutState("42")).toBeNull();
    expect(parseLayoutState('"string"')).toBeNull();
  });

  it("returns null for JSON null", () => {
    expect(parseLayoutState("null")).toBeNull();
  });

  it("parses a valid state object", () => {
    const result = parseLayoutState('{"explorerWidth":300}');
    expect(result).toEqual({ explorerWidth: 300 });
  });

  it("returns partial object (extra fields allowed)", () => {
    const result = parseLayoutState('{"explorerWidth":300,"unknown":true}');
    expect(result).toBeDefined();
    expect(result?.explorerWidth).toBe(300);
  });
});

describe("normalizeLayoutState", () => {
  it("applies defaults for null raw state", () => {
    const result = normalizeLayoutState(null, 1440);
    expect(result.explorerWidth).toBe(
      clampExplorerWidth(LAYOUT_DEFAULTS.explorerWidth, 1440),
    );
  });

  it("applies defaults for empty object", () => {
    const result = normalizeLayoutState({}, 1440);
    expect(result.explorerWidth).toBe(
      clampExplorerWidth(LAYOUT_DEFAULTS.explorerWidth, 1440),
    );
  });

  it("uses parsed explorerWidth and clamps it", () => {
    const result = normalizeLayoutState({ explorerWidth: 9999 }, 1440);
    expect(result.explorerWidth).toBeLessThanOrEqual(LAYOUT_MAX_EXPLORER_WIDTH);
  });

  it("clamps to min for too-small stored width", () => {
    const result = normalizeLayoutState({ explorerWidth: 10 }, 1440);
    expect(result.explorerWidth).toBe(LAYOUT_MIN_EXPLORER_WIDTH);
  });

  it("works with workspaceWidth 0 - no ratio cap applied", () => {
    const result = normalizeLayoutState({ explorerWidth: 400 }, 0);
    expect(result.explorerWidth).toBe(400);
  });
});

describe("serializeLayoutState", () => {
  it("round-trips through parseLayoutState", () => {
    const state = { explorerWidth: 350 };
    const parsed = parseLayoutState(serializeLayoutState(state));
    expect(parsed?.explorerWidth).toBe(350);
  });
});
