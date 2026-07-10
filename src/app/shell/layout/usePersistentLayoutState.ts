import { useState, useEffect, type RefObject } from "react";
import {
  STORAGE_KEY,
  LAYOUT_DEFAULTS,
  LAYOUT_MIN_EXPLORER_WIDTH,
  LAYOUT_MAX_EXPLORER_WIDTH,
  LAYOUT_MAX_EXPLORER_RATIO,
  clampExplorerWidth,
  parseLayoutState,
  serializeLayoutState,
} from "./layoutState";

export function usePersistentLayoutState(containerRef: RefObject<HTMLElement | null>): {
  explorerWidth: number;
  minExplorerWidth: number;
  maxExplorerWidth: number;
  setExplorerWidth: (width: number) => void;
} {
  const [workspaceWidth, setWorkspaceWidth] = useState(0);
  const [storedWidth, setStoredWidth] = useState(() => {
    const raw = parseLayoutState(localStorage.getItem(STORAGE_KEY));
    return typeof raw?.explorerWidth === "number" ? raw.explorerWidth : LAYOUT_DEFAULTS.explorerWidth;
  });

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) setWorkspaceWidth(entry.contentRect.width);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [containerRef]);

  const explorerWidth = clampExplorerWidth(storedWidth, workspaceWidth);

  const maxExplorerWidth =
    workspaceWidth > 0
      ? Math.min(LAYOUT_MAX_EXPLORER_WIDTH, Math.floor(workspaceWidth * LAYOUT_MAX_EXPLORER_RATIO))
      : LAYOUT_MAX_EXPLORER_WIDTH;

  // On very narrow windows, ratio cap can fall below the stated minimum.
  // Keep aria-valuemin <= aria-valuemax so ARIA attributes are always consistent.
  const minExplorerWidth = Math.min(LAYOUT_MIN_EXPLORER_WIDTH, maxExplorerWidth);

  function setExplorerWidth(width: number) {
    const clamped = clampExplorerWidth(width, workspaceWidth);
    setStoredWidth(clamped);
    localStorage.setItem(STORAGE_KEY, serializeLayoutState({ explorerWidth: clamped }));
  }

  return { explorerWidth, minExplorerWidth, maxExplorerWidth, setExplorerWidth };
}
