import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2 } from "lucide-react";
import type { AboutDependency, ValidationDiagnostic } from "../../../xml-editor/types/xmlDocument";
import type { AboutEditor, NewDependencyFields } from "../../hooks/useAboutEditor";
import { diagnosticsForNode } from "../../lib/aboutValidationText";
import { renderDiagnostic } from "../../../../i18n/diagnostics";
import { AboutStringListField } from "../AboutStringListField/AboutStringListField";
import styles from "./AboutDependencySection.module.css";

interface Props {
  dependencies: AboutDependency[];
  diagnostics: ValidationDiagnostic[];
  readOnly: boolean;
  editor: AboutEditor;
}

export function AboutDependencySection({ dependencies, diagnostics, readOnly, editor }: Props) {
  const [adding, setAdding] = useState(false);
  const { t } = useTranslation("editor");

  return (
    <section className={styles.section}>
      <div className={styles.headingRow}>
        <h3 className={styles.heading}>{t("about.dependencies.heading")}</h3>
        {!readOnly && !adding && (
          <button type="button" className={styles.addButton} onClick={() => setAdding(true)}>
            <Plus size={12} /> {t("about.dependencies.addDependency")}
          </button>
        )}
      </div>
      {dependencies.length === 0 && !adding && (
        <p className={styles.empty}>{t("about.dependencies.empty")}</p>
      )}
      {dependencies.map((dep) => (
        <DependencyRow
          key={dep.nodeId}
          dependency={dep}
          readOnly={readOnly}
          editor={editor}
          diagnostics={diagnosticsForNode(diagnostics, dep.nodeId)}
        />
      ))}
      {adding && (
        <NewDependencyRow
          onCancel={() => setAdding(false)}
          onAdd={async (fields) => {
            await editor.insertDependency(fields);
            setAdding(false);
          }}
        />
      )}
    </section>
  );
}

interface RowProps {
  dependency: AboutDependency;
  readOnly: boolean;
  editor: AboutEditor;
  diagnostics: ValidationDiagnostic[];
}

