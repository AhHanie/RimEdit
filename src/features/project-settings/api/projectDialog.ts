import { open } from "@tauri-apps/plugin-dialog";
import { classifySourceFolder, upsertLocation } from "./projectSettings";
import type { ProjectSettings } from "../types";

export interface OpenProjectResult {
  settings: ProjectSettings;
  locationId: string;
}

export interface AddSourceFolderResult {
  settings: ProjectSettings;
  locationId: string;
  /** True when the picked folder looked like it could be a Steam Workshop collection root
   * (numeric-id-shaped child directories present) but automatic detection wasn't confident
   * enough to switch its source type -- callers should surface a non-blocking reminder that
   * the user can set it to "Steam Workshop" in Preferences if that's what it actually is. */
  ambiguousWorkshopRoot: boolean;
}

function normalizePathForCompare(p: string): string {
  return p
    .replace(/^\\\\\?\\/, "")
    .replace(/\\/g, "/")
    .replace(/\/+$/, "")
    .toLowerCase();
}

export async function pickProjectFolder(): Promise<OpenProjectResult | null> {
  const selected = await open({ directory: true, multiple: false });
  if (!selected || typeof selected !== "string") return null;

  const displayName =
    selected.replace(/\\/g, "/").split("/").filter(Boolean).pop() ?? selected;

  const updated = await upsertLocation({
    displayName,
    rootPath: selected,
    kind: "project",
    sourceType: "folder",
    modId: undefined,
    gameVersion: undefined,
  });

  const pickedNorm = normalizePathForCompare(selected);
  const match =
    updated.locations.find(
      (l) =>
        l.kind === "project" &&
        normalizePathForCompare(l.rootPath) === pickedNorm,
    ) ?? updated.locations.filter((l) => l.kind === "project").slice(-1)[0];

  if (!match) return null;
  return { settings: updated, locationId: match.id };
}

export async function pickSourceFolder(
  currentSettings: ProjectSettings | null,
): Promise<AddSourceFolderResult | null> {
  const selected = await open({ directory: true, multiple: false });
  if (!selected || typeof selected !== "string") return null;

  const displayName =
    selected.replace(/\\/g, "/").split("/").filter(Boolean).pop() ?? selected;

  const beforeSourceIds = new Set(
    (currentSettings?.locations ?? [])
      .filter((l) => l.kind === "source")
      .map((l) => l.id),
  );

  // Classify before creating the location so a high-confidence Steam Workshop collection root
  // (e.g. `.../steamapps/workshop/content/294100`, where every immediate child is one
  // subscribed mod) is registered with the correct resolver from the start, instead of always
  // defaulting to `folder` and relying on the user to notice and fix it manually. Classification
  // failure (e.g. running under the Vite-only dev server with no Tauri backend) falls back to the
  // previous unconditional `folder` behavior rather than blocking the add.
  const classification = await classifySourceFolder(selected).catch(() => ({
    suggestedSourceType: "folder" as const,
    highConfidence: false,
    numericItemCount: 0,
  }));

  const updated = await upsertLocation({
    displayName,
    rootPath: selected,
    kind: "source",
    sourceType: classification.highConfidence ? "steamWorkshop" : "folder",
    modId: undefined,
    gameVersion: undefined,
  });

  const pickedNorm = normalizePathForCompare(selected);
  // Ambiguous: some Workshop-item-shaped children were found, but not confidently enough to
  // switch source type automatically -- an ordinary single-mod folder has none of these at all.
  const ambiguousWorkshopRoot =
    !classification.highConfidence && classification.numericItemCount > 0;

  // Prefer an exact path match - handles source/source duplicates correctly since
  // the backend stores the canonical path and normalizePathForCompare strips UNC prefixes.
  const byPath = updated.locations.find(
    (l) =>
      l.kind === "source" && normalizePathForCompare(l.rootPath) === pickedNorm,
  );
  if (byPath) return { settings: updated, locationId: byPath.id, ambiguousWorkshopRoot };

  // Fall back to a genuinely new source ID - handles symlink/canonicalization mismatches
  // where the stored path differs from the selected path but a new source was still added.
  const byNewId = updated.locations.find(
    (l) => l.kind === "source" && !beforeSourceIds.has(l.id),
  );
  if (byNewId) return { settings: updated, locationId: byNewId.id, ambiguousWorkshopRoot };

  // No new source was added - duplicate path registered as a project, or other no-op.
  return null;
}
