// Small, reusable keyboard/focus behavior for the two new Form View overlay dialogs
// (`FormViewSwitchConfirmDialog`, `FormViewManagerDialog`). Not a shared *dialog component* --
// existing overlays (`SaveDefTemplateDialog`, `CommandPalette`) each own their visual structure
// and CSS module independently, and this issue follows that same convention -- just the
// Escape-to-close/focus-trap/focus-restoration *behavior* the issue asks for, factored out once
// because both new dialogs need it identically rather than copy-pasted twice.
import { useEffect, useRef, type RefObject } from "react";

const FOCUSABLE_SELECTOR =
  'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

// `FormViewSwitchConfirmDialog` can be nested *inside* `FormViewManagerDialog` (switching views
// from within the manager while an override is dirty), so both instances' document-level
// listeners are live at once. Every instance registers itself here on mount and only the
// topmost (most recently mounted, still-open) one actually reacts to Escape/Tab -- otherwise
// Escape on the nested confirm dialog would also close the manager dialog underneath it.
const dialogStack: symbol[] = [];

/**
 * While the owning component is mounted: closes on Escape, keeps Tab/Shift+Tab cycling within
 * `containerRef`'s subtree, and restores focus to whatever was focused before the dialog opened
 * once it unmounts (if that element is still attached to the document).
 */
export function useDialogKeyboard(
  containerRef: RefObject<HTMLElement | null>,
  onClose: () => void,
): void {
  // Captured via ref (not a `useCallback` dep) so a caller passing a fresh inline `onClose`
  // every render never forces this effect to re-run and re-capture `previouslyFocused` at the
  // wrong time -- only the *first* mount's focused element should ever be restored.
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const dialogIdRef = useRef<symbol | null>(null);
  if (dialogIdRef.current === null) dialogIdRef.current = Symbol("form-view-dialog");

  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const dialogId = dialogIdRef.current!;
    dialogStack.push(dialogId);

    function isTopmost(): boolean {
      return dialogStack[dialogStack.length - 1] === dialogId;
    }

    function focusableElements(): HTMLElement[] {
      const container = containerRef.current;
      if (!container) return [];
      return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
    }

    // Move focus INTO the dialog on open. Without this, focus stays on whatever triggered the
    // dialog (e.g. the "Customize view" button) and the Tab-wrap logic below never engages,
    // because it only wraps once focus is already on the first/last focusable element inside
    // the container -- so Tab would keep cycling through the background page instead of being
    // trapped. Skip only if something inside the dialog already has focus (e.g. an `autoFocus`
    // input rendered on first paint) so we don't steal focus from a more specific target.
    const container = containerRef.current;
    if (container && !container.contains(document.activeElement)) {
      const initialItems = focusableElements();
      if (initialItems.length > 0) {
        initialItems[0].focus();
      } else if (!container.hasAttribute("tabindex")) {
        container.setAttribute("tabindex", "-1");
        container.focus();
      } else {
        container.focus();
      }
    }

    function handleKeyDown(e: KeyboardEvent) {
      if (!isTopmost()) return;
      if (e.key === "Escape") {
        e.stopPropagation();
        onCloseRef.current();
        return;
      }
      if (e.key !== "Tab") return;
      const items = focusableElements();
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      const current = document.activeElement;
      if (e.shiftKey && current === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && current === last) {
        e.preventDefault();
        first.focus();
      }
    }

    document.addEventListener("keydown", handleKeyDown, true);
    return () => {
      document.removeEventListener("keydown", handleKeyDown, true);
      const idx = dialogStack.indexOf(dialogId);
      if (idx !== -1) dialogStack.splice(idx, 1);
      if (previouslyFocused && document.body.contains(previouslyFocused)) {
        previouslyFocused.focus();
      }
    };
    // Intentionally runs once per mount: `onClose` is read fresh via closure, and re-running
    // this effect on every `onClose` identity change would re-capture `previouslyFocused` at the
    // wrong time (any render after the first, not just the initial mount).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [containerRef]);
}
