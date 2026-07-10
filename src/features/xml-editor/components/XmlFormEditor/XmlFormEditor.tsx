import React, { useCallback, useState, useSyncExternalStore } from "react";
import type { DefEditorView } from "../../types/xmlDocument";
import type { XmlEditorSnapshot } from "../../types/editorSession";
import type { XmlFormApi } from "../../hooks/useXmlFormController";
import type {
  FormFieldModel,
  FormSectionDefaults,
  FormValue,
} from "../../types/editorForm";
import type { FormFieldStore } from "../../lib/formFieldStore";
import { useXmlEditorContext } from "../../context/XmlEditorContext";
import { FormFieldControl } from "../FormFieldControl/FormFieldControl";
import { UnknownXmlFields } from "../UnknownXmlFields/UnknownXmlFields";
import { GraphicDataPreview } from "../GraphicDataPreview/GraphicDataPreview";
import styles from "./XmlFormEditor.module.css";

function formValueText(value: FormValue): string {
  switch (value.kind) {
    case "boolean":
      return value.value ? "true" : "false";
    case "enum":
    case "readonly":
    case "scalar":
      return value.value;
    case "list":
      return value.items.join("\n");
    case "flags":
      return [...value.selected, ...value.custom].join("\n");
    case "namedMap":
      return value.entries.map((e) => `${e.key}=${e.value}`).join("\n");
    case "typedReferenceList":
      return value.items.map((i) => `${i.defType}:${i.defName}`).join("\n");
    case "objectList":
      return `(${value.items.length} item${value.items.length === 1 ? "" : "s"})`;
  }
}

function getFieldSectionPath(model: FormFieldModel): string[] | null {
  const path = model.path;
  switch (path.kind) {
    case "nestedObjectField":
    case "nestedListItems":
    case "nestedAttribute":
      return path.objectPath;
    case "namedMap":
    case "objectList":
      return path.objectPath.length > 0 ? path.objectPath : null;
    default:
      return null;
  }
}

function getSectionMeta(
  model: FormFieldModel,
  sectionPath: string[],
): FormSectionDefaults | undefined {
  const key = sectionPath.join(".");
  return model.sectionDefaults.find((s) => s.path.join(".") === key);
}

interface Props {
  snapshot: XmlEditorSnapshot;
  selectedDefNodeId: number | null;
  onSelectDef: (nodeId: number | null) => Promise<void>;
  formApi: XmlFormApi;
}

function formatSectionLabel(objectPath: string[]): string {
  return objectPath
    .map((part) =>
      part.replace(/([A-Z])/g, " $1").replace(/^[a-z]/, (c) => c.toUpperCase()),
    )
    .join(" / ");
}

function sectionStateKey(defNodeId: number, sectionPath: string[]): string {
  return `${defNodeId}:${sectionPath.join(".")}`;
}

function computeInitialCollapsed(
  defaultCollapsed: boolean | undefined,
  hasData: boolean | undefined,
): boolean {
  if (defaultCollapsed !== undefined) return defaultCollapsed;
  // Only collapse when we explicitly know there is no data.
  // Undefined means metadata wasn't set (e.g. directly-constructed test fields) - treat as open.
  if (hasData === false) return true;
  return false;
}

/** Finds the value of a graphicData child field by subscribing to it from the store. */
function useGraphicDataFieldValue(
  store: FormFieldStore,
  models: FormFieldModel[],
  fieldName: string,
): string {
  const model = models.find(
    (m) =>
      m.path.kind === "nestedObjectField" &&
      m.path.objectPath.length === 1 &&
      m.path.objectPath[0] === "graphicData" &&
      m.path.fieldName === fieldName,
  );
  const id = model?.id ?? null;
  const subscribe = useCallback(
    (cb: () => void) => (id ? store.subscribeField(id, cb) : () => undefined),
    [store, id],
  );
  const getSnapshot = useCallback(
    () => (id ? store.getFieldState(id) : undefined),
    [store, id],
  );
  const field = useSyncExternalStore(subscribe, getSnapshot);
  return field ? formValueText(field.value) : "";
}

/** Live graphicData preview - re-renders only when its three source fields change. */
function GraphicDataPreviewConnected({
  store,
  models,
  projectId,
}: {
  store: FormFieldStore;
  models: FormFieldModel[];
  projectId?: string;
}) {
  const texPath = useGraphicDataFieldValue(store, models, "texPath");
  const graphicClass = useGraphicDataFieldValue(store, models, "graphicClass");
  const maskPath = useGraphicDataFieldValue(store, models, "maskPath");
  return (
    <GraphicDataPreview
      projectId={projectId}
      texPath={texPath}
      graphicClass={graphicClass}
      maskPath={maskPath || undefined}
    />
  );
}

