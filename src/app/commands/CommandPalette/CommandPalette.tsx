import { useRef, useState, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Command } from "lucide-react";
import type { CommandAction } from "../commandTypes";
import styles from "./CommandPalette.module.css";

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  commands: CommandAction[];
}

interface RenderedCommand extends CommandAction {
  label: string;
  keywords: string[];
}

export function CommandPalette({ open, onClose, commands }: CommandPaletteProps) {
  // Requesting two namespaces (even though this component only reads "shell" keys) keeps the
  // generated `t` type permissive for the dynamic `labelKey`/`keywordsKey` lookups below -- see
  // the analogous multi-namespace calls elsewhere in this codebase.
  const { t, i18n } = useTranslation(["shell", "common"]);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  // Re-derived from `labelKey`/`keywordsKey` whenever the active locale changes -- `i18n.language`
  // is an explicit dependency (not just `t`, whose reference can stay stable across a locale
  // switch) so a switch always recomputes rendered text instead of reusing a cached label.
  const rendered = useMemo<RenderedCommand[]>(
    () =>
      commands.map((c) => ({
        ...c,
        // `labelKey`/`keywordsKey` are runtime data (not literal types), so this uses the
        // "key, defaultValue" overload -- the default is never actually shown since every
        // command descriptor references a key that exists in `shell:commands`.
        label: t(c.labelKey, c.labelKey),
        keywords: t(c.keywordsKey, c.keywordsKey)
          .split(",")
          .map((k: string) => k.trim())
          .filter(Boolean),
      })),
    [commands, t, i18n.language],
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return rendered;
    return rendered.filter(
      (c) =>
        c.label.toLowerCase().includes(q) || c.keywords.some((k) => k.toLowerCase().includes(q)),
    );
  }, [query, rendered]);

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      onClose();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      const cmd = filtered[selectedIndex];
      if (cmd && !cmd.disabled) {
        void cmd.run();
        onClose();
      }
    }
  }

  if (!open) return null;

  return (
    <div
      className={styles.overlay}
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label={t("shell:commandPalette.ariaLabel")}
    >
      <div
        className={styles.modal}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <div className={styles.searchRow}>
          <Command size={15} className={styles.searchIcon} aria-hidden="true" />
          <input
            ref={inputRef}
            className={styles.searchInput}
            type="text"
            placeholder={t("shell:commandPalette.searchPlaceholder")}
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            aria-label={t("shell:commandPalette.searchAriaLabel")}
            aria-autocomplete="list"
            aria-controls="cp-results"
          />
        </div>
        <ul id="cp-results" className={styles.results} role="listbox">
          {filtered.map((cmd, i) => {
            const Icon = cmd.icon;
            return (
              <li
                key={cmd.id}
                className={`${styles.item}${i === selectedIndex ? ` ${styles.itemSelected}` : ""}${cmd.disabled ? ` ${styles.itemDisabled}` : ""}`}
                role="option"
                aria-selected={i === selectedIndex}
                onMouseEnter={() => setSelectedIndex(i)}
                onClick={() => {
                  if (!cmd.disabled) {
                    void cmd.run();
                    onClose();
                  }
                }}
              >
                <Icon size={14} className={styles.itemIcon} aria-hidden="true" />
                <span className={styles.itemLabel}>{cmd.label}</span>
              </li>
            );
          })}
          {filtered.length === 0 && (
            <li className={styles.empty}>{t("shell:commandPalette.empty")}</li>
          )}
        </ul>
      </div>
    </div>
  );
}
