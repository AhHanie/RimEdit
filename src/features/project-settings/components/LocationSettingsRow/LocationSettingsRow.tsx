import { useState } from "react";
import { Pencil, Trash2, Check, X, Loader2 } from "lucide-react";
import type { RegisteredLocation, RegisteredLocationUpdate, SourceType } from "../../types";
import styles from "./LocationSettingsRow.module.css";

const SOURCE_TYPE_LABELS: Record<SourceType, string> = {
  baseGame: "Base Game",
  localMod: "Local Mod",
  steamWorkshop: "Steam Workshop",
  folder: "Folder",
};

const SOURCE_TYPE_OPTIONS: SourceType[] = [
  "baseGame",
  "localMod",
  "steamWorkshop",
  "folder",
];

interface LocationSettingsRowProps {
  location: RegisteredLocation;
  isActive: boolean;
  onSave: (update: RegisteredLocationUpdate) => Promise<void>;
  onRemove: (id: string) => Promise<void>;
}

interface Draft {
  displayName: string;
  sourceType: SourceType;
  modId: string;
}

function draftFromLocation(loc: RegisteredLocation): Draft {
  return {
    displayName: loc.displayName,
    sourceType: loc.sourceType,
    modId: loc.modId ?? "",
  };
}

export function LocationSettingsRow({
  location,
  isActive,
  onSave,
  onRemove,
}: LocationSettingsRowProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Draft>(() => draftFromLocation(location));
  const [saving, setSaving] = useState(false);

  function startEdit() {
    setDraft(draftFromLocation(location));
    setEditing(true);
  }

  function cancelEdit() {
    setEditing(false);
  }

  async function handleSave() {
    const trimmed = draft.displayName.trim();
    if (!trimmed) return;
    setSaving(true);
    try {
      await onSave({
        id: location.id,
        displayName: trimmed,
        sourceType: draft.sourceType,
        modId: draft.modId.trim() || undefined,
        gameVersion: location.gameVersion,
      });
      setEditing(false);
    } catch {
      // parent panel shows the error banner; stay in edit mode so the user's draft is preserved
    } finally {
      setSaving(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter") void handleSave();
    if (e.key === "Escape") cancelEdit();
  }

  const isDirty =
    draft.displayName.trim() !== location.displayName ||
    draft.sourceType !== location.sourceType ||
    (draft.modId.trim() || undefined) !== location.modId;
  const canSave = draft.displayName.trim().length > 0 && isDirty;

  if (editing) {
    return (
      <div className={`${styles.row} ${styles.rowEditing}`}>
        <div className={styles.editFields}>
          <input
            className={styles.nameInput}
            value={draft.displayName}
            onChange={(e) => setDraft((d) => ({ ...d, displayName: e.target.value }))}
            onKeyDown={handleKeyDown}
            disabled={saving}
            autoFocus
            aria-label="Display name"
          />
          {location.kind === "source" && (
            <select
              className={styles.sourceTypeSelect}
              value={draft.sourceType}
              onChange={(e) =>
                setDraft((d) => ({ ...d, sourceType: e.target.value as SourceType }))
              }
              disabled={saving}
              aria-label="Source type"
            >
              {SOURCE_TYPE_OPTIONS.map((t) => (
                <option key={t} value={t}>
                  {SOURCE_TYPE_LABELS[t]}
                </option>
              ))}
            </select>
          )}
          {location.kind === "source" && (
            <input
              className={styles.nameInput}
              value={draft.modId}
              onChange={(e) => setDraft((d) => ({ ...d, modId: e.target.value }))}
              disabled={saving}
              placeholder="Mod ID (optional)"
              aria-label="Mod ID"
            />
          )}
        </div>
        <div className={styles.actions}>
          <button
            className="icon-btn"
            onClick={() => void handleSave()}
            disabled={saving || !canSave}
            aria-label="Save"
            title="Save"
          >
            {saving ? <Loader2 size={14} className={styles.spinner} /> : <Check size={14} />}
          </button>
          <button
            className="icon-btn"
            onClick={cancelEdit}
            disabled={saving}
            aria-label="Cancel"
            title="Cancel"
          >
            <X size={14} />
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className={`${styles.row}${isActive ? ` ${styles.rowActive}` : ""}`}>
      <div className={styles.rowMain}>
        <div className={styles.rowInfo}>
          <span className={styles.nameText}>
            {location.displayName}
            {isActive && <span className={`${styles.badge} ${styles.badgeActive}`}>Active</span>}
          </span>
          <span className={styles.pathText} title={location.rootPath}>
            {location.rootPath}
          </span>
          <div className={styles.badgeRow}>
            <span className={styles.badge}>{location.kind}</span>
            <span className={styles.badge}>
              {SOURCE_TYPE_LABELS[location.sourceType] ?? location.sourceType}
            </span>
            {location.readOnly && <span className={styles.badge}>read-only</span>}
          </div>
          {(location.modId || location.gameVersion) && (
            <div className={styles.metaRow}>
              {location.modId && (
                <span className={styles.metaItem}>mod: {location.modId}</span>
              )}
              {location.gameVersion && (
                <span className={styles.metaItem}>version: {location.gameVersion}</span>
              )}
            </div>
          )}
        </div>
        <div className={styles.actions}>
          <button
            className="icon-btn"
            onClick={startEdit}
            aria-label="Edit"
            title="Edit"
          >
            <Pencil size={14} />
          </button>
          <button
            className="icon-btn"
            onClick={() => void onRemove(location.id)}
            aria-label="Remove"
            title="Remove"
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>
    </div>
  );
}
