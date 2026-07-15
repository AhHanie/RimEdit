import { screen } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import userEvent from "@testing-library/user-event";
import { XmlEditorToolbar } from "./XmlEditorToolbar";
import type { UseXmlEditorSessionReturn } from "../../hooks/useXmlEditorSession";

function makeSession(
  overrides: Partial<UseXmlEditorSessionReturn> = {},
): UseXmlEditorSessionReturn {
  return {
    projectId: "proj1",
    relativePath: "Defs/Things.xml",
    readOnly: false,
    baseRawXml: "<Defs></Defs>",
    currentRawXml: "<Defs></Defs>",
    currentParseDiagnostics: [],
    currentValidationDiagnostics: [],
    isBufferValid: true,
    lastValidSnapshot: {
      rawXml: "<Defs></Defs>",
      parsed: { nodeCount: 1, rootElement: "Defs", profile: "defs", about: null, defs: [] },
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: null,
    },
    mode: "form",
    dirty: false,
    canUndo: false,
    canRedo: false,
    savePreview: null,
    saveError: null,
    saveBusy: false,
    loading: false,
    loadError: null,
    applyFormEdit: vi.fn(),
    applyFormEdits: vi.fn(),
    insertDefFromTemplate: vi.fn(),
    insertDefFromUserTemplate: vi.fn(),
    insertDefFromIndexedDef: vi.fn(),
    saveSelectedDefAsTemplate: vi.fn(),
    listUserDefTemplates: vi.fn(),
    deleteUserDefTemplate: vi.fn(),
    updateRawXml: vi.fn(),
    switchMode: vi.fn(),
    undo: vi.fn(),
    redo: vi.fn(),
    selectDef: vi.fn(),
    requestSavePreview: vi.fn(),
    loadFullSavePreview: vi.fn(),
    confirmSave: vi.fn(),
    clearSavePreview: vi.fn(),
    savePreviewTraceId: null,
    savePreviewStartedAt: null,
    ...overrides,
  };
}

describe("XmlEditorToolbar - Save as Template", () => {
  it("is hidden when the handler is not provided", () => {
    render(<XmlEditorToolbar session={makeSession()} />);
    expect(screen.queryByLabelText("Save as Template")).toBeNull();
  });

  it("is hidden for read-only files even when the handler is provided", () => {
    render(
      <XmlEditorToolbar
        session={makeSession({ readOnly: true })}
        onSaveAsTemplate={vi.fn()}
        canSaveAsTemplate={true}
      />,
    );
    expect(screen.queryByLabelText("Save as Template")).toBeNull();
  });

  it("is disabled when canSaveAsTemplate is false", () => {
    render(
      <XmlEditorToolbar
        session={makeSession()}
        onSaveAsTemplate={vi.fn()}
        canSaveAsTemplate={false}
      />,
    );
    const button = screen.getByLabelText(
      "Save as Template",
    ) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("is enabled and invokes the handler when canSaveAsTemplate is true", async () => {
    const onSaveAsTemplate = vi.fn();
    render(
      <XmlEditorToolbar
        session={makeSession()}
        onSaveAsTemplate={onSaveAsTemplate}
        canSaveAsTemplate={true}
      />,
    );
    const button = screen.getByLabelText(
      "Save as Template",
    ) as HTMLButtonElement;
    expect(button.disabled).toBe(false);
    await userEvent.click(button);
    expect(onSaveAsTemplate).toHaveBeenCalledTimes(1);
  });
});
