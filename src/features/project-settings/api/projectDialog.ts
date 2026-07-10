import { open } from "@tauri-apps/plugin-dialog";
import { upsertLocation } from "./projectSettings";
import type { ProjectSettings } from "../types";

export interface OpenProjectResult {
  settings: ProjectSettings;
  locationId: string;
}

export interface AddSourceFolderResult {
  settings: ProjectSettings;
  locationId: string;
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

  const updated = await upsertLocation({
    displayName,
    rootPath: selected,
    kind: "source",
    sourceType: "folder",
    modId: undefined,
    gameVersion: undefined,
  });

  const pickedNorm = normalizePathForCompare(selected);

  // Prefer an exact path match - handles source/source duplicates correctly since
  // the backend stores the canonical path and normalizePathForCompare strips UNC prefixes.
  const byPath = updated.locations.find(
    (l) =>
      l.kind === "source" && normalizePathForCompare(l.rootPath) === pickedNorm,
  );
  if (byPath) return { settings: updated, locationId: byPath.id };

  // Fall back to a genuinely new source ID - handles symlink/canonicalization mismatches
  // where the stored path differs from the selected path but a new source was still added.
  const byNewId = updated.locations.find(
    (l) => l.kind === "source" && !beforeSourceIds.has(l.id),
  );
  if (byNewId) return { settings: updated, locationId: byNewId.id };

  // No new source was added - duplicate path registered as a project, or other no-op.
  return null;
}
