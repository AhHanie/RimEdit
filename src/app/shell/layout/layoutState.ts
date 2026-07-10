export interface ShellLayoutState {
  explorerWidth: number;
}

export const STORAGE_KEY = "rimedit.shellLayout.v1";

export const LAYOUT_DEFAULTS: ShellLayoutState = { explorerWidth: 300 };
export const LAYOUT_MIN_EXPLORER_WIDTH = 220;
export const LAYOUT_MAX_EXPLORER_WIDTH = 520;
export const LAYOUT_MAX_EXPLORER_RATIO = 0.45;

export function clampExplorerWidth(width: number, workspaceWidth: number): number {
  const absMax =
    workspaceWidth > 0
      ? Math.min(LAYOUT_MAX_EXPLORER_WIDTH, Math.floor(workspaceWidth * LAYOUT_MAX_EXPLORER_RATIO))
      : LAYOUT_MAX_EXPLORER_WIDTH;
  // On very narrow windows the ratio cap can fall below the stated minimum.
  // Editor protection takes priority: clamp to the ratio cap, not the minimum.
  const effectiveMin = Math.min(LAYOUT_MIN_EXPLORER_WIDTH, absMax);
  return Math.max(effectiveMin, Math.min(absMax, width));
}

export function parseLayoutState(value: string | null): Partial<ShellLayoutState> | null {
  if (value === null) return null;
  try {
    const parsed = JSON.parse(value);
    if (typeof parsed !== "object" || parsed === null) return null;
    return parsed as Partial<ShellLayoutState>;
  } catch {
    return null;
  }
}

export function normalizeLayoutState(
  raw: Partial<ShellLayoutState> | null,
  workspaceWidth: number,
): ShellLayoutState {
  const width = typeof raw?.explorerWidth === "number" ? raw.explorerWidth : LAYOUT_DEFAULTS.explorerWidth;
  return { explorerWidth: clampExplorerWidth(width, workspaceWidth) };
}

export function serializeLayoutState(state: ShellLayoutState): string {
  return JSON.stringify(state);
}
