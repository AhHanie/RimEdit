import { screen, fireEvent } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { FileTree } from "./FileTree";
import type { FileTreeFolderNode } from "../../types";

function root(): FileTreeFolderNode {
  return {
    type: "folder",
    id: "root",
    name: "Project",
    relativePath: "",
    children: [
      {
        type: "file",
        id: "Defs/Thing.xml",
        name: "Thing.xml",
        relativePath: "Defs/Thing.xml",
        folderPath: "Defs",
        extension: "xml",
        sizeBytes: 10,
        fileKind: "xml",
      },
    ],
  };
}

describe("FileTree", () => {
  it("renders a rename failure's code/args through the shared diagnostic catalog, not the raw backend message", async () => {
    const onRename = vi.fn().mockRejectedValue({
      // Same wire shape a rejected Tauri command actually carries (`AppError`'s `code`/`args`) --
      // deliberately different `message` from the catalog text below, so a passing assertion
      // proves the row renders the translated code/args lookup, not this compatibility fallback.
      code: "invalid_location_path",
      message: "backend raw message that must not be shown",
      args: { path: "Defs/Thing.xml" },
    });

    render(
      <FileTree
        root={root()}
        activeFilePath={null}
        expandedFolders={new Set(["root"])}
        onToggleFolder={vi.fn()}
        onSelectFile={vi.fn()}
        onCreateFile={vi.fn()}
        onCreateAndOpenFile={vi.fn()}
        onCreateFolder={vi.fn()}
        onRename={onRename}
        onDelete={vi.fn()}
      />,
    );

    const row = screen.getByText("Thing.xml").closest('[role="treeitem"]')!;
    fireEvent.keyDown(row, { key: "F2" });

    const input = await screen.findByDisplayValue("Thing.xml");
    fireEvent.change(input, { target: { value: "Renamed.xml" } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(
      await screen.findByText('"Defs/Thing.xml" is not a valid location path.'),
    ).toBeTruthy();
    expect(
      screen.queryByText("backend raw message that must not be shown"),
    ).toBeFalsy();
  });
});
