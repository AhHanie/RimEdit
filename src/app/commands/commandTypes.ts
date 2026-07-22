import type { LucideIcon } from "lucide-react";

export interface CommandAction {
  id: string;
  /** Fully-qualified `namespace:key` translation key for the command's display label. */
  labelKey: string;
  /** Fully-qualified `namespace:key` translation key for the command's comma-separated
   * search keywords. Kept as a key (not a pre-rendered list) so the command palette can
   * re-derive keywords for the active locale at render time. */
  keywordsKey: string;
  icon: LucideIcon;
  run: () => void | Promise<void>;
  disabled?: boolean;
}

/** A single selectable entry in a header menu (`AppMenuBar`). References a `CommandAction.id`
 * from the same `commands` array the command palette renders, rather than duplicating a
 * translated label/icon/handler for the menu -- see `AppMenuBar`'s doc comment. */
export interface MenuCommandEntry {
  kind: "command";
  commandId: string;
  /** Present for stateful entries (the active theme, explorer visibility). When set, the item
   * renders with a checked indicator instead of as a plain command. */
  checked?: boolean;
  /** True for entries that are one of a mutually exclusive set (the three theme choices) --
   * rendered with `role="menuitemradio"` instead of `role="menuitemcheckbox"`. */
  radioGroup?: boolean;
}

export interface MenuSeparatorEntry {
  kind: "separator";
}

export type MenuEntry = MenuCommandEntry | MenuSeparatorEntry;

/** One top-level menu (File/View/Theme/Help) in `AppMenuBar`. */
export interface MenuDescriptor {
  id: string;
  /** Fully-qualified `namespace:key` translation key for the menu's trigger label. */
  labelKey: string;
  entries: MenuEntry[];
}
