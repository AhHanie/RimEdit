import { screen } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { PatchOperationNodeRow } from "./PatchOperationNodeRow";
import type { PatchOperationNode } from "../../types/patchFile";

function unknownNode(rawXml: string): PatchOperationNode {
  return {
    id: 1,
    className: "SomeThirdPartyOperation",
    success: "normal",
    attributes: [],
    kind: { type: "unknown", data: { rawXml } },
    span: null,
  };
}

describe("PatchOperationNodeRow", () => {
  it("forces dir=ltr on the raw XML textarea for an unknown/unsupported operation class", () => {
    // XML is machine-readable syntax, not natural-language prose -- this must stay LTR even once
    // a future RTL locale flips `dir` on `<html>` (docs/i18n/issues/08-editor-and-patch-ui-
    // migration.md's "keep code editor/XML/XPath controls dir=ltr by semantic policy").
    render(
      <ul>
        <PatchOperationNodeRow
          node={unknownNode("<li Class=\"SomeThirdPartyOperation\" />")}
          catalog={null}
          readOnly={false}
          projectId={null}
          depth={0}
          generateId={() => 2}
          onChange={() => {}}
          onRemove={() => {}}
        />
      </ul>,
    );

    const textarea = screen.getByDisplayValue('<li Class="SomeThirdPartyOperation" />');
    expect(textarea.getAttribute("dir")).toBe("ltr");
  });
});
