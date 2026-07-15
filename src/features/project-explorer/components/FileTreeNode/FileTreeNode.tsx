import { useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  ChevronRight,
  ChevronDown,
  Folder,
  FolderOpen,
  FileCode2,
  FileText,
  File,
} from "lucide-react";
import type {
  FileTreeNode,
  FileTreeFolderNode,
  ProjectFileKind,
} from "../../types";
import styles from "./FileTreeNode.module.css";

export type ContextMenuTarget = {
  kind: "file" | "folder";
  id: string;
  name: string;
  relativePath: string;
  isRoot: boolean;
};

export type EditingNode =
  | {
      mode: "createFile" | "createFolder";
      parentPath: string;
      tempId: string;
      value: string;
      error: string | null;
      /** Initial file contents to write, and whether to open the created file
       * in the editor once created. Used by "New Defs File"/"New Patches File";
       * absent for plain "New File"/"New Folder". */
      contents?: string;
      openAfterCreate?: boolean;
    }
  | {
      mode: "rename";
      nodeId: string;
      kind: "file" | "folder";
      value: string;
      error: string | null;
    }
  | null;

function countFiles(node: FileTreeFolderNode): number {
  return node.children.reduce<number>((acc, child) => {
    return acc + (child.type === "file" ? 1 : countFiles(child));
  }, 0);
}

function fileIcon(fileKind: ProjectFileKind) {
  switch (fileKind) {
    case "xml":
      return FileCode2;
    case "text":
      return FileText;
    default:
      return File;
  }
}

interface FileTreeNodeProps {
  node: FileTreeNode;
  depth: number;
  activeFilePath: string | null;
  expandedFolders: Set<string>;
  editingNode: EditingNode;
  onToggleFolder: (id: string) => void;
  onSelectFile: (relativePath: string) => void;
  onSetEditing: (editing: EditingNode) => void;
  onCommitEdit: (editing: NonNullable<EditingNode>) => void;
  onDeleteNode: (relativePath: string, kind: "file" | "folder") => void;
  onOpenContextMenu: (
    target: ContextMenuTarget,
    event: React.MouseEvent,
  ) => void;
  isRoot?: boolean;
}

