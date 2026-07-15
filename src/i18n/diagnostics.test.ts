import { describe, it, expect } from "vitest";
import { createI18nInstance } from "./index";
import {
  formatCommandError,
  getDiagnosticTechnicalDetail,
  renderDiagnostic,
  renderDiagnosticCountSummary,
  renderDiagnosticLocation,
  renderDiagnosticSectionHeading,
  renderDiagnosticSeverity,
  renderDiagnosticSource,
} from "./diagnostics";

describe("renderDiagnostic", () => {
  it("renders a known code, interpolating args verbatim", () => {
    const i18n = createI18nInstance();
    const text = renderDiagnostic(
      { code: "location_not_found", args: { path: "loc-1" } },
      i18n,
    );
    expect(text).toBe('The location "loc-1" could not be found.');
  });

  it("never translates literal argument values", () => {
    const i18n = createI18nInstance();
    const text = renderDiagnostic(
      { code: "validation_missing_required_field", args: { fieldName: "defName" } },
      i18n,
    );
    expect(text).toBe("defName is required.");
  });

  it("falls back to the compatibility message when the code has no catalog entry", () => {
    const i18n = createI18nInstance();
    const text = renderDiagnostic(
      { code: "template_locked", message: "Template is in use" },
      i18n,
    );
    expect(text).toBe("Template is in use");
  });

  it("falls back to a generic message naming the code when neither a translation nor a message exists", () => {
    const i18n = createI18nInstance();
    const text = renderDiagnostic({ code: "some_future_unmigrated_code" }, i18n);
    expect(text).toBe("An unexpected error occurred (code: some_future_unmigrated_code).");
  });

  it("falls back to a fully generic message when there is no code and no message", () => {
    const i18n = createI18nInstance();
    expect(renderDiagnostic({}, i18n)).toBe("An unexpected error occurred.");
  });

  it("uses the message directly when no code is present at all", () => {
    const i18n = createI18nInstance();
    expect(renderDiagnostic({ message: "raw parser detail" }, i18n)).toBe("raw parser detail");
  });

  it("renders the frontend-raised form_view_no_active_project code through the catalog", () => {
    // Covers `DiagnosticError`-raised, frontend-detected conditions (see `src/lib/diagnostics.ts`
    // and `useCustomFormViews.ts`'s `noActiveProjectError`) -- not just backend-originated codes.
    const i18n = createI18nInstance();
    const text = renderDiagnostic(
      { code: "form_view_no_active_project", message: "No active project to save a custom Form View to." },
      i18n,
    );
    expect(text).toBe("No active project is selected. Select a project before managing custom Form Views.");
  });

  it("renders other frontend-raised DiagnosticError codes through the catalog", () => {
    // Covers round 13's fix for `useProjectFiles.ts`, `useFormViews.ts`'s
    // `saveOverrideAsCustomView`, and `useXmlEditorSession.ts` -- each threw a raw, untranslatable
    // `new Error(...)` for a known/enumerable frontend precondition; see the `DiagnosticError`
    // helpers in `src/features/project-explorer/lib/projectFilesErrors.ts`,
    // `src/features/form-views/lib/formViewErrors.ts`, and
    // `src/features/xml-editor/lib/xmlEditorSessionErrors.ts`.
    const i18n = createI18nInstance();
    expect(
      renderDiagnostic({ code: "project_file_no_active_project", message: "No active project" }, i18n),
    ).toBe("No active project is selected. Select a project before managing files.");
    expect(
      renderDiagnostic(
        { code: "form_view_no_unsaved_changes", message: "No unsaved Form View changes to save." },
        i18n,
      ),
    ).toBe("There are no unsaved Form View changes to save.");
    expect(
      renderDiagnostic(
        { code: "xml_editor_session_no_active_file", message: "Cannot insert def: read-only or no active file." },
        i18n,
      ),
    ).toBe("This action isn't available: the document is read-only or no file is open.");
    expect(
      renderDiagnostic({ code: "xml_editor_session_no_def_selected", message: "No Def is selected." }, i18n),
    ).toBe("No Def is selected.");
    expect(
      renderDiagnostic(
        { code: "xml_editor_session_no_active_project", message: "Cannot delete template: no active project." },
        i18n,
      ),
    ).toBe("No active project is selected.");
  });

  it("falls back to the generic code fallback (not a raw {{arg}} placeholder) when a catalog code is missing required args", () => {
    const i18n = createI18nInstance();
    // "location_not_found" requires {{path}}; omitting `args` entirely must not leak the raw
    // catalog string with its placeholder unresolved.
    const text = renderDiagnostic({ code: "location_not_found" }, i18n);
    expect(text).not.toMatch(/\{\{.*\}\}/);
    expect(text).toBe("An unexpected error occurred (code: location_not_found).");
  });

  it("falls back to the compatibility message when a catalog code is missing required args but a message is present", () => {
    const i18n = createI18nInstance();
    const text = renderDiagnostic(
      { code: "location_not_found", message: "Location not found: loc-1" },
      i18n,
    );
    expect(text).not.toMatch(/\{\{.*\}\}/);
    expect(text).toBe("Location not found: loc-1");
  });

  it("falls back to the app-wide singleton when passed a non-functional i18n instance", () => {
    const text = renderDiagnostic({ code: "location_not_found", args: { path: "loc-1" } }, {} as never);
    expect(text).toBe('The location "loc-1" could not be found.');
  });

  // Every `ProjectFileError` variant (src-tauri/src/project_files/error.rs) now has a catalog
  // entry, including the args-free ones -- these used to fall through to the compatibility
  // `message` (assembled English from the Rust `Display` impl) because their code was entirely
  // absent from diagnostics.json, not because they legitimately have no translatable text.
  it("renders every ProjectFileError code from the catalog, not the compatibility message", () => {
    const i18n = createI18nInstance();
    expect(
      renderDiagnostic({ code: "project_file_scan_failed", message: "File scan failed: boom" }, i18n),
    ).toBe("The project files could not be scanned.");
    expect(
      renderDiagnostic({ code: "project_file_outside_root", message: "File path is outside project root" }, i18n),
    ).toBe("The file path is outside the project root.");
    expect(
      renderDiagnostic({ code: "unsupported_project_file", message: "File type is not supported" }, i18n),
    ).toBe("This file type is not supported.");
    expect(
      renderDiagnostic({ code: "cannot_modify_root", message: "Cannot modify the project root" }, i18n),
    ).toBe("The project root cannot be modified.");
  });
});