function DependencyRow({ dependency, readOnly, editor, diagnostics }: RowProps) {
  // Two separate single-namespace hooks (rather than `useTranslation(["diagnostics", "editor"])`
  // with `"editor:key"`-prefixed lookups) because TypeScript's cross-namespace `"ns:key"` typed
  // overload becomes unusable once a namespace's key set grows large (see `editor.json`) -- it
  // silently fails to type-check even for keys that genuinely exist. Bare keys scoped to their
  // own single-namespace `t` avoid that entirely.
  const { i18n } = useTranslation("diagnostics");
  const { t } = useTranslation("editor");
  const [packageId, setPackageId] = useState(dependency.packageId ?? "");
  const [displayName, setDisplayName] = useState(dependency.displayName ?? "");
  const [downloadUrl, setDownloadUrl] = useState(dependency.downloadUrl ?? "");
  const [steamWorkshopUrl, setSteamWorkshopUrl] = useState(dependency.steamWorkshopUrl ?? "");

  useEffect(() => setPackageId(dependency.packageId ?? ""), [dependency.packageId]);
  useEffect(() => setDisplayName(dependency.displayName ?? ""), [dependency.displayName]);
  useEffect(() => setDownloadUrl(dependency.downloadUrl ?? ""), [dependency.downloadUrl]);
  useEffect(
    () => setSteamWorkshopUrl(dependency.steamWorkshopUrl ?? ""),
    [dependency.steamWorkshopUrl],
  );

  return (
    <div className={styles.row}>
      <div className={styles.rowGrid}>
        <input
          className={styles.input}
          placeholder="packageId"
          value={packageId}
          readOnly={readOnly}
          onChange={(e) => setPackageId(e.target.value)}
          onBlur={() => {
            if (!readOnly && packageId !== (dependency.packageId ?? "")) {
              editor.setDependencyField(dependency.nodeId, "packageId", packageId);
            }
          }}
        />
        <input
          className={styles.input}
          placeholder="displayName"
          value={displayName}
          readOnly={readOnly}
          onChange={(e) => setDisplayName(e.target.value)}
          onBlur={() => {
            if (!readOnly && displayName !== (dependency.displayName ?? "")) {
              editor.setDependencyField(dependency.nodeId, "displayName", displayName);
            }
          }}
        />
        <input
          className={styles.input}
          placeholder="downloadUrl"
          value={downloadUrl}
          readOnly={readOnly}
          onChange={(e) => setDownloadUrl(e.target.value)}
          onBlur={() => {
            if (!readOnly && downloadUrl !== (dependency.downloadUrl ?? "")) {
              editor.setDependencyField(dependency.nodeId, "downloadUrl", downloadUrl);
            }
          }}
        />
        <input
          className={styles.input}
          placeholder="steamWorkshopUrl"
          value={steamWorkshopUrl}
          readOnly={readOnly}
          onChange={(e) => setSteamWorkshopUrl(e.target.value)}
          onBlur={() => {
            if (!readOnly && steamWorkshopUrl !== (dependency.steamWorkshopUrl ?? "")) {
              editor.setDependencyField(dependency.nodeId, "steamWorkshopUrl", steamWorkshopUrl);
            }
          }}
        />
        {!readOnly && (
          <button
            type="button"
            className={styles.removeBtn}
            onClick={() => editor.removeDependency(dependency.nodeId)}
            aria-label={t("about.dependencies.removeDependency")}
          >
            <Trash2 size={13} />
          </button>
        )}
      </div>
      <AboutStringListField
        label={t("about.dependencies.alternativePackageIds")}
        items={dependency.alternativePackageIds}
        readOnly={readOnly}
        placeholder="old.package.id"
        onCommit={(items) => editor.setDependencyAlternativeIds(dependency.nodeId, items)}
      />
      {diagnostics.length > 0 && (
        <ul className={styles.diagnostics}>
          {diagnostics.map((d, i) => (
            <li key={i} className={d.severity === "Error" ? styles.diagError : styles.diagWarning}>
              {renderDiagnostic(d, i18n)}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function NewDependencyRow({
  onCancel,
  onAdd,
}: {
  onCancel: () => void;
  onAdd: (fields: NewDependencyFields) => Promise<void>;
}) {
  const [packageId, setPackageId] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [downloadUrl, setDownloadUrl] = useState("");
  const [steamWorkshopUrl, setSteamWorkshopUrl] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const { t: tCommon } = useTranslation("common");

  return (
    <div className={styles.row}>
      <div className={styles.rowGrid}>
        <input
          className={styles.input}
          placeholder="packageId"
          value={packageId}
          onChange={(e) => setPackageId(e.target.value)}
        />
        <input
          className={styles.input}
          placeholder="displayName"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
        />
        <input
          className={styles.input}
          placeholder="downloadUrl"
          value={downloadUrl}
          onChange={(e) => setDownloadUrl(e.target.value)}
        />
        <input
          className={styles.input}
          placeholder="steamWorkshopUrl"
          value={steamWorkshopUrl}
          onChange={(e) => setSteamWorkshopUrl(e.target.value)}
        />
      </div>
      <div className={styles.newRowActions}>
        <button
          type="button"
          className={styles.saveBtn}
          disabled={!packageId.trim() || submitting}
          onClick={async () => {
            setSubmitting(true);
            await onAdd({
              packageId: packageId.trim(),
              displayName: displayName.trim() || undefined,
              downloadUrl: downloadUrl.trim() || undefined,
              steamWorkshopUrl: steamWorkshopUrl.trim() || undefined,
            });
          }}
        >
          {tCommon("actions.add")}
        </button>
        <button type="button" className={styles.cancelBtn} onClick={onCancel} disabled={submitting}>
          {tCommon("actions.cancel")}
        </button>
      </div>
    </div>
  );
}
