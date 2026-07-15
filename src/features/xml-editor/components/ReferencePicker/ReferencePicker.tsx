import { startTransition, useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ExternalLink } from "lucide-react";
import type { ReferenceMetadata } from "../../../schema-catalog";
import { resolveDefReference, suggestDefReferences } from "../../../def-index/api/defIndex";
import type { DefReferenceSuggestion } from "../../../def-index/types";
import type { XmlEditorFileRef } from "../../hooks/useXmlEditorSession";
import styles from "./ReferencePicker.module.css";

interface Props {
  inputId?: string;
  value: string;
  reference: ReferenceMetadata;
  projectId: string;
  onChange: (value: string) => void;
  onFocus?: () => void;
  onBlur?: () => void;
  readOnly?: boolean;
  onNavigateDef?: (fileRef: XmlEditorFileRef, nodeId: number | null) => void;
}

const DEBOUNCE_MS = 180;
const SUGGESTION_LIMIT = 20;

function effectiveTargets(reference: ReferenceMetadata): string[] {
  const raw = reference.acceptedDefTypes?.length ? reference.acceptedDefTypes : [reference.defType];
  return [...new Set(raw)];
}

interface ReferenceTarget {
  locationId: string;
  locationName?: string;
  relativePath: string;
  nodeId: number | null;
  readOnly: boolean;
  defName: string;
  defType: string;
}

