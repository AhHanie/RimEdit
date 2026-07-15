import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Pencil, Trash2, Check, X, Loader2 } from "lucide-react";
import type { RegisteredLocation, RegisteredLocationUpdate, SourceType } from "../../types";
import styles from "./LocationSettingsRow.module.css";

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
  const { t } = useTranslation(["settings", "common"]);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Draft>(() => draftFromLocation(location));
  const [saving, setSaving] = useState(false);

  function sourceTypeLabel(type: SourceType): string {
    return t(`settings:location.sourceTypeLabels.${type}`);
  }

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
            aria-label={t("settings:location.displayNameLabel")}
          />
          {location.kind === "source" && (
            <select
              className={styles.sourceTypeSelect}
              value={draft.sourceType}
              onChange={(e) =>
                setDraft((d) => ({ ...d, sourceType: e.target.value as SourceType }))
              }
              disabled={saving}
              aria-label={t("settings:location.sourceTypeLabel")}
            >
              {SOURCE_TYPE_OPTIONS.map((type) => (
                <option key={type} value={type}>
                  {sourceTypeLabel(type)}
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
              placeholder={t("settings:location.modIdPlaceholder")}
              aria-label={t("settings:location.modIdLabel")}
            />
          )}
        </div>
        <div className={styles.actions}>
          <button
            className="icon-btn"
            onClick={() => void handleSave()}
            disabled={saving || !canSave}
            aria-label={t("common:actions.save")}
            title={t("common:actions.save")}
          >
            {saving ? <Loader2 size={14} className={styles.spinner} /> : <Check size={14} />}
          </button>
          <button
            className="icon-btn"
            onClick={cancelEdit}
            disabled={saving}
            aria-label={t("common:actions.cancel")}
            title={t("common:actions.cancel")}
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
            {isActive && (
              <span className={`${styles.badge} ${styles.badgeActive}`}>
                {t("settings:location.active")}
              </span>
            )}
          </span>
          <span className={styles.pathText} title={location.rootPath}>
            {location.rootPath}
          </span>
          <div className={styles.badgeRow}>
            <span className={styles.badge}>
              {t(location.kind === "project" ? "settings:location.kindProject" : "settings:location.kindSource")}
            </span>
            <span className={styles.badge}>{sourceTypeLabel(location.sourceType)}</span>
            {location.readOnly && (
              <span className={styles.badge}>{t("settings:location.readOnly")}</span>
            )}
          </div>
          {(location.modId || location.gameVersion) && (
            <div className={styles.metaRow}>
              {location.modId && (
                <span className={styles.metaItem}>
                  {t("settings:location.mod", { modId: location.modId })}
                </span>
              )}
              {location.gameVersion && (
                <span className={styles.metaItem}>
                  {t("settings:location.version", { version: location.gameVersion })}
                </span>
              )}
            </div>
          )}
        </div>
        <div className={styles.actions}>
          <button
            className="icon-btn"
            onClick={startEdit}
            aria-label={t("common:actions.edit")}
            title={t("common:actions.edit")}
          >
            <Pencil size={14} />
          </button>
          <button
            className="icon-btn"
            onClick={() => void onRemove(location.id)}
            aria-label={t("common:actions.remove")}
            title={t("common:actions.remove")}
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>
    </div>
  );
}
