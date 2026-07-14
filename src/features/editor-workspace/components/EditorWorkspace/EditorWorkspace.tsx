import { useCallback, useEffect, useRef } from "react";
import type { ActiveEditorCommands, OpenFileTab } from "../../types";
import type { SchemaCatalog } from "../../../schema-catalog";
import type { XmlEditorFileRef } from "../../../xml-editor/hooks/useXmlEditorSession";
import { confirmDiscardChanges } from "../../../../lib/confirmDiscardChanges";
import { EditorTabs } from "../EditorTabs/EditorTabs";
import { XmlEditorPane } from "../../../xml-editor";
import { UnsupportedFilePane } from "../UnsupportedFilePane/UnsupportedFilePane";
import styles from "./EditorWorkspace.module.css";

interface EditorWorkspaceProps {
  tabs: OpenFileTab[];
  activeTabKey: string | null;
  projectId: string | undefined;
  catalog: SchemaCatalog | null;
  /** Form Views (issue 06): threaded through to `XmlEditorPane` -> `useFormViews`, which scopes
   * custom-view selection/persistence by `{project, gameVersion, defType}`. */
  gameVersion?: string;
  createDefSignal?: number;
  onActivateTab: (tabKey: string) => void;
  onCloseTab: (tabKey: string) => void;
  onTabDirtyChange: (tabKey: string, dirty: boolean) => void;
  onNavigateDef?: (fileRef: XmlEditorFileRef, nodeId: number | null) => void;
  onActiveCommandsChange?: (commands: ActiveEditorCommands | null) => void;
}

export function EditorWorkspace({
  tabs,
  activeTabKey,
  projectId,
  catalog,
  gameVersion,
  createDefSignal,
  onActivateTab,
  onCloseTab,
  onTabDirtyChange,
  onNavigateDef,
  onActiveCommandsChange,
}: EditorWorkspaceProps) {
  const handleCloseTab = useCallback(async (tabKey: string) => {
    const tab = tabs.find((candidate) => candidate.tabKey === tabKey);
    if (tab?.dirty) {
      const discard = await confirmDiscardChanges(
        "Close this file and discard unsaved changes?",
      );
      if (!discard) return;
    }
    onCloseTab(tabKey);
  }, [tabs, onCloseTab]);

  // Keep a ref so close-command closures always call the latest handleCloseTab.
  const handleCloseTabRef = useRef(handleCloseTab);
  handleCloseTabRef.current = handleCloseTab;
  const activeTabKeyRef = useRef(activeTabKey);
  activeTabKeyRef.current = activeTabKey;

  // For unsupported-file tabs, XmlEditorPane is not rendered, so we publish
  // close-only commands from here. XML tabs are handled by XmlEditorPane itself.
  const isUnsupportedTabActive =
    !!activeTabKey &&
    tabs.find((t) => t.tabKey === activeTabKey)?.editorKind === "unsupported";

  useEffect(() => {
    if (!isUnsupportedTabActive || !onActiveCommandsChange) return;
    onActiveCommandsChange({
      undo: () => {},
      redo: () => {},
      save: async () => {},
      close: async () => {
        const key = activeTabKeyRef.current;
        if (key) await handleCloseTabRef.current(key);
      },
      canUndo: false,
      canRedo: false,
      canSave: false,
      canClose: true,
    });
    return () => onActiveCommandsChange(null);
  }, [isUnsupportedTabActive, onActiveCommandsChange]);

  return (
    <section className={styles.root}>
      <EditorTabs
        tabs={tabs}
        activeTabKey={activeTabKey}
        onActivate={onActivateTab}
        onClose={handleCloseTab}
      />
      <div className={styles.panes}>
        {tabs.length === 0 ? (
          <XmlEditorPane
            projectId={projectId}
            file={undefined}
            catalog={catalog}
            gameVersion={gameVersion}
            hasOpenTabs={false}
            onNavigateDef={onNavigateDef}
          />
        ) : (
          tabs.map((tab) => {
            const active = tab.tabKey === activeTabKey;
            return (
              <div
                key={tab.tabKey}
                className={styles.paneSlot}
                hidden={!active}
              >
                {tab.editorKind === "unsupported" ? (
                  <UnsupportedFilePane file={tab} />
                ) : (
                  <XmlEditorPane
                    projectId={projectId}
                    file={tab}
                    catalog={catalog}
                    gameVersion={gameVersion}
                    hasOpenTabs={tabs.length > 0}
                    active={active}
                    selectedDefNodeId={tab.selectedDefNodeId}
                    selectionRequestId={tab.selectionRequestId}
                    createDefSignal={active ? createDefSignal : undefined}
                    onDirtyChange={(dirty) => onTabDirtyChange(tab.tabKey, dirty)}
                    onNavigateDef={onNavigateDef}
                    onCloseActiveTab={active ? () => handleCloseTab(tab.tabKey) : undefined}
                    onActiveCommandsChange={active ? onActiveCommandsChange : undefined}
                  />
                )}
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}