export function ReferencePicker({
  inputId,
  value,
  reference,
  projectId,
  onChange,
  onFocus,
  onBlur,
  readOnly,
  onNavigateDef,
}: Props) {
  const { t } = useTranslation("editor");
  const [draftValue, setDraftValue] = useState(value);
  const [suggestions, setSuggestions] = useState<DefReferenceSuggestion[]>([]);
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [lastSelectedSuggestion, setLastSelectedSuggestion] = useState<DefReferenceSuggestion | null>(null);

  // Sync draftValue from value prop when it changes externally
  useEffect(() => {
    setDraftValue(value);
  }, [value]);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);

  // Effective target types: acceptedDefTypes when provided, otherwise [defType].
  const acceptedTargetsKey = (reference.acceptedDefTypes ?? []).join(",");

  const fetchSuggestions = useCallback(
    (query: string) => {
      const targets = effectiveTargets(reference);
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => {
        suggestDefReferences(projectId, targets, query, reference.scope, SUGGESTION_LIMIT)
          .then((results) => {
            setSuggestions(results);
            setActiveIndex(-1);
          })
          .catch(() => setSuggestions([]));
      }, DEBOUNCE_MS);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [projectId, reference.defType, reference.scope, acceptedTargetsKey],
  );

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  // Close dropdown when clicking outside.
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

  function handleInputChange(e: React.ChangeEvent<HTMLInputElement>) {
    const v = e.currentTarget.value;
    setDraftValue(v);
    setLastSelectedSuggestion(null);
    setOpen(true);
    fetchSuggestions(v);
    startTransition(() => {
      onChange(v);
    });
  }

  function handleInputFocus() {
    setOpen(true);
    fetchSuggestions(draftValue);
    onFocus?.();
  }

  function handleInputBlur() {
    // Delay to allow suggestion clicks to register.
    setTimeout(() => {
      setOpen(false);
      onBlur?.();
    }, 150);
  }

  function selectSuggestion(suggestion: DefReferenceSuggestion) {
    setDraftValue(suggestion.defName);
    onChange(suggestion.defName);
    setLastSelectedSuggestion(suggestion);
    setOpen(false);
    setSuggestions([]);
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (!open || suggestions.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, suggestions.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter" && activeIndex >= 0) {
      e.preventDefault();
      selectSuggestion(suggestions[activeIndex]);
    } else if (e.key === "Escape") {
      setOpen(false);
    }
  }

  function suggestionToTarget(suggestion: DefReferenceSuggestion): ReferenceTarget {
    return {
      locationId: suggestion.readOnly ? suggestion.locationId : projectId,
      locationName: suggestion.locationName,
      relativePath: suggestion.relativePath,
      nodeId: suggestion.nodeId,
      readOnly: suggestion.readOnly,
      defName: suggestion.defName,
      defType: suggestion.defType,
    };
  }

  async function suggestionTarget(trimmed: string): Promise<ReferenceTarget | null> {
    const targets = effectiveTargets(reference);
    try {
      const results = await suggestDefReferences(
        projectId,
        targets,
        trimmed,
        reference.scope,
        SUGGESTION_LIMIT,
      );
      const normalized = trimmed.toLowerCase();
      const exactMatches = results.filter((s) => s.defName.toLowerCase() === normalized);
      const candidates = exactMatches.length > 0 ? exactMatches : results;
      const best = candidates.find((s) => !s.readOnly) ?? candidates[0];
      return best ? suggestionToTarget(best) : null;
    } catch {
      return null;
    }
  }

  async function resolveTarget(trimmed: string): Promise<ReferenceTarget | null> {
    if (lastSelectedSuggestion?.defName === trimmed) {
      return suggestionToTarget(lastSelectedSuggestion);
    }

    const suggested = await suggestionTarget(trimmed);
    if (suggested) return suggested;

    const targets = effectiveTargets(reference);
    try {
      const resolution = await resolveDefReference(projectId, targets, trimmed, reference.scope);
      if (resolution.kind === "editableProjectDef") {
        return {
          locationId: projectId,
          relativePath: resolution.relativePath,
          nodeId: resolution.nodeId,
          readOnly: false,
          defName: trimmed,
          defType: reference.defType,
        };
      }
      if (resolution.kind === "readOnlySourceDef") {
        return {
          locationId: resolution.locationId,
          relativePath: resolution.relativePath,
          nodeId: resolution.nodeId,
          readOnly: true,
          defName: trimmed,
          defType: reference.defType,
        };
      }
    } catch {
      return null;
    }

    return null;
  }

  function actOnTarget(target: ReferenceTarget) {
    onNavigateDef?.(
      {
        locationId: target.locationId,
        locationName: target.locationName,
        sourceKind: target.readOnly ? "source" : "project",
        readOnly: target.readOnly,
        relativePath: target.relativePath,
      },
      target.nodeId,
    );
  }

  async function handleGoToDef() {
    if (!draftValue.trim()) return;
    const trimmed = draftValue.trim();
    const target = await resolveTarget(trimmed);
    if (target) actOnTarget(target);
  }

  return (
    <div className={styles.wrapper} ref={wrapperRef}>
      <div className={styles.inputRow}>
        <input
          id={inputId}
          type="text"
          className={styles.input}
          value={draftValue}
          readOnly={readOnly}
          onChange={readOnly ? undefined : handleInputChange}
          onFocus={readOnly ? undefined : handleInputFocus}
          onBlur={readOnly ? undefined : handleInputBlur}
          onKeyDown={readOnly ? undefined : handleKeyDown}
          autoComplete="off"
        />
        <button
          className={styles.actionBtn}
          onClick={() => void handleGoToDef()}
          type="button"
          title={t("referencePicker.goTo", { defType: reference.defType })}
          aria-label={t("referencePicker.goTo", { defType: reference.defType })}
          disabled={!draftValue.trim()}
        >
          <ExternalLink size={12} />
        </button>
      </div>

      {!readOnly && open && suggestions.length > 0 && (
        <div className={styles.dropdown} role="listbox">
          {suggestions.map((s, i) => (
            <div
              key={`${s.defType}:${s.defName}:${s.relativePath}`}
              className={styles.suggestion}
              data-active={i === activeIndex}
              role="option"
              aria-selected={i === activeIndex}
              onMouseDown={() => selectSuggestion(s)}
            >
              <span className={styles.suggestionName}>{s.defName}</span>
              <span className={styles.suggestionType}>{s.defType}</span>
              <span className={styles.suggestionSource}>{s.locationName}</span>
              {s.readOnly && (
                <span className={styles.badge}>{t("referencePicker.readOnlyBadge")}</span>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