export const XmlFormEditor = React.memo(function XmlFormEditor({
  snapshot,
  selectedDefNodeId,
  onSelectDef,
  formApi,
}: Props) {
  const { projectId } = useXmlEditorContext();
  const { parsed } = snapshot;
  const store = formApi.store;

  // Stable ordered model list - changes only on a structural rebuild, not on value edits.
  const models = useSyncExternalStore(
    store.subscribeStructure,
    store.getModels,
  );

  // Tracks explicit section toggle overrides, keyed by `{defNodeId}:{sectionPath}`.
  // Sections without an entry fall back to the computed default from schema/data metadata.
  const [explicitCollapsed, setExplicitCollapsed] = useState<
    Record<string, boolean>
  >({});

  if (!parsed || parsed.defs.length === 0) {
    return (
      <div className={styles.empty}>
        <p>No Def found in this file.</p>
      </div>
    );
  }

  const selectedDef: DefEditorView =
    parsed.defs.find((d) => d.nodeId === selectedDefNodeId) ?? parsed.defs[0];

  const knownModels = models.filter((m) => m.control !== "readonlyUnknown");
  const unknownModels = models.filter((m) => m.control === "readonlyUnknown");
  const unknownFields = unknownModels
    .map((m) => store.getFieldState(m.id))
    .filter((f): f is NonNullable<typeof f> => !!f);

  function isSectionCollapsedForPath(
    stateKey: string,
    meta: FormSectionDefaults | undefined,
  ): boolean {
    if (Object.prototype.hasOwnProperty.call(explicitCollapsed, stateKey)) {
      return explicitCollapsed[stateKey];
    }
    return computeInitialCollapsed(meta?.defaultCollapsed, meta?.hasData);
  }

  function toggleSection(key: string, currentlyCollapsed: boolean) {
    setExplicitCollapsed((prev) => ({ ...prev, [key]: !currentlyCollapsed }));
  }

  function renderSectionContent(
    sectionModels: FormFieldModel[],
    parentPath: string[],
  ): React.ReactNode[] {
    const nodes: React.ReactNode[] = [];
    const depth = parentPath.length;
    let i = 0;

    while (i < sectionModels.length) {
      const model = sectionModels[i];
      const sectionPath = getFieldSectionPath(model);

      if (!sectionPath || sectionPath.length <= depth) {
        nodes.push(
          <FormFieldControl
            key={model.id}
            fieldId={model.id}
            store={store}
            formApi={formApi}
            nestedDepth={depth}
          />,
        );
        i++;
      } else {
        const subPath = sectionPath.slice(0, depth + 1);
        const subPathKey = subPath.join(".");

        const subModels: FormFieldModel[] = [];
        while (i < sectionModels.length) {
          const m = sectionModels[i];
          const fp = getFieldSectionPath(m);
          if (
            fp &&
            fp.length > depth &&
            fp.slice(0, depth + 1).join(".") === subPathKey
          ) {
            subModels.push(m);
            i++;
          } else {
            break;
          }
        }

        const stateKey = sectionStateKey(selectedDef.nodeId, subPath);
        const meta = getSectionMeta(subModels[0], subPath);
        const collapsed = isSectionCollapsedForPath(stateKey, meta);
        const safeKey = stateKey.replace(/[^a-z0-9]/gi, "-");
        const headerId = `section-header-${safeKey}`;
        const sectionId = `section-content-${safeKey}`;

        nodes.push(
          <button
            key={`header-${stateKey}`}
            id={headerId}
            className={styles.nestedSectionHeader}
            onClick={() => toggleSection(stateKey, collapsed)}
            aria-expanded={!collapsed}
            aria-controls={sectionId}
          >
            <span aria-hidden="true" className={styles.sectionToggleIcon}>
              {collapsed ? "▶" : "▼"}
            </span>
            <span>{formatSectionLabel(subPath)}</span>
          </button>,
        );

        nodes.push(
          <div key={`content-${stateKey}`} id={sectionId}>
            {!collapsed && (
              <>
                {subPathKey === "graphicData" && (
                  <GraphicDataPreviewConnected
                    store={store}
                    models={models}
                    projectId={projectId}
                  />
                )}
                {renderSectionContent(subModels, subPath)}
              </>
            )}
          </div>,
        );
      }
    }

    return nodes;
  }

  const fieldNodes = renderSectionContent(knownModels, []);

  return (
    <div className={styles.root}>
      {parsed.defs.length > 1 && (
        <div className={styles.defSelector}>
          <label htmlFor="def-selector" className={styles.defSelectorLabel}>
            Def
          </label>
          <select
            id="def-selector"
            className={styles.defSelectorInput}
            value={selectedDef.nodeId}
            onChange={(e) => void onSelectDef(Number(e.target.value))}
          >
            {parsed.defs.map((d) => (
              <option key={d.nodeId} value={d.nodeId}>
                {d.defName ?? d.defType} ({d.defType})
              </option>
            ))}
          </select>
        </div>
      )}

      <div className={styles.fields}>
        {formApi.formError && (
          <p className={styles.formError}>{formApi.formError}</p>
        )}

        {knownModels.length === 0 && unknownModels.length === 0 && (
          <p className={styles.noFields}>
            No schema available for <strong>{selectedDef.defType}</strong>.
          </p>
        )}

        {fieldNodes}

        <UnknownXmlFields fields={unknownFields} />
      </div>
    </div>
  );
});
