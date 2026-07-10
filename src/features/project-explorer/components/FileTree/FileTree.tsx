import React, { useState, useCallback } from "react";
import { confirm } from "@tauri-apps/plugin-dialog";
import type { FileTreeFolderNode } from "../../types";
import { FileTreeNode, type EditingNode, type ContextMenuTarget } from "../FileTreeNode/FileTreeNode";
import { ProjectExplorerContextMenu } from "../ProjectExplorerContextMenu/ProjectExplorerContextMenu";
import {
  DEFS_FILE_TEMPLATE,
  PATCH_FILE_TEMPLATE,
  ensureXmlExtension,
  nextAvailableFileName,
} from "../../utils/newFileTemplates";
import styles from "./FileTree.module.css";

function validateName(name: string, siblings: string[]): string | null {
  if (!name) return "Name cannot be empty";
  if (name === "." || name === "..") return `'${name}' is not a valid name`;
  if (name.includes("/") || name.includes("\\")) return "Name cannot contain path separators";
  if (/[<>:"|?*]/.test(name)) return "Name contains an invalid character";
  // eslint-disable-next-line no-control-regex
  if (/[\x00-\x1f\x7f]/.test(name)) return "Name contains control characters";
  if (siblings.some((s) => s.toLowerCase() === name.toLowerCase()))
    return "A file or folder with this name already exists";
  return null;
}

function findSiblings(root: FileTreeFolderNode, parentPath: string): string[] {
  if (parentPath === "") return root.children.map((c) => c.name);
  // BFS to find the folder node with matching relativePath
  const queue: FileTreeFolderNode[] = [root];
  while (queue.length) {
    const node = queue.shift()!;
    for (const child of node.children) {
      if (child.type === "folder") {
        if (child.relativePath === parentPath) return child.children.map((c) => c.name);
        queue.push(child);
      }
    }
  }
  return [];
}

function findFolderNode(
  node: FileTreeFolderNode,
  relativePath: string,
): FileTreeFolderNode | null {
  if (node.relativePath === relativePath) return node;
  for (const child of node.children) {
    if (child.type === "folder") {
      const found = findFolderNode(child, relativePath);
      if (found) return found;
    }
  }
  return null;
}

function countDescendants(node: FileTreeFolderNode): { files: number; folders: number } {
  let files = 0;
  let folders = 0;
  for (const child of node.children) {
    if (child.type === "file") {
      files++;
    } else {
      folders++;
      const sub = countDescendants(child);
      files += sub.files;
      folders += sub.folders;
    }
  }
  return { files, folders };
}

interface FileTreeProps {
  root: FileTreeFolderNode;
  activeFilePath: string | null;
  expandedFolders: Set<string>;
  onToggleFolder: (id: string) => void;
  onSelectFile: (relativePath: string) => void;
  onCreateFile: (parentPath: string, fileName: string) => Promise<void>;
  onCreateAndOpenFile: (
    parentPath: string,
    fileName: string,
    contents: string,
  ) => Promise<void>;
  onCreateFolder: (parentPath: string, folderName: string) => Promise<void>;
  onRename: (relativePath: string, newName: string, kind: "file" | "folder") => Promise<void>;
  onDelete: (relativePath: string, kind: "file" | "folder") => Promise<void>;
}

export function FileTree({
  root,
  activeFilePath,
  expandedFolders,
  onToggleFolder,
  onSelectFile,
  onCreateFile,
  onCreateAndOpenFile,
  onCreateFolder,
  onRename,
  onDelete,
}: FileTreeProps) {
  const [editingNode, setEditingNode] = useState<EditingNode>(null);
  const [contextMenu, setContextMenu] = useState<{
    target: ContextMenuTarget;
    x: number;
    y: number;
  } | null>(null);

  const handleSetEditing = useCallback((next: EditingNode) => {
    setContextMenu(null);
    setEditingNode(next);
  }, []);

  const handleSelectFile = useCallback(
    (relativePath: string) => {
      setContextMenu(null);
      onSelectFile(relativePath);
    },
    [onSelectFile],
  );

  const handleToggleFolder = useCallback(
    (id: string) => {
      setContextMenu(null);
      onToggleFolder(id);
    },
    [onToggleFolder],
  );

  const handleCommitEdit = useCallback(
    async (editing: NonNullable<EditingNode>) => {
      const siblings =
        editing.mode === "rename"
          ? []
          : findSiblings(
              root,
              editing.mode === "createFile" || editing.mode === "createFolder"
                ? editing.parentPath
                : "",
            );

      // Defs/Patches files must stay ".xml" (so they scan as XML and route to
      // the right editor) even if the user retypes the suggested name.
      const trimmed = editing.value.trim();
      const needsXmlExtension =
        editing.mode === "createFile" &&
        editing.openAfterCreate &&
        trimmed !== "" &&
        trimmed !== "." &&
        trimmed !== "..";
      const name = needsXmlExtension ? ensureXmlExtension(trimmed) : trimmed;

      const err = validateName(name, siblings);
      if (err) {
        setEditingNode({ ...editing, error: err });
        return;
      }

      try {
        if (editing.mode === "createFile") {
          if (editing.openAfterCreate) {
            await onCreateAndOpenFile(editing.parentPath, name, editing.contents ?? "");
          } else {
            await onCreateFile(editing.parentPath, name);
          }
        } else if (editing.mode === "createFolder") {
          await onCreateFolder(editing.parentPath, name);
        } else if (editing.mode === "rename") {
          await onRename(editing.nodeId, name, editing.kind);
        }
        setEditingNode(null);
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : "Operation failed";
        setEditingNode({ ...editing, error: msg });
      }
    },
    [root, onCreateFile, onCreateAndOpenFile, onCreateFolder, onRename],
  );

  const handleDeleteNode = useCallback(
    async (relativePath: string, kind: "file" | "folder") => {
      const title = kind === "file" ? "Delete file?" : "Delete folder?";
      let message: string;
      if (kind === "folder") {
        const node = findFolderNode(root, relativePath);
        if (node) {
          const { files, folders } = countDescendants(node);
          const parts: string[] = [];
          if (files > 0) parts.push(`${files} file${files !== 1 ? "s" : ""}`);
          if (folders > 0) parts.push(`${folders} subfolder${folders !== 1 ? "s" : ""}`);
          const desc = parts.length > 0 ? ` and its contents (${parts.join(", ")})` : "";
          message = `Delete folder "${relativePath}"${desc}? This cannot be undone.`;
        } else {
          message = `Delete folder "${relativePath}" and all its contents? This cannot be undone.`;
        }
      } else {
        message = `Delete file "${relativePath}"? This cannot be undone.`;
      }
      const ok = await confirm(message, { title, kind: "warning", okLabel: "Delete", cancelLabel: "Cancel" });
      if (!ok) return;
      void onDelete(relativePath, kind);
    },
    [root, onDelete],
  );

  const handleOpenContextMenu = useCallback(
    (target: ContextMenuTarget, event: React.MouseEvent) => {
      event.preventDefault();
      setEditingNode(null);
      setContextMenu({ target, x: event.clientX, y: event.clientY });
    },
    [],
  );

  const handleMenuNewFile = useCallback(() => {
    if (!contextMenu || contextMenu.target.kind !== "folder") return;
    const { target } = contextMenu;
    setContextMenu(null);
    if (!expandedFolders.has(target.id)) onToggleFolder(target.id);
    setEditingNode({
      mode: "createFile",
      parentPath: target.relativePath,
      tempId: `new-file-${target.id}`,
      value: "",
      error: null,
    });
  }, [contextMenu, expandedFolders, onToggleFolder]);

  const handleMenuNewFolder = useCallback(() => {
    if (!contextMenu || contextMenu.target.kind !== "folder") return;
    const { target } = contextMenu;
    setContextMenu(null);
    if (!expandedFolders.has(target.id)) onToggleFolder(target.id);
    setEditingNode({
      mode: "createFolder",
      parentPath: target.relativePath,
      tempId: `new-folder-${target.id}`,
      value: "",
      error: null,
    });
  }, [contextMenu, expandedFolders, onToggleFolder]);

  const handleMenuNewDefsFile = useCallback(() => {
    if (!contextMenu || contextMenu.target.kind !== "folder") return;
    const { target } = contextMenu;
    setContextMenu(null);
    if (!expandedFolders.has(target.id)) onToggleFolder(target.id);
    const siblings = findSiblings(root, target.relativePath);
    setEditingNode({
      mode: "createFile",
      parentPath: target.relativePath,
      tempId: `new-defs-file-${target.id}`,
      value: nextAvailableFileName("NewDefs", "xml", siblings),
      error: null,
      contents: DEFS_FILE_TEMPLATE,
      openAfterCreate: true,
    });
  }, [contextMenu, expandedFolders, onToggleFolder, root]);

  const handleMenuNewPatchesFile = useCallback(() => {
    if (!contextMenu || contextMenu.target.kind !== "folder") return;
    const { target } = contextMenu;
    setContextMenu(null);
    if (!expandedFolders.has(target.id)) onToggleFolder(target.id);
    const siblings = findSiblings(root, target.relativePath);
    setEditingNode({
      mode: "createFile",
      parentPath: target.relativePath,
      tempId: `new-patches-file-${target.id}`,
      value: nextAvailableFileName("NewPatches", "xml", siblings),
      error: null,
      contents: PATCH_FILE_TEMPLATE,
      openAfterCreate: true,
    });
  }, [contextMenu, expandedFolders, onToggleFolder, root]);

  const handleMenuRename = useCallback(() => {
    if (!contextMenu) return;
    const { target } = contextMenu;
    setContextMenu(null);
    if (target.kind === "file") {
      setEditingNode({ mode: "rename", nodeId: target.relativePath, kind: "file", value: target.name, error: null });
    } else {
      setEditingNode({ mode: "rename", nodeId: target.id, kind: "folder", value: target.name, error: null });
    }
  }, [contextMenu]);

  const handleMenuDelete = useCallback(() => {
    if (!contextMenu) return;
    const { relativePath, kind } = contextMenu.target;
    setContextMenu(null);
    void handleDeleteNode(relativePath, kind);
  }, [contextMenu, handleDeleteNode]);

  const handleRootContextMenu = useCallback(
    (event: React.MouseEvent) => {
      event.preventDefault();
      setEditingNode(null);
      setContextMenu({
        target: {
          kind: "folder",
          id: root.id,
          name: root.name,
          relativePath: root.relativePath,
          isRoot: true,
        },
        x: event.clientX,
        y: event.clientY,
      });
    },
    [root],
  );

  return (
    <div
      className={styles.root}
      role="tree"
      aria-label="Project files"
      onContextMenu={handleRootContextMenu}
    >
      <FileTreeNode
        node={root}
        depth={0}
        activeFilePath={activeFilePath}
        expandedFolders={expandedFolders}
        editingNode={editingNode}
        onToggleFolder={handleToggleFolder}
        onSelectFile={handleSelectFile}
        onSetEditing={handleSetEditing}
        onCommitEdit={handleCommitEdit}
        onDeleteNode={handleDeleteNode}
        onOpenContextMenu={handleOpenContextMenu}
        isRoot
      />
      {contextMenu && (
        <ProjectExplorerContextMenu
          target={contextMenu.target}
          x={contextMenu.x}
          y={contextMenu.y}
          onNewFile={handleMenuNewFile}
          onNewFolder={handleMenuNewFolder}
          onNewDefsFile={handleMenuNewDefsFile}
          onNewPatchesFile={handleMenuNewPatchesFile}
          onRename={handleMenuRename}
          onDelete={handleMenuDelete}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}
