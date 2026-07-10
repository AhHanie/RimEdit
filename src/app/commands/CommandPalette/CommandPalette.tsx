import { useRef, useState, useEffect, useMemo } from "react";
import { Command } from "lucide-react";
import type { CommandAction } from "../commandTypes";
import styles from "./CommandPalette.module.css";

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  commands: CommandAction[];
}

export function CommandPalette({ open, onClose, commands }: CommandPaletteProps) {
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

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter(
      (c) =>
        c.label.toLowerCase().includes(q) || c.keywords.some((k) => k.toLowerCase().includes(q)),
    );
  }, [query, commands]);

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
      aria-label="Command Palette"
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
            placeholder="Type a command…"
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            aria-label="Search commands"
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
          {filtered.length === 0 && <li className={styles.empty}>No matching commands</li>}
        </ul>
      </div>
    </div>
  );
}
