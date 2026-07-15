import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { completePatchOperationXPath } from "../../api/xpathCompletion";
import { emptyToNull, nullToEmpty } from "../../lib/arrayUtils";
import { renderDiagnostic } from "../../../../i18n/diagnostics";
import { useLocale } from "../../../../i18n/LocaleProvider";
import type { XPathCompletionItem, XPathDiagnostic } from "../../types/xpathCompletion";
import styles from "./PatchPathInput.module.css";

interface Props {
  value: string | null;
  readOnly: boolean;
  label: string;
  placeholder?: string;
  /** Absent when the patch editor has no project context to query completions against (e.g. a
   * read-only source-location file opened outside a project) -- the field still works as a plain
   * text input, just without autocomplete. */
  projectId: string | null;
  onChange: (value: string | null) => void;
}

const DEBOUNCE_MS = 180;

/** `replaceFrom` is a UTF-8 byte offset (computed in Rust over `str`), but JS strings are indexed
 * in UTF-16 code units -- for any non-ASCII text before the offset (e.g. a defName with accented
 * characters), a raw `.slice(0, replaceFrom)` would cut at the wrong position. Rust only ever
 * returns offsets that fall on a UTF-8 character boundary, so re-encoding the prefix and reading
 * its UTF-16 length back out is always well-formed. */
function byteOffsetToStringIndex(text: string, byteOffset: number): number {
  const bytes = new TextEncoder().encode(text);
  return new TextDecoder().decode(bytes.subarray(0, byteOffset)).length;
}

/** XPath input for patch operation forms: a plain text field backed by schema- and Def-index-aware
 * completions from `complete_patch_operation_xpath` (see `docs/patches-editor/05-xpath-autocomplete-and-target-inference.md`).
 * Unlike `ReferencePicker` (which replaces its whole value on selection), completions here replace
 * only the current segment -- from the completion result's `replaceFrom` offset onward -- since an
 * XPath is composed of many segments typed one after another. */
export function PatchPathInput({ value, readOnly, label, placeholder, projectId, onChange }: Props) {
  const { i18n } = useTranslation("diagnostics");
  const { locale } = useLocale();
  const [draftValue, setDraftValue] = useState(nullToEmpty(value));
  const [items, setItems] = useState<XPathCompletionItem[]>([]);
  const [diagnostics, setDiagnostics] = useState<XPathDiagnostic[]>([]);
  const [replaceFrom, setReplaceFrom] = useState(0);
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);

  useEffect(() => {
    setDraftValue(nullToEmpty(value));
  }, [value]);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const requestIdRef = useRef(0);
  // Gates the "refocus without an edit shouldn't refetch" skip in `handleInputFocus` -- keyed on
  // both the xpath text and the locale it was fetched under, so a locale switch (which changes
  // the completion command's localized labels/details) always triggers a refetch even when the
  // xpath text itself is unchanged. Keying on xpath alone would let a stale locale's completion
  // labels survive a focus/reopen after switching the app locale.
  const lastFetchedRef = useRef<{ xpath: string; locale: string } | null>(null);

  const fetchCompletions = useCallback(
    (xpath: string) => {
      if (!projectId) return;
      if (debounceRef.current) clearTimeout(debounceRef.current);
      // Bump the request id synchronously, not inside the timer callback below -- otherwise an
      // earlier request that's already in flight (its own debounce window elapsed, its `invoke`
      // call pending) would still look "current" for the whole span of this new debounce window,
      // and could apply its stale items/replaceFrom to `draftValue` right before this newer
      // request's own response arrives.
      const requestId = ++requestIdRef.current;
      debounceRef.current = setTimeout(() => {
        completePatchOperationXPath(projectId, xpath, locale)
          .then((result) => {
            if (requestIdRef.current !== requestId) return;
            lastFetchedRef.current = { xpath, locale };
            setItems(result.items);
            setDiagnostics(result.diagnostics);
            setReplaceFrom(result.replaceFrom);
            setActiveIndex(-1);
          })
          .catch(() => {
            if (requestIdRef.current !== requestId) return;
            setItems([]);
            setDiagnostics([]);
          });
      }, DEBOUNCE_MS);
    },
    [projectId, locale],
  );

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  useEffect(() => {
    if (!open) return;
    function handleClick(e: MouseEvent) {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [open]);

  function commit(next: string) {
    setDraftValue(next);
    onChange(emptyToNull(next));
  }

  function handleInputChange(e: React.ChangeEvent<HTMLInputElement>) {
    const next = e.currentTarget.value;
    commit(next);
    setOpen(true);
    fetchCompletions(next);
  }

  function handleInputFocus() {
    setOpen(true);
    // Refocusing without an edit (e.g. tabbing away and back) shouldn't re-fetch -- the last
    // result is still valid for the same text under the same locale. A locale switch since the
    // last fetch always forces a refetch so stale localized labels don't linger.
    if (draftValue !== lastFetchedRef.current?.xpath || locale !== lastFetchedRef.current?.locale) {
      fetchCompletions(draftValue);
    }
  }

  function handleInputBlur() {
    // Delay to allow a suggestion click to register before the dropdown closes.
    setTimeout(() => setOpen(false), 150);
  }

  function selectItem(item: XPathCompletionItem) {
    const next = draftValue.slice(0, byteOffsetToStringIndex(draftValue, replaceFrom)) + item.insertText;
    commit(next);
    setOpen(true);
    fetchCompletions(next);
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (!open || items.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, items.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter" && activeIndex >= 0) {
      e.preventDefault();
      selectItem(items[activeIndex]);
    } else if (e.key === "Escape") {
      setOpen(false);
    }
  }

  return (
    <label className={styles.field}>
      <span className={styles.label}>{label}</span>
      <div className={styles.wrapper} ref={wrapperRef}>
        <input
          type="text"
          className={styles.input}
          // XPath is machine-readable syntax, not natural-language prose -- keep it forced LTR
          // even once a future RTL locale flips `dir` on `<html>` (see
          // docs/i18n/issues/08-editor-and-patch-ui-migration.md's "keep code editor/XML/XPath
          // controls dir=ltr by semantic policy" carve-out).
          dir="ltr"
          value={draftValue}
          disabled={readOnly}
          placeholder={placeholder ?? 'Defs/ThingDef[defName="Wall"]'}
          autoComplete="off"
          spellCheck={false}
          onChange={readOnly ? undefined : handleInputChange}
          onFocus={readOnly ? undefined : handleInputFocus}
          onBlur={readOnly ? undefined : handleInputBlur}
          onKeyDown={readOnly ? undefined : handleKeyDown}
        />
        {!readOnly && open && items.length > 0 && (
          <div className={styles.dropdown} role="listbox">
            {items.map((item, i) => (
              <div
                key={`${item.kind}:${item.label}:${i}`}
                className={styles.suggestion}
                data-active={i === activeIndex}
                role="option"
                aria-selected={i === activeIndex}
                onMouseDown={() => selectItem(item)}
              >
                <span className={styles.suggestionLabel}>{item.label}</span>
                {item.detail && <span className={styles.suggestionDetail}>{item.detail}</span>}
              </div>
            ))}
          </div>
        )}
      </div>
      {diagnostics.length > 0 && (
        <ul className={styles.diagnostics}>
          {diagnostics.map((d, i) => (
            <li key={i} className={styles.diagnostic} data-severity={d.severity}>
              {renderDiagnostic(d, i18n)}
            </li>
          ))}
        </ul>
      )}
    </label>
  );
}
