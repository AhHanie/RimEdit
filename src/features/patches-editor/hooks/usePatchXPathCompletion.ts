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
 * `complete_patch_operation_xpath` request instead of two independent debounced fetches (Plan.md's
 * "share one completion result with value-target inference").
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
 * commit) -- every render where `projectId`, `xpath`, or `locale` differs from the previous one
 * (re)starts the debounce window; an unchanged `xpath` (e.g. a mere refocus with no edit) is a
 * no-op since the effect's dependencies haven't changed. A request id is bumped synchronously
 * whenever the effect (re)runs, before the debounce timer is scheduled, so a request already
 * in flight from a prior debounce window can never overwrite a newer edit's result once it
 * resolves. */
export function usePatchXPathCompletion(
  projectId: string | null,
  xpath: string,
  locale: string,
): UsePatchXPathCompletionResult {
  const [result, setResult] = useState<XPathCompletionResult | null>(null);
  const requestIdRef = useRef(0);

  useEffect(() => {
    if (!projectId) {
      setResult(null);
      return;
    }
    const requestId = ++requestIdRef.current;

    const timer = setTimeout(() => {
      measureAsync("patches.xpathCompletion", () => completePatchOperationXPath(projectId, xpath, locale), {
        xpathLength: xpath.length,
      })
        .then((res) => {
          if (requestIdRef.current !== requestId) return; // superseded by a newer edit
          setResult(res);
        })
        .catch(() => {
          if (requestIdRef.current !== requestId) return;
          setResult(null);
        });
    }, DEBOUNCE_MS);

    return () => clearTimeout(timer);
  }, [projectId, xpath, locale]);

  return { result };
}
