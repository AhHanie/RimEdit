import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import type { CommandAction, MenuDescriptor, MenuEntry } from "../../commands/commandTypes";
import styles from "./AppMenuBar.module.css";

interface AppMenuBarProps {
  /** The full flat command list `CommandPalette` also renders -- `menus` only references command
   * ids, so a menu item's label/icon/handler/disabled state always comes from this single source
   * instead of being duplicated per menu (Plan.md section 3: "Do not duplicate translated visible
   * labels in two separate registries"). */
  commands: CommandAction[];
  menus: MenuDescriptor[];
}

interface ResolvedSeparator {
  kind: "separator";
  key: string;
}

interface ResolvedItem {
  kind: "item";
  commandId: string;
  label: string;
  icon: CommandAction["icon"];
  disabled: boolean;
  onSelect: () => void | Promise<void>;
  checked?: boolean;
  radioGroup?: boolean;
}

type ResolvedEntry = ResolvedSeparator | ResolvedItem;

interface ResolvedMenu {
  id: string;
  label: string;
  entries: ResolvedEntry[];
}

function resolveEntry(entry: MenuEntry, menuId: string, index: number, commandsById: Map<string, CommandAction>, t: (key: string, fallback: string) => string): ResolvedEntry {
  if (entry.kind === "separator") {
    return { kind: "separator", key: `${menuId}-sep-${index}` };
  }
  const command = commandsById.get(entry.commandId);
  if (!command) {
    throw new Error(`AppMenuBar: unknown command id "${entry.commandId}"`);
  }
  return {
    kind: "item",
    commandId: command.id,
    label: t(command.labelKey, command.labelKey),
    icon: command.icon,
    disabled: !!command.disabled,
    onSelect: command.run,
    checked: entry.checked,
    radioGroup: entry.radioGroup,
  };
}

/**
 * The header's File/View/Theme/Help menu bar (Plan.md: "Header Menu Bar Implementation Plan").
 * Renders as a `nav` with trigger buttons (`aria-haspopup="menu"`) and popup `role="menu"` lists.
 * Exactly one popup may be open at a time, tracked as a single `openMenuId`. Keyboard model:
 * ArrowDown/Up opens a menu or moves within it (wrapping, skipping disabled entries), Home/End
 * jump to the first/last enabled entry, ArrowLeft/Right switch the active top-level menu, Enter/
 * Space activates the focused trigger or item, and Escape closes the open menu and restores focus
 * to its trigger. Mouse hover/click retain ordinary button semantics -- switching menus on hover
 * only happens once a menu is already open, matching desktop menu-bar conventions.
 */
