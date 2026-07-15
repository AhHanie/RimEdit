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
