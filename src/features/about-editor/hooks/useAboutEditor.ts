import { useCallback, useEffect, useRef, type RefObject } from "react";
import type { XmlEdit } from "../../xml-editor/types/xmlDocument";

const DEPENDENCY_FIELD_ORDER = [
  "packageId",
  "displayName",
  "downloadUrl",
  "steamWorkshopUrl",
  "alternativePackageIds",
];

export interface NewDependencyFields {
  packageId: string;
  displayName?: string;
  downloadUrl?: string;
  steamWorkshopUrl?: string;
}

export interface AboutEditor {
  commitScalar: (fieldName: string, value: string) => void;
  commitList: (fieldName: string, items: string[]) => void;
  insertDependency: (fields: NewDependencyFields) => Promise<unknown>;
  setDependencyField: (listItemNodeId: number, childName: string, value: string) => void;
  removeDependency: (listItemNodeId: number) => void;
  setDependencyAlternativeIds: (listItemNodeId: number, items: string[]) => void;
}

interface ApplyFormEdit {
  (edit: XmlEdit): Promise<string>;
}

/**
 * Builds and commits About.xml edits through the same `applyFormEdit` pipeline
 * (and thus the same undo/redo, save-preview, and validation flow) as the Def
 * form editor, using only existing generic `XmlEdit` operations. `registerFlush`
 * wires an `await`-able flush into `XmlEditorPane`'s mode-switch/save-preview
 * flow, since edits here commit directly rather than through `FormFieldStore`.
 *
 * Scalar text fields only commit `onBlur`, not on every keystroke -- if the user
 * triggers save (e.g. a keyboard shortcut) while still focused in a field, no
 * `applyFormEdit` call has happened yet, so there is nothing in `pendingRef` to
 * await. The flush blurs whatever's still focused inside `containerRef` first,
 * synchronously firing that field's own `onBlur` commit, before awaiting the
 * (now up to date) pending edit.
 */
export function useAboutEditor(
  applyFormEdit: ApplyFormEdit,
  rootNodeId: number,
  containerRef: RefObject<HTMLElement | null>,
  registerFlush?: (flush: () => Promise<void>) => void,
): AboutEditor {
  const pendingRef = useRef<Promise<unknown>>(Promise.resolve());

  const commit = useCallback(
    (edit: XmlEdit) => {
      const result = applyFormEdit(edit).catch(() => undefined);
      pendingRef.current = result;
      return result;
    },
    [applyFormEdit],
  );

  useEffect(() => {
    registerFlush?.(async () => {
      const active = document.activeElement;
      if (active instanceof HTMLElement && containerRef.current?.contains(active)) {
        active.blur();
      }
      await pendingRef.current;
    });
  }, [registerFlush, containerRef]);

  const commitScalar = useCallback(
    (fieldName: string, value: string) => {
      const trimmed = value.trim();
      if (trimmed.length === 0) {
        commit({ type: "removeChildElement", parentNodeId: rootNodeId, childName: fieldName });
      } else {
        commit({
          type: "setChildElementText",
          parentNodeId: rootNodeId,
          childName: fieldName,
          value: trimmed,
        });
      }
    },
    [commit, rootNodeId],
  );

  const commitList = useCallback(
    (fieldName: string, items: string[]) => {
      commit({ type: "setListItems", parentNodeId: rootNodeId, childName: fieldName, items });
    },
    [commit, rootNodeId],
  );

  const insertDependency = useCallback(
    (fields: NewDependencyFields) => {
      const initialChildFields = Object.entries(fields)
        .filter((entry): entry is [string, string] => (entry[1]?.length ?? 0) > 0)
        .map(([name, value]) => ({ name, value }));
      return commit({
        type: "insertObjectListItem",
        parentNodeId: rootNodeId,
        objectPath: [],
        listName: "modDependencies",
        initialChildFields,
        fieldOrder: DEPENDENCY_FIELD_ORDER,
      });
    },
    [commit, rootNodeId],
  );

  const setDependencyField = useCallback(
    (listItemNodeId: number, childName: string, value: string) => {
      const trimmed = value.trim();
      if (trimmed.length === 0) {
        commit({ type: "removeObjectListItemChild", listItemNodeId, childName });
      } else {
        commit({
          type: "setObjectListItemChildText",
          listItemNodeId,
          childName,
          value: trimmed,
          fieldOrder: DEPENDENCY_FIELD_ORDER,
        });
      }
    },
    [commit],
  );

  const removeDependency = useCallback(
    (listItemNodeId: number) => {
      commit({ type: "removeObjectListItem", listItemNodeId, pruneEmptyAncestors: true });
    },
    [commit],
  );

  const setDependencyAlternativeIds = useCallback(
    (listItemNodeId: number, items: string[]) => {
      // `alternativePackageIds` is a direct child of the dependency `<li>` item, not
      // nested inside a further object -- `setListItems` (not `setNestedListItems`,
      // which requires a non-empty objectPath) is the correct op here.
      commit({
        type: "setListItems",
        parentNodeId: listItemNodeId,
        childName: "alternativePackageIds",
        items,
      });
    },
    [commit],
  );

  return {
    commitScalar,
    commitList,
    insertDependency,
    setDependencyField,
    removeDependency,
    setDependencyAlternativeIds,
  };
}