export function AppMenuBar({ commands, menus }: AppMenuBarProps) {
  const { t, i18n } = useTranslation(["shell", "common"]);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  // Set right before `setOpenMenuId` so the effect below knows whether (and where) to move focus
  // once the newly opened popup has mounted -- `null` (mouse open) leaves focus on the trigger.
  const focusIntentRef = useRef<"first" | "last" | null>(null);
  // The id of the menu most recently opened by a deliberate action (click or keyboard), as
  // opposed to `onMouseEnter`'s passive hover-switch. Clicking a trigger only *closes* an already
  // -open menu when that menu is both currently open AND was the one deliberately opened -- a
  // click that lands on a trigger that's open only because the pointer hovered over it on the way
  // there (switching from a previously click-opened menu) must leave it open, not toggle it shut.
  const explicitOpenIdRef = useRef<string | null>(null);
  const groupRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const triggerRefs = useRef<Record<string, HTMLButtonElement | null>>({});

  const commandsById = useMemo(() => {
    const map = new Map<string, CommandAction>();
    for (const command of commands) map.set(command.id, command);
    return map;
  }, [commands]);

  // Re-derived whenever the active locale changes, matching `CommandPalette`'s re-render policy
  // for its own `labelKey`-driven list.
  const resolvedMenus = useMemo<ResolvedMenu[]>(
    () =>
      menus.map((menu) => ({
        id: menu.id,
        label: t(menu.labelKey, menu.labelKey),
        entries: menu.entries.map((entry, index) => resolveEntry(entry, menu.id, index, commandsById, t)),
      })),
    [menus, commandsById, t, i18n.language],
  );

  function enabledItemEls(menuId: string): HTMLButtonElement[] {
    const container = groupRefs.current[menuId];
    if (!container) return [];
    return Array.from(
      container.querySelectorAll<HTMLButtonElement>('[data-menu-item="true"]:not(:disabled)'),
    );
  }

  function openMenu(id: string, focus: "first" | "last" | null) {
    focusIntentRef.current = focus;
    setOpenMenuId(id);
  }

  function closeMenu(restoreFocus: boolean) {
    setOpenMenuId((current) => {
      if (restoreFocus && current) {
        triggerRefs.current[current]?.focus();
      }
      return null;
    });
  }

  // Move focus into the freshly opened popup when it was opened via keyboard.
  useEffect(() => {
    if (!openMenuId) return;
    const intent = focusIntentRef.current;
    focusIntentRef.current = null;
    if (!intent) return;
    const items = enabledItemEls(openMenuId);
    if (items.length === 0) return;
    if (intent === "first") items[0].focus();
    else items[items.length - 1].focus();
    // Only re-run when the open menu identity changes -- `enabledItemEls` reads live refs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [openMenuId]);

  // Close on pointer-down outside the active group, window blur, or window resize.
  useEffect(() => {
    if (!openMenuId) return;
    function handlePointerDown(e: PointerEvent) {
      const container = groupRefs.current[openMenuId!];
      if (container && !container.contains(e.target as Node)) {
        closeMenu(false);
      }
    }
    function handleWindowChange() {
      closeMenu(false);
    }
    document.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("blur", handleWindowChange);
    window.addEventListener("resize", handleWindowChange);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("blur", handleWindowChange);
      window.removeEventListener("resize", handleWindowChange);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [openMenuId]);

  function handleRootKeyDown(e: KeyboardEvent<HTMLElement>) {
    const target = e.target as HTMLElement;
    const isTrigger = target.dataset.menuTrigger === "true";
    const isItem = target.dataset.menuItem === "true";
    if (!isTrigger && !isItem) return;

    if (e.key === "Tab") {
      if (openMenuId) closeMenu(false);
      return;
    }

    const menuIds = resolvedMenus.map((m) => m.id);
    const currentMenuId = isTrigger ? target.dataset.menuId! : openMenuId;

    if (e.key === "Escape") {
      if (openMenuId) {
        e.preventDefault();
        closeMenu(true);
      }
      return;
    }

    if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      if (!currentMenuId) return;
      e.preventDefault();
      const idx = menuIds.indexOf(currentMenuId);
      if (idx === -1) return;
      const nextIdx =
        e.key === "ArrowRight" ? (idx + 1) % menuIds.length : (idx - 1 + menuIds.length) % menuIds.length;
      explicitOpenIdRef.current = menuIds[nextIdx];
      openMenu(menuIds[nextIdx], "first");
      return;
    }

    if (isTrigger) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        explicitOpenIdRef.current = currentMenuId;
        openMenu(currentMenuId!, "first");
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        explicitOpenIdRef.current = currentMenuId;
        openMenu(currentMenuId!, "last");
      } else if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        if (openMenuId === currentMenuId) {
          closeMenu(true);
        } else {
          explicitOpenIdRef.current = currentMenuId;
          openMenu(currentMenuId!, "first");
        }
      }
      return;
    }

    if (isItem && openMenuId) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        const items = enabledItemEls(openMenuId);
        if (items.length === 0) return;
        const idx = items.indexOf(target as HTMLButtonElement);
        const nextIdx =
          e.key === "ArrowDown" ? (idx + 1) % items.length : (idx - 1 + items.length) % items.length;
        items[nextIdx].focus();
      } else if (e.key === "Home" || e.key === "End") {
        e.preventDefault();
        const items = enabledItemEls(openMenuId);
        if (items.length === 0) return;
        (e.key === "Home" ? items[0] : items[items.length - 1]).focus();
      }
    }
  }

  return (
    <nav
      className={styles.root}
      aria-label={t("shell:menuBar.ariaLabel")}
      onKeyDown={handleRootKeyDown}
    >
      {resolvedMenus.map((menu) => {
        const isOpen = openMenuId === menu.id;
        const triggerId = `app-menu-trigger-${menu.id}`;
        const popupId = `app-menu-popup-${menu.id}`;
        return (
          <div
            key={menu.id}
            className={styles.group}
            ref={(el) => {
              groupRefs.current[menu.id] = el;
            }}
          >
            <button
              type="button"
              id={triggerId}
              ref={(el) => {
                triggerRefs.current[menu.id] = el;
              }}
              className={`${styles.trigger}${isOpen ? ` ${styles.triggerActive}` : ""}`}
              data-menu-trigger="true"
              data-menu-id={menu.id}
              aria-haspopup="menu"
              aria-expanded={isOpen}
              aria-controls={popupId}
              onClick={() => {
                if (isOpen && explicitOpenIdRef.current === menu.id) {
                  closeMenu(false);
                } else {
                  explicitOpenIdRef.current = menu.id;
                  openMenu(menu.id, null);
                }
              }}
              onMouseEnter={() => {
                if (openMenuId && openMenuId !== menu.id) openMenu(menu.id, null);
              }}
            >
              {menu.label}
            </button>
            {isOpen && (
              <ul id={popupId} role="menu" aria-labelledby={triggerId} className={styles.popup}>
                {menu.entries.map((entry) =>
                  entry.kind === "separator" ? (
                    <li key={entry.key} role="separator" className={styles.separator} />
                  ) : (
                    <li key={entry.commandId} role="none">
                      <button
                        type="button"
                        role={
                          entry.radioGroup
                            ? "menuitemradio"
                            : entry.checked !== undefined
                              ? "menuitemcheckbox"
                              : "menuitem"
                        }
                        aria-checked={entry.checked !== undefined ? entry.checked : undefined}
                        data-menu-item="true"
                        className={styles.menuItem}
                        disabled={entry.disabled}
                        onClick={() => {
                          void entry.onSelect();
                          closeMenu(true);
                        }}
                      >
                        <entry.icon size={14} className={styles.menuItemIcon} aria-hidden="true" />
                        <span className={styles.menuItemLabel}>{entry.label}</span>
                        {entry.checked && (
                          <Check size={13} className={styles.menuItemCheck} aria-hidden="true" />
                        )}
                      </button>
                    </li>
                  ),
                )}
              </ul>
            )}
          </div>
        );
      })}
    </nav>
  );
}
