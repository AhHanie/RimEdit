import type { LucideIcon } from "lucide-react";

export interface CommandAction {
  id: string;
  label: string;
  keywords: string[];
  icon: LucideIcon;
  run: () => void | Promise<void>;
  disabled?: boolean;
}