describe("getDiagnosticTechnicalDetail", () => {
  it("joins code and message for a copy/support view", () => {
    expect(
      getDiagnosticTechnicalDetail({ code: "location_not_found", message: "Location not found: loc-1" }),
    ).toBe("location_not_found: Location not found: loc-1");
  });

  it("returns just the code when there is no message", () => {
    expect(getDiagnosticTechnicalDetail({ code: "location_not_found" })).toBe("location_not_found");
  });

  it("returns null when there is neither a code nor a message", () => {
    expect(getDiagnosticTechnicalDetail({})).toBeNull();
  });
});

describe("formatCommandError", () => {
  it("renders a structured AppError-shaped rejection through the same renderer", () => {
    const i18n = createI18nInstance();
    const rejection = {
      code: "location_not_found",
      message: "Location not found: loc-1",
      details: null,
      args: { path: "loc-1" },
    };
    expect(formatCommandError(rejection, i18n)).toBe('The location "loc-1" could not be found.');
  });

  it("falls back to the raw message for a legacy rejection with no code", () => {
    expect(formatCommandError({ message: "boom" })).toBe("boom");
  });

  it("stringifies an arbitrary thrown value with neither code nor message", () => {
    expect(formatCommandError("plain string failure")).toBe("plain string failure");
  });

  it("unwraps a JSON-encoded string rejection and renders its code/args through the catalog", () => {
    // Some Tauri command rejections still surface as a raw JSON-encoded string rather than a
    // plain object (e.g. a command that returns `Result<T, String>`). The shared function must
    // unwrap that string form itself so every caller benefits, not just callers that add their
    // own local unwrap (see `CreateDefWizard.tsx`'s now-thin `formatWizardError`).
    const i18n = createI18nInstance();
    const rejection = JSON.stringify({
      code: "location_not_found",
      message: "backend raw message that must not be shown",
      args: { path: "loc-1" },
    });
    expect(formatCommandError(rejection, i18n)).toBe('The location "loc-1" could not be found.');
  });

  it("does not leak raw JSON text when a JSON-encoded string rejection has no usable code or message", () => {
    // A JSON-encoded string that decodes to an object without `code`/`message` (e.g. an array or
    // a plain data payload) should fall back to the original raw string, not throw or produce
    // `[object Object]`.
    const rejection = JSON.stringify({ unrelated: "value" });
    expect(formatCommandError(rejection)).toBe(rejection);
  });
});

describe("renderDiagnosticSource", () => {
  it("renders the parse and validation source labels", () => {
    const i18n = createI18nInstance();
    expect(renderDiagnosticSource("parse", i18n)).toBe("Parse");
    expect(renderDiagnosticSource("validation", i18n)).toBe("Validation");
  });
});

describe("renderDiagnosticSeverity", () => {
  it("renders a severity label regardless of wire casing", () => {
    const i18n = createI18nInstance();
    expect(renderDiagnosticSeverity("Error", i18n)).toBe("Error");
    expect(renderDiagnosticSeverity("warning", i18n)).toBe("Warning");
  });
});

describe("renderDiagnosticLocation", () => {
  it("renders line and column together", () => {
    const i18n = createI18nInstance();
    expect(renderDiagnosticLocation({ line: 3, column: 5 }, i18n)).toBe("line 3:5");
  });

  it("renders line alone when column is absent", () => {
    const i18n = createI18nInstance();
    expect(renderDiagnosticLocation({ line: 3, column: null }, i18n)).toBe("line 3");
  });

  it("returns null when there is no line", () => {
    const i18n = createI18nInstance();
    expect(renderDiagnosticLocation({ line: null }, i18n)).toBeNull();
  });
});

describe("renderDiagnosticCountSummary", () => {
  it("pluralizes a single issue with no error/warning breakdown", () => {
    const i18n = createI18nInstance();
    expect(renderDiagnosticCountSummary({ total: 1, errorCount: 0, warningCount: 0 }, i18n)).toBe("1 issue");
  });

  it("pluralizes multiple issues with error and warning counts", () => {
    const i18n = createI18nInstance();
    expect(renderDiagnosticCountSummary({ total: 3, errorCount: 1, warningCount: 2 }, i18n)).toBe(
      "3 issues, 1 error, 2 warnings",
    );
  });
});

describe("renderDiagnosticSectionHeading", () => {
  it("renders a diagnostics section heading with the count", () => {
    const i18n = createI18nInstance();
    expect(renderDiagnosticSectionHeading(4, i18n)).toBe("Diagnostics (4)");
  });
});
