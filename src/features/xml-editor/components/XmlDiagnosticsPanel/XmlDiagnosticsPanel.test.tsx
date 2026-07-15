import { describe, it, expect } from "vitest";
import { fireEvent, screen } from "@testing-library/react";
import { renderWithI18n } from "../../../../i18n/testing/renderWithI18n";
import { XmlDiagnosticsPanel } from "./XmlDiagnosticsPanel";
import type { ParseDiagnostic, ValidationDiagnostic } from "../../types/xmlDocument";

describe("XmlDiagnosticsPanel", () => {
  it("renders nothing when there are no diagnostics", () => {
    const { container } = renderWithI18n(<XmlDiagnosticsPanel diagnostics={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders a pluralized count summary and expands to show a known-code diagnostic", () => {
    const diagnostics: ValidationDiagnostic[] = [
      {
        relativePath: "Defs/Things.xml",
        nodeId: 1,
        line: 3,
        column: 5,
        severity: "Error",
        message: "Required field 'label' is missing from ThingDef.",
        code: "validation_missing_required_field",
        defType: "ThingDef",
        defName: "Wall",
        fieldPath: "label",
        blocking: true,
        args: { fieldName: "label", defType: "ThingDef" },
      },
    ];

    renderWithI18n(<XmlDiagnosticsPanel diagnostics={diagnostics} />);

    expect(screen.getByText("1 issue, 1 error")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Toggle diagnostics list" }));

    expect(screen.getByText("label is required.")).toBeTruthy();
    expect(screen.getByText("line 3:5")).toBeTruthy();
    expect(screen.getByText("Validation")).toBeTruthy();
    expect(screen.getByText("Wall:")).toBeTruthy();
  });

  it("falls back to the raw message for an unmigrated code, never showing raw message text as a translated code lookup", () => {
    const diagnostics: ParseDiagnostic[] = [
      {
        relativePath: "Defs/Things.xml",
        line: null,
        column: null,
        byteOffset: null,
        message: "some third-party parser detail",
        code: "parse_some_future_condition",
      },
    ];

    renderWithI18n(<XmlDiagnosticsPanel diagnostics={diagnostics} />);
    fireEvent.click(screen.getByRole("button", { name: "Toggle diagnostics list" }));

    expect(screen.getByText("some third-party parser detail")).toBeTruthy();
    expect(screen.getByText("Parse")).toBeTruthy();
  });
});