export function FileTreeNode({
  node,
  depth,
  activeFilePath,
  expandedFolders,
  editingNode,
  onToggleFolder,
  onSelectFile,
  onSetEditing,
  onCommitEdit,
  onDeleteNode,
  onOpenContextMenu,
  isRoot = false,
}: FileTreeNodeProps) {
  const { t } = useTranslation(["shell", "common"]);
  const indentPx = depth * 16;

  if (node.type === "file") {
    const isActive = node.relativePath === activeFilePath;
    const isRenaming =
      editingNode?.mode === "rename" &&
      editingNode.nodeId === node.relativePath;
    const FileIcon = fileIcon(node.fileKind);

    if (isRenaming) {
      return (
        <InlineEditRow
          indentPx={indentPx + 8}
          icon={<FileIcon size={14} className={styles.icon} />}
          value={editingNode!.value}
          error={editingNode!.error}
          onChange={(v) =>
            onSetEditing({
              ...(editingNode as NonNullable<EditingNode>),
              value: v,
              error: null,
            })
          }
          onCommit={() => onCommitEdit(editingNode!)}
          onCancel={() => onSetEditing(null)}
        />
      );
    }

    const isInactive = node.activeForGameVersion === false;
    const rowTitle = isInactive
      ? t("shell:explorer.inactiveForGameVersion", { relativePath: node.relativePath })
      : node.relativePath;

    return (
      <div
        role="treeitem"
        tabIndex={0}
        className={[
          styles.row,
          isActive ? styles.rowActive : "",
          isInactive ? styles.rowInactive : "",
        ]
          .filter(Boolean)
          .join(" ")}
        style={{ paddingInlineStart: indentPx + 8 }}
        onClick={() => onSelectFile(node.relativePath)}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          onOpenContextMenu(
            {
              kind: "file",
              id: node.id,
              name: node.name,
              relativePath: node.relativePath,
              isRoot: false,
            },
            e,
          );
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelectFile(node.relativePath);
          }
          if (e.key === "F2") {
            e.preventDefault();
            onSetEditing({
              mode: "rename",
              nodeId: node.relativePath,
              kind: "file",
              value: node.name,
              error: null,
            });
          }
          if (e.key === "Delete") {
            e.preventDefault();
            onDeleteNode(node.relativePath, "file");
          }
        }}
        title={rowTitle}
        aria-current={isActive ? "true" : undefined}
      >
        <FileIcon size={14} className={styles.icon} />
        <span className={styles.label}>{node.name}</span>
      </div>
    );
  }

  const isExpanded = expandedFolders.has(node.id);
  const ChevronIcon = isExpanded ? ChevronDown : ChevronRight;
  const FolderIcon = isExpanded ? FolderOpen : Folder;
  const count = countFiles(node);
  const isRenamingFolder =
    editingNode?.mode === "rename" && editingNode.nodeId === node.id;
  const isCreatingChild =
    (editingNode?.mode === "createFile" ||
      editingNode?.mode === "createFolder") &&
    editingNode.parentPath === node.relativePath;

  return (
    <>
      {isRenamingFolder ? (
        <InlineEditRow
          indentPx={indentPx}
          icon={<FolderOpen size={14} className={styles.icon} />}
          value={editingNode!.value}
          error={editingNode!.error}
          onChange={(v) =>
            onSetEditing({
              ...(editingNode as NonNullable<EditingNode>),
              value: v,
              error: null,
            })
          }
          onCommit={() => onCommitEdit(editingNode!)}
          onCancel={() => onSetEditing(null)}
        />
      ) : (
        <div
          role="treeitem"
          tabIndex={0}
          className={styles.row}
          style={{ paddingInlineStart: indentPx }}
          onClick={() => onToggleFolder(node.id)}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onOpenContextMenu(
              {
                kind: "folder",
                id: node.id,
                name: node.name,
                relativePath: node.relativePath,
                isRoot,
              },
              e,
            );
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onToggleFolder(node.id);
            }
            if (!isRoot) {
              if (e.key === "F2") {
                e.preventDefault();
                onSetEditing({
                  mode: "rename",
                  nodeId: node.id,
                  kind: "folder",
                  value: node.name,
                  error: null,
                });
              }
              if (e.key === "Delete") {
                e.preventDefault();
                onDeleteNode(node.relativePath, "folder");
              }
            }
          }}
          aria-expanded={isExpanded}
        >
          <ChevronIcon size={12} className={styles.chevron} />
          <FolderIcon size={14} className={styles.icon} />
          <span className={styles.label}>{node.name}</span>
          {count > 0 && <span className={styles.count}>{count}</span>}
        </div>
      )}
      {isExpanded && (
        <>
          {isCreatingChild && editingNode && (
            <InlineEditRow
              indentPx={(depth + 1) * 16 + 8}
              icon={
                editingNode.mode === "createFolder" ? (
                  <Folder size={14} className={styles.icon} />
                ) : (
                  <File size={14} className={styles.icon} />
                )
              }
              value={editingNode.value}
              error={editingNode.error}
              onChange={(v) =>
                onSetEditing({ ...editingNode, value: v, error: null })
              }
              onCommit={() => onCommitEdit(editingNode)}
              onCancel={() => onSetEditing(null)}
            />
          )}
          {node.children.map((child) => (
            <FileTreeNode
              key={child.id}
              node={child}
              depth={depth + 1}
              activeFilePath={activeFilePath}
              expandedFolders={expandedFolders}
              editingNode={editingNode}
              onToggleFolder={onToggleFolder}
              onSelectFile={onSelectFile}
              onSetEditing={onSetEditing}
              onCommitEdit={onCommitEdit}
              onDeleteNode={onDeleteNode}
              onOpenContextMenu={onOpenContextMenu}
            />
          ))}
        </>
      )}
    </>
  );
}

interface InlineEditRowProps {
  indentPx: number;
  icon: React.ReactNode;
  value: string;
  error: string | null;
  onChange: (v: string) => void;
  onCommit: () => void;
  onCancel: () => void;
}

function InlineEditRow({
  indentPx,
  icon,
  value,
  error,
  onChange,
  onCommit,
  onCancel,
}: InlineEditRowProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  return (
    <div onContextMenu={(e) => e.stopPropagation()}>
      <div className={styles.inlineEditRow} style={{ paddingInlineStart: indentPx }}>
        {icon}
        <input
          ref={inputRef}
          className={styles.inlineInput}
          value={value}
          onChange={(e) => onChange(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              onCommit();
            }
            if (e.key === "Escape") {
              e.preventDefault();
              onCancel();
            }
          }}
          onBlur={onCancel}
          spellCheck={false}
        />
      </div>
      {error && <p className={styles.inlineError}>{error}</p>}
    </div>
  );
}
