import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { markTiming } from "../../../../instrumentation";
import { emptyToNull, nullToEmpty } from "../../lib/arrayUtils";
import { renderDiagnostic } from "../../../../i18n/diagnostics";
import { useLocale } from "../../../../i18n/LocaleProvider";
import { usePatchXPathCompletion } from "../../hooks/usePatchXPathCompletion";
import type { XPathCompletionItem, XPathCompletionResult } from "../../types/xpathCompletion";
import styles from "./PatchPathInput.module.css";

interface Props {
  value: string | null;
  readOnly: boolean;
  label: string;
  placeholder?: string;
  /** Absent when the patch editor has no project context to query completions against (e.g. a
   * read-only source-location file opened outside a project) -- the field still works as a plain
   * text area, just without autocomplete. */
  projectId: string | null;
  onChange: (value: string | null) => void;
  /** Reports the latest shared completion result for this field's draft text, so a sibling
   * `PatchValueEditor` can derive `target`/`resolvedField` from the same request instead of
   * issuing its own (Plan.md's "share one completion result"). */
  onCompletionResult?: (result: XPathCompletionResult | null) => void;
  /** Registers a callback that commits any pending (not-yet-propagated) draft text immediately.
   * `usePatchOperationTree`'s `flush()` calls every registered draft flush before awaiting its own
   * pending serialize, so a save/mode-switch/navigation never loses an in-progress edit that
   * hasn't hit its idle-commit timer yet. */
  registerDraftFlush?: (flush: () => void) => () => void;
}

/** How long the field can sit idle after a keystroke before its draft is committed to the patch
 * operation tree (reserializing the whole file). Separate from and much longer than the
 * completion debounce -- typing should feel instant and the dropdown should stay responsive, but
 * the tree itself only needs to catch up once the user pauses, not on every character. */
const IDLE_COMMIT_MS = 500;

/** `replaceFrom`/`replaceTo` are UTF-8 byte offsets (computed in Rust over `str`), but JS strings
 * are indexed in UTF-16 code units -- for any non-ASCII text before an offset (e.g. a defName with
 * accented characters), a raw `.slice(0, byteOffset)` would cut at the wrong position. Rust only
 * ever returns offsets that fall on a UTF-8 character boundary, so re-encoding the prefix and
 * reading its UTF-16 length back out is always well-formed. */
function byteOffsetToStringIndex(text: string, byteOffset: number): number {
  const bytes = new TextEncoder().encode(text);
  return new TextDecoder().decode(bytes.subarray(0, byteOffset)).length;
}

/** The reverse of `byteOffsetToStringIndex`: a UTF-16 string index (as reported by a DOM
 * `selectionStart`/`selectionEnd`) to a UTF-8 byte offset, for sending the caret position to the
 * completion command. `stringIndex` is clamped to `text`'s own length first so an out-of-range
 * value (there shouldn't be one, but a DOM selection index is not statically guaranteed to match a
 * React-rendered value in every edge case) can never encode a slice past the end of the string. */
function stringIndexToByteOffset(text: string, stringIndex: number): number {
  const clamped = Math.max(0, Math.min(stringIndex, text.length));
  return new TextEncoder().encode(text.slice(0, clamped)).length;
}

/** XPath input for patch operation forms: a monospace, multiline `<textarea>` backed by schema-
 * and Def-index-aware completions from `complete_patch_operation_xpath` (see
 * `docs/patches-editor/05-xpath-autocomplete-and-target-inference.md`). Unlike `ReferencePicker`
 * (which replaces its whole value on selection), completions here replace only the active token --
 * the completion result's `[replaceFrom, replaceTo)` byte span -- since an XPath is composed of
 * many segments typed one after another, and the caret can sit anywhere in the string, not only at
 * its end (hard line breaks are stored as XPath text and never normalized away, so a hand-formatted
 * multiline expression round-trips exactly as typed).
 *
 * The textbox is a *staged* editor: typing updates the local draft (and, via
 * `usePatchXPathCompletion`, the dropdown) immediately, but does not itself call `onChange` --
 * committing the patch-operation AST and reserializing the whole file on every keystroke is what
 * made typing feel sluggish (Plan.md finding #1). The draft commits to `onChange` at deliberate
 * boundaries instead: selecting a completion, blurring the field, an idle pause after typing
 * stops, or an explicit `registerDraftFlush` call before save/mode-switch/navigation. Changing only
 * the caret/selection never commits or reserializes -- it just re-queries completions for the new
 * position. */
