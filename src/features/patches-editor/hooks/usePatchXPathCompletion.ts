import { useEffect, useRef, useState } from "react";
import { measureAsync } from "../../../instrumentation";
import { completePatchOperationXPath } from "../api/xpathCompletion";
import type { XPathCompletionResult } from "../types/xpathCompletion";

const DEBOUNCE_MS = 180;

export interface UsePatchXPathCompletionResult {
  /** The latest settled completion result for `xpath`, or `null` while there's no project
   * context, no result has arrived yet, or the last request failed. */
  result: XPathCompletionResult | null;
}

/** Debounced, stale-response-safe XPath completion fetcher shared by `PatchPathInput` (dropdown
 * items/diagnostics/replaceFrom) and, via a callback `PatchPathInput` reports its result through,
 * `PatchValueEditor` (`target`/`resolvedField`) -- so one settled XPath edit produces exactly one
 * `complete_patch_operation_xpath` request instead of two independent debounced fetches.
 *
 * Deliberately does not cache results client-side: this hook has no visibility into project
 * settings/schema-catalog changes (registered locations, game version), so a `(projectId, locale,
 * xpath)`-keyed cache here would have no invalidation path and could serve a stale field list
 * after such a change -- unlike the backend's `SchemaCatalogCacheState`, which is explicitly
 * cleared whenever those settings change. The backend cache already makes a repeated request for
 * the same input cheap, so skipping a client-side cache only costs one extra IPC round trip, not
 * a full catalog rebuild.
 *
 * `xpath` is expected to be the field's live *draft* text (updated every keystroke, not just on
 * commit) and `cursorByteOffset` its caret's UTF-8 byte offset into that draft (updated on every
 * caret move, not just an edit) -- every render where `projectId`, `xpath`, `cursorByteOffset`, or
 * `locale` differs from the previous one (re)starts the debounce window; an unchanged tuple (e.g.
 * a mere refocus with no edit or caret movement) is a no-op since the effect's dependencies
 * haven't changed. A request id is bumped synchronously whenever the effect (re)runs, before the
 * debounce timer is scheduled, so a request already in flight from a prior debounce window --
 * whether superseded by a text edit or just a caret move -- can never overwrite a newer one's
 * result once it resolves. */
export function usePatchXPathCompletion(
  projectId: string | null,
  xpath: string,
  locale: string,
  cursorByteOffset: number,
): UsePatchXPathCompletionResult {
  const [result, setResult] = useState<XPathCompletionResult | null>(null);
  const requestIdRef = useRef(0);
  const prevXpathRef = useRef(xpath);

  useEffect(() => {
    if (!projectId) {
      setResult(null);
      return;
    }
    // A caret move with no text change (e.g. a click or arrow-key) still reruns this effect
    // (`cursorByteOffset` changed) but leaves the previously displayed `result` in place, which
    // was computed for a *different* position in the same string -- unlike a text edit, where the
    // string itself changing already makes stale-looking items obviously about to be replaced.
    // Clear it immediately here so a suggestion for an old caret location can never be shown (and
    // therefore never accepted) once the caret has moved on, rather than waiting out the debounce.
    if (prevXpathRef.current === xpath) {
      setResult(null);
    }
    prevXpathRef.current = xpath;

    const requestId = ++requestIdRef.current;

    const timer = setTimeout(() => {
      measureAsync(
        "patches.xpathCompletion",
        () => completePatchOperationXPath(projectId, xpath, locale, cursorByteOffset),
        { xpathLength: xpath.length },
      )
        .then((res) => {
          if (requestIdRef.current !== requestId) return; // superseded by a newer edit/caret move
          setResult(res);
        })
        .catch(() => {
          if (requestIdRef.current !== requestId) return;
          setResult(null);
        });
    }, DEBOUNCE_MS);

    return () => clearTimeout(timer);
  }, [projectId, xpath, locale, cursorByteOffset]);

  return { result };
}
