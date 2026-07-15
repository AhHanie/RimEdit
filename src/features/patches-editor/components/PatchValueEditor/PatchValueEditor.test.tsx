import { fireEvent, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { useLocale } from "../../../../i18n/LocaleProvider";
import { PatchValueEditor } from "./PatchValueEditor";
import type { SchemaCatalog } from "../../../schema-catalog";
import type { XPathCompletionResult } from "../../types/xpathCompletion";
import type { XmlChildView } from "../../../xml-editor";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Mocks only `useLocale` (keeping the real `LocaleProvider`/`I18nextProvider` tree the other
// hooks in this component rely on) so tests can drive an app-wide locale switch, mirroring
// `PatchPathInput.test.tsx`'s pattern for the same completion command.
vi.mock("../../../../i18n/LocaleProvider", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../../i18n/LocaleProvider")>();
  return { ...actual, useLocale: vi.fn() };
});

const invokeMock = vi.mocked(invoke);
const mockUseLocale = vi.mocked(useLocale);

const catalog: SchemaCatalog = {
  formatVersion: 1,
  packs: [],
  defTypes: {
    Def: {
      inherits: [],
      abstractType: true,
      fieldOrder: ["modExtensions"],
      fields: {
        modExtensions: {
          type: { kind: "list" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "listOfLi",
          flags: false,
        },
      },
    },
    ThingDef: {
      inherits: ["Def"],
      abstractType: false,
      fieldOrder: ["label", "comps"],
      fields: {
        label: {
          type: { kind: "string" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "element",
          flags: false,
        },
        comps: {
          type: { kind: "list" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "listOfLi",
          flags: false,
          items: { kind: "object", schemaRef: "CompProperties" },
        },
      },
    },
  },
  objectTypes: {
    CompProperties: {
      fieldOrder: [],
      fields: {},
      discriminator: {
        attribute: "Class",
        allowMissing: false,
        allowUnknown: true,
        variants: { CompProperties_Foo: "CompProperties_Foo" },
      },
    },
    CompProperties_Foo: {
      inherits: ["CompProperties"],
      fieldOrder: ["hitPoints"],
      fields: {
        hitPoints: {
          type: { kind: "integer" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "element",
          flags: false,
        },
      },
    },
  },
};

function completionResult(overrides: Partial<XPathCompletionResult> = {}): XPathCompletionResult {
  return {
    replaceFrom: 0,
    items: [],
    diagnostics: [],
    target: { kind: "unsupported" },
    resolvedField: null,
    ...overrides,
  };
}

function mockInvoke(handlers: {
  xpath?: XPathCompletionResult;
  parse?: XmlChildView[];
  serialize?: string;
}) {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "complete_patch_operation_xpath") return Promise.resolve(handlers.xpath ?? completionResult());
    if (cmd === "parse_patch_value_xml") return Promise.resolve(handlers.parse ?? []);
    if (cmd === "serialize_patch_value_fragment") return Promise.resolve(handlers.serialize ?? "");
    return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  mockUseLocale.mockReturnValue({ locale: "en", direction: "ltr", changeLocale: vi.fn() });
});

describe("PatchValueEditor", () => {
  it("defaults to raw XML when the xpath has no resolvable target", async () => {
    mockInvoke({ xpath: completionResult({ target: { kind: "unsupported" } }) });

    render(
      <PatchValueEditor
        valueXml="<foo>bar</foo>"
        xpath="Defs/ThingDef/foo/bar/baz"
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("complete_patch_operation_xpath", expect.anything()));
    expect((screen.getByRole("button", { name: "Structured" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByDisplayValue("<foo>bar</foo>")).toBeTruthy();
    // XML is machine-readable syntax, not natural-language prose -- this must stay LTR even once
    // a future RTL locale flips `dir` on `<html>` (docs/i18n/issues/08-editor-and-patch-ui-
    // migration.md's "keep code editor/XML/XPath controls dir=ltr by semantic policy").
    expect(screen.getByDisplayValue("<foo>bar</foo>").getAttribute("dir")).toBe("ltr");
  });

  it("adds a scalar field payload in structured mode", async () => {
    mockInvoke({
      xpath: completionResult({
        target: { kind: "def", defType: "ThingDef", defName: "Wall" },
        resolvedField: { defType: "ThingDef", fieldName: "label", field: catalog.defTypes.ThingDef.fields.label },
      }),
      parse: [],
      serialize: "<label>Wall</label>\n",
    });

    const onChange = vi.fn();
    render(
      <PatchValueEditor
        valueXml={null}
        xpath='Defs/ThingDef[defName="Wall"]/label'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="add"
        label="Value"
        onChange={onChange}
      />,
    );

    // Add defaults to structured mode once a supported field resolves -- find the structured
    // scalar input by its field sub-label rather than by empty display value, since the raw
    // textarea (briefly rendered before the mode-default effect flips to "structured") is also
    // empty at that point.
    const subLabel = await screen.findByText("label");
    const input = subLabel.parentElement!.querySelector("input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Wall" } });

    await waitFor(() => expect(onChange).toHaveBeenLastCalledWith("<label>Wall</label>\n"));
    expect(invokeMock).toHaveBeenCalledWith("serialize_patch_value_fragment", {
      elements: [{ name: "label", value: "Wall" }],
    });
  });

  it("edits a replace scalar field payload parsed from existing XML", async () => {
    mockInvoke({
      xpath: completionResult({
        target: { kind: "def", defType: "ThingDef", defName: "Wall" },
        resolvedField: { defType: "ThingDef", fieldName: "label", field: catalog.defTypes.ThingDef.fields.label },
      }),
      parse: [
        {
          nodeId: 1,
          name: "label",
          textValue: "OldWall",
          listItems: [],
          xmlShape: "element",
          order: 0,
          known: true,
          line: null,
          column: null,
        },
      ],
      serialize: "<label>NewWall</label>\n",
    });

    const onChange = vi.fn();
    render(
      <PatchValueEditor
        valueXml="<label>OldWall</label>"
        xpath='Defs/ThingDef[defName="Wall"]/label'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="replace"
        label="Value"
        onChange={onChange}
      />,
    );

    const input = await screen.findByDisplayValue("OldWall");
    fireEvent.change(input, { target: { value: "NewWall" } });

    await waitFor(() => expect(onChange).toHaveBeenLastCalledWith("<label>NewWall</label>\n"));
  });

  it("adds a list item payload", async () => {
    mockInvoke({
      xpath: completionResult({
        target: { kind: "def", defType: "ThingDef", defName: "Wall" },
        resolvedField: {
          defType: "ThingDef",
          fieldName: "comps",
          field: { ...catalog.defTypes.ThingDef.fields.comps, items: undefined },
        },
      }),
      parse: [],
      serialize: "<comps>\n  <li>Foo</li>\n</comps>\n",
    });

    const onChange = vi.fn();
    render(
      <PatchValueEditor
        valueXml={null}
        xpath='Defs/ThingDef[defName="Wall"]/comps'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="add"
        label="Value"
        onChange={onChange}
      />,
    );

    const addButton = await screen.findByRole("button", { name: /Add item/ });
    fireEvent.click(addButton);
    const input = await screen.findByDisplayValue("");
    fireEvent.change(input, { target: { value: "Foo" } });

    await waitFor(() => expect(onChange).toHaveBeenLastCalledWith("<comps>\n  <li>Foo</li>\n</comps>\n"));
  });

  it("adds an object list item payload with a Class discriminator", async () => {
    mockInvoke({
      xpath: completionResult({
        target: { kind: "def", defType: "ThingDef", defName: "Wall" },
        resolvedField: { defType: "ThingDef", fieldName: "comps", field: catalog.defTypes.ThingDef.fields.comps },
      }),
      parse: [],
      serialize: '<comps>\n  <li Class="CompProperties_Foo" />\n</comps>\n',
    });

    const onChange = vi.fn();
    render(
      <PatchValueEditor
        valueXml={null}
        xpath='Defs/ThingDef[defName="Wall"]/comps'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="add"
        label="Value"
        onChange={onChange}
      />,
    );

    const addButton = await screen.findByRole("button", { name: /Add item/ });
    fireEvent.click(addButton);

    await waitFor(() =>
      expect(onChange).toHaveBeenLastCalledWith('<comps>\n  <li Class="CompProperties_Foo" />\n</comps>\n'),
    );
    expect(invokeMock).toHaveBeenCalledWith(
      "serialize_patch_value_fragment",
      expect.objectContaining({
        elements: [
          expect.objectContaining({
            name: "comps",
            liItems: [expect.objectContaining({ name: "li", attributes: [{ name: "Class", value: "CompProperties_Foo" }] })],
          }),
        ],
      }),
    );
  });

  it("falls back to raw XML when the existing value's root element doesn't match the target field", async () => {
    mockInvoke({
      xpath: completionResult({
        target: { kind: "def", defType: "ThingDef", defName: "Wall" },
        resolvedField: { defType: "ThingDef", fieldName: "label", field: catalog.defTypes.ThingDef.fields.label },
      }),
      parse: [
        {
          nodeId: 1,
          name: "comps",
          textValue: null,
          listItems: [],
          xmlShape: "listOfLi",
          order: 0,
          known: true,
          line: null,
          column: null,
        },
      ],
    });

    render(
      <PatchValueEditor
        valueXml="<comps></comps>"
        xpath='Defs/ThingDef[defName="Wall"]/label'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await screen.findByText(/doesn't match the target field/);
    expect((screen.getByDisplayValue("<comps></comps>") as HTMLTextAreaElement).disabled).toBe(false);
  });

  it("dedents raw XML captured with the source file's original indentation for display", async () => {
    mockInvoke({ xpath: completionResult({ target: { kind: "unsupported" } }) });

    const rawFromSource =
      '\n                        <li Class="aRandomKiwi.PPP.CompProperties_LocalWirelessPowerReceptor">\n' +
      "                            <compClass>aRandomKiwi.PPP.CompLocalWirelessPowerReceptor</compClass>\n" +
      "                        </li>\n                    ";
    const onChange = vi.fn();

    const { container } = render(
      <PatchValueEditor
        valueXml={rawFromSource}
        xpath="Defs/ThingDef/comps"
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="replace"
        label="Value"
        onChange={onChange}
      />,
    );

    const dedented =
      '<li Class="aRandomKiwi.PPP.CompProperties_LocalWirelessPowerReceptor">\n' +
      "    <compClass>aRandomKiwi.PPP.CompLocalWirelessPowerReceptor</compClass>\n" +
      "</li>";
    const textarea = container.querySelector("textarea") as HTMLTextAreaElement;
    await waitFor(() => expect(textarea.value).toBe(dedented));

    // Editing sends the edited (cleanly-indented) text upstream -- not a re-indented copy of the
    // original source formatting.
    const edited = dedented.replace("CompLocalWirelessPowerReceptor", "CompLocalWirelessPowerReceptorV2");
    fireEvent.change(textarea, { target: { value: edited } });
    expect(onChange).toHaveBeenLastCalledWith(edited);
  });

  it("lets the user toggle back to raw XML manually", async () => {
    mockInvoke({
      xpath: completionResult({
        target: { kind: "def", defType: "ThingDef", defName: "Wall" },
        resolvedField: { defType: "ThingDef", fieldName: "label", field: catalog.defTypes.ThingDef.fields.label },
      }),
      parse: [
        {
          nodeId: 1,
          name: "label",
          textValue: "Wall",
          listItems: [],
          xmlShape: "element",
          order: 0,
          known: true,
          line: null,
          column: null,
        },
      ],
    });

    render(
      <PatchValueEditor
        valueXml="<label>Wall</label>"
        xpath='Defs/ThingDef[defName="Wall"]/label'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await screen.findByDisplayValue("Wall");
    fireEvent.click(screen.getByRole("button", { name: "Raw XML" }));
    expect(screen.getByDisplayValue("<label>Wall</label>")).toBeTruthy();
  });

  it("does not offer structured mode for AddModExtension when modExtensions has no object item shape", async () => {
    // The fixture's modExtensions field (like the real built-in schema pack's) is a plain
    // scalar listOfLi with no items.schemaRef -- a scalar list editor would let the user create
    // invalid `<li>text</li>` entries instead of the `<li Class="...">...` RimWorld requires.
    mockInvoke({
      xpath: completionResult({ target: { kind: "def", defType: "ThingDef", defName: "Wall" } }),
    });

    render(
      <PatchValueEditor
        valueXml={null}
        xpath='Defs/ThingDef[defName="Wall"]'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="addModExtension"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("complete_patch_operation_xpath", expect.anything()));
    expect((screen.getByRole("button", { name: "Structured" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("re-parses when valueXml changes externally (e.g. undo) while structured mode stays active", async () => {
    mockInvoke({
      xpath: completionResult({
        target: { kind: "def", defType: "ThingDef", defName: "Wall" },
        resolvedField: { defType: "ThingDef", fieldName: "label", field: catalog.defTypes.ThingDef.fields.label },
      }),
      parse: [
        {
          nodeId: 1,
          name: "label",
          textValue: "Wall",
          listItems: [],
          xmlShape: "element",
          order: 0,
          known: true,
          line: null,
          column: null,
        },
      ],
    });

    const { rerender } = render(
      <PatchValueEditor
        valueXml="<label>Wall</label>"
        xpath='Defs/ThingDef[defName="Wall"]/label'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await screen.findByDisplayValue("Wall");

    // Simulate an external change to valueXml (undo/redo, or a hand-edit made while briefly in
    // raw mode) landing on this same mounted instance without ever going through
    // updateStructuredValue -- the structured field must pick up the new content, not keep
    // showing the stale "Wall" value.
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "complete_patch_operation_xpath") {
        return Promise.resolve(
          completionResult({
            target: { kind: "def", defType: "ThingDef", defName: "Wall" },
            resolvedField: { defType: "ThingDef", fieldName: "label", field: catalog.defTypes.ThingDef.fields.label },
          }),
        );
      }
      if (cmd === "parse_patch_value_xml") {
        return Promise.resolve([
          {
            nodeId: 1,
            name: "label",
            textValue: "UndoneWall",
            listItems: [],
            xmlShape: "element",
            order: 0,
            known: true,
            line: null,
            column: null,
          },
        ]);
      }
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });

    rerender(
      <PatchValueEditor
        valueXml="<label>UndoneWall</label>"
        xpath='Defs/ThingDef[defName="Wall"]/label'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await screen.findByDisplayValue("UndoneWall");
    expect(screen.queryByDisplayValue("Wall")).toBeNull();
  });

  it("passes the active locale to xpath completion and refetches when the locale changes", async () => {
    mockInvoke({
      xpath: completionResult({
        target: { kind: "def", defType: "ThingDef", defName: "Wall" },
        resolvedField: { defType: "ThingDef", fieldName: "label", field: catalog.defTypes.ThingDef.fields.label },
      }),
    });

    const { rerender } = render(
      <PatchValueEditor
        valueXml={null}
        xpath='Defs/ThingDef[defName="Wall"]/label'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("complete_patch_operation_xpath", {
        projectId: "proj1",
        xpath: 'Defs/ThingDef[defName="Wall"]/label',
        locale: "en",
      }),
    );

    invokeMock.mockClear();
    mockUseLocale.mockReturnValue({ locale: "fr", direction: "ltr", changeLocale: vi.fn() });

    rerender(
      <PatchValueEditor
        valueXml={null}
        xpath='Defs/ThingDef[defName="Wall"]/label'
        readOnly={false}
        catalog={catalog}
        projectId="proj1"
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("complete_patch_operation_xpath", {
        projectId: "proj1",
        xpath: 'Defs/ThingDef[defName="Wall"]/label',
        locale: "fr",
      }),
    );
  });
});