export function PatchPathInput({
  value,
  readOnly,
  label,
  placeholder,
  projectId,
  onChange,
  onCompletionResult,
  registerDraftFlush,
}: Props) {
  const { i18n } = useTranslation("diagnostics");
  const { t } = useTranslation("patches");
  const { locale } = useLocale();
  const [draftValue, setDraftValue] = useState(nullToEmpty(value));
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [caretByteOffset, setCaretByteOffset] = useState(() =>
    stringIndexToByteOffset(nullToEmpty(value), nullToEmpty(value).length),
  );
  const wrapperRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const listboxId = useId();

  // A read-only field can never be typed into or select a suggestion (its textarea is disabled),
  // and `PatchValueEditor`'s own structured-vs-raw choice for a read-only file isn't worth an
  // extra backend request per expanded row either -- so skip the shared fetch entirely by
  // reporting no project context, exactly like the pre-staged-draft `PatchPathInput` did.
  const { result } = usePatchXPathCompletion(readOnly ? null : projectId, draftValue, locale, caretByteOffset);
  const items: XPathCompletionItem[] = result?.items ?? [];
  const diagnostics = result?.diagnostics ?? [];

  const onCompletionResultRef = useRef(onCompletionResult);
  onCompletionResultRef.current = onCompletionResult;
  useEffect(() => {
    onCompletionResultRef.current?.(result);
  }, [result]);

  // Tracks what `onChange` was last called with (or the last externally-adopted `value`), so
  // commit boundaries (idle timer, blur, flush) can tell whether there's actually a pending edit
  // to propagate, and the reconciliation effect below can distinguish "this is our own commit
  // echoing back through `value`" from a genuine external change.
  const lastCommittedRef = useRef(nullToEmpty(value));
  const draftValueRef = useRef(draftValue);
  draftValueRef.current = draftValue;
  const isFocusedRef = useRef(false);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const idleCommitRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Timestamp of the first keystroke since the last commit -- lets `commitDraft` report how long
  // a draft sat edited-but-uncommitted before it finally reached the parent tree mutation.
  const draftStartedAtRef = useRef<number | null>(null);
  // A caret position to restore (in UTF-16 string indices) after `draftValue` next re-renders the
  // textarea, set only by `selectItem` -- normal typing/clicking lets the browser manage the
  // caret itself and never touches this.
  const pendingCaretIndexRef = useRef<number | null>(null);

  function commitDraft(raw: string) {
    if (idleCommitRef.current) {
      clearTimeout(idleCommitRef.current);
      idleCommitRef.current = null;
    }
    const next = emptyToNull(raw);
    const normalized = nullToEmpty(next);
    if (normalized === lastCommittedRef.current) return;
    lastCommittedRef.current = normalized;
    if (draftStartedAtRef.current !== null) {
      markTiming("patches.xpathDraftCommit", performance.now() - draftStartedAtRef.current);
      draftStartedAtRef.current = null;
    }
    onChangeRef.current(next);
  }

  // External value changes (undo/redo, a hand-edit made in raw mode, or another draft-flush
  // reconciling first): adopt them into the draft immediately, *unless* this field is focused
  // with its own not-yet-committed edit in progress -- that local draft must survive until it is
  // itself committed or discarded, not get silently overwritten by an unrelated external change.
  useEffect(() => {
    const incoming = nullToEmpty(value);
    if (incoming === lastCommittedRef.current) return; // our own commit's echo
    lastCommittedRef.current = incoming;
    if (isFocusedRef.current && draftValueRef.current !== lastCommittedRef.current) return;
    setDraftValue(incoming);
    setCaretByteOffset(stringIndexToByteOffset(incoming, incoming.length));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- draftValueRef/lastCommittedRef are
    // refs read for their current value, not reactive dependencies.
  }, [value]);

  useEffect(() => {
    return () => {
      if (idleCommitRef.current) clearTimeout(idleCommitRef.current);
    };
  }, []);

  useEffect(() => {
    if (!registerDraftFlush) return;
    return registerDraftFlush(() => commitDraft(draftValueRef.current));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- commitDraft reads onChange/lastCommittedRef
    // via refs, so only the registry function identity itself needs to be a dependency.
  }, [registerDraftFlush]);

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

  // Restore the caret after a completion is applied and `draftValue` re-renders the textarea with
  // the spliced text -- a no-op on every other `draftValue` change (typing, external adoption),
  // since the browser already manages the caret for those on its own.
  useEffect(() => {
    const pending = pendingCaretIndexRef.current;
    if (pending === null) return;
    pendingCaretIndexRef.current = null;
    const el = textareaRef.current;
    if (!el) return;
    el.focus();
    el.setSelectionRange(pending, pending);
    setCaretByteOffset(stringIndexToByteOffset(el.value, pending));
  }, [draftValue]);

  function syncSelection(el: HTMLTextAreaElement) {
    const index = el.selectionStart ?? el.value.length;
    setCaretByteOffset(stringIndexToByteOffset(el.value, index));
  }

  function scheduleIdleCommit(next: string) {
    if (idleCommitRef.current) clearTimeout(idleCommitRef.current);
    idleCommitRef.current = setTimeout(() => commitDraft(next), IDLE_COMMIT_MS);
  }

  function handleInputChange(e: React.ChangeEvent<HTMLTextAreaElement>) {
    const next = e.currentTarget.value;
    if (draftStartedAtRef.current === null) draftStartedAtRef.current = performance.now();
    setDraftValue(next);
    setActiveIndex(-1);
    setOpen(true);
    scheduleIdleCommit(next);
    syncSelection(e.currentTarget);
  }

  function handleInputFocus() {
    isFocusedRef.current = true;
    setOpen(true);
  }

  function handleInputBlur() {
    isFocusedRef.current = false;
    commitDraft(draftValueRef.current);
    // Delay to allow a suggestion click to register before the dropdown closes -- belt-and-braces
    // alongside `preventDefault` in the option's own `onMouseDown` (see below), which stops the
    // textarea from blurring in the first place for a mouse selection.
    setTimeout(() => setOpen(false), 150);
  }

  function selectItem(item: XPathCompletionItem) {
    if (!result) return;
    const fromIndex = byteOffsetToStringIndex(draftValue, result.replaceFrom);
    const toIndex = byteOffsetToStringIndex(draftValue, result.replaceTo);
    const next = draftValue.slice(0, fromIndex) + item.insertText + draftValue.slice(toIndex);
    pendingCaretIndexRef.current = fromIndex + item.insertText.length;
    setDraftValue(next);
    setActiveIndex(-1);
    commitDraft(next);
    setOpen(true);
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    // Enter always inserts a newline (the textarea's own default behavior -- never intercepted
    // here) so multiline editing never needs a hidden key chord for its primary action; Tab
    // accepts the highlighted completion instead, and only then blocks its normal focus-traversal
    // behavior.
    if (!open || items.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, items.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Tab" && activeIndex >= 0) {
      e.preventDefault();
      selectItem(items[activeIndex]);
    } else if (e.key === "Escape") {
      setOpen(false);
    }
  }

  const activeDescendantId = activeIndex >= 0 ? `${listboxId}-option-${activeIndex}` : undefined;

  return (
    <label className={styles.field}>
      <span className={styles.label}>{label}</span>
      <div className={styles.wrapper} ref={wrapperRef}>
        <textarea
          ref={textareaRef}
          className={styles.input}
          // XPath is machine-readable syntax, not natural-language prose -- keep it forced LTR
          // even once a future RTL locale flips `dir` on `<html>` (see
          // docs/i18n/issues/08-editor-and-patch-ui-migration.md's "keep code editor/XML/XPath
          // controls dir=ltr by semantic policy" carve-out).
          dir="ltr"
          rows={1}
          wrap="off"
          value={draftValue}
          // `readOnly`, not `disabled`: a read-only/source-location file's XPath must remain
          // selectable, copyable plain text (Plan.md's contract) rather than an inert control --
          // `disabled` form controls are inconsistently selectable across browsers, `readOnly`
          // ones are reliably both focusable and selectable while still rejecting edits.
          readOnly={readOnly}
          placeholder={placeholder ?? 'Defs/ThingDef[defName="Wall"]'}
          autoComplete="off"
          spellCheck={false}
          // Deliberately keeps the textarea's implicit `textbox` accessibility role (matching
          // `<input type="text">`'s) rather than overriding it to `combobox` -- this is a plain
          // multiline text field with an associated autocomplete popup, not a combobox widget
          // with its own distinct value/selection model. `aria-autocomplete`/`aria-controls`/
          // `aria-expanded`/`aria-activedescendant` are the standard way to associate a textbox
          // with a listbox popup without changing its base role.
          aria-expanded={!readOnly && open && items.length > 0}
          aria-controls={listboxId}
          aria-activedescendant={activeDescendantId}
          aria-autocomplete="list"
          onChange={readOnly ? undefined : handleInputChange}
          onFocus={readOnly ? undefined : handleInputFocus}
          onBlur={readOnly ? undefined : handleInputBlur}
          onKeyDown={readOnly ? undefined : handleKeyDown}
          onClick={readOnly ? undefined : (e) => syncSelection(e.currentTarget)}
          onKeyUp={readOnly ? undefined : (e) => syncSelection(e.currentTarget)}
          onSelect={readOnly ? undefined : (e) => syncSelection(e.currentTarget)}
        />
        {!readOnly && open && items.length > 0 && (
          <div className={styles.dropdown} role="listbox" id={listboxId}>
            {items.map((item, i) => (
              <div
                key={`${item.kind}:${item.label}:${i}`}
                id={`${listboxId}-option-${i}`}
                className={styles.suggestion}
                data-active={i === activeIndex}
                role="option"
                aria-selected={i === activeIndex}
                onMouseDown={(e) => {
                  // Prevent the textarea from ever blurring for this click, rather than relying
                  // solely on `handleInputBlur`'s delayed close to win the race.
                  e.preventDefault();
                  selectItem(item);
                }}
              >
                <span className={styles.suggestionLabel}>{item.label}</span>
                {item.detail && <span className={styles.suggestionDetail}>{item.detail}</span>}
              </div>
            ))}
            {result?.isTruncated && (
              <div className={styles.suggestionStatus} role="status">
                {t("patchPathInput.truncatedResults", { shown: items.length, total: result.totalMatches })}
              </div>
            )}
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
