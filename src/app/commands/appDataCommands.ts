import { invoke } from "@tauri-apps/api/core";

/** Opens RimEdit's persistent-data directory (settings, index caches, form views, user templates,
 * save backups) in the OS file explorer. The backend resolves and creates the fixed app-storage
 * path itself -- this call carries no arguments, so the frontend cannot direct it at an arbitrary
 * filesystem path. */
export function openAppDataFolder(): Promise<void> {
  return invoke("open_app_data_folder");
}
