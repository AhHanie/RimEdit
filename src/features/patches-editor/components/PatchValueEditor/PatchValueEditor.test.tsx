import { fireEvent, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { PatchValueEditor } from "./PatchValueEditor";
import type { SchemaCatalog } from "../../../schema-catalog";
import type { XPathResolvedField, XPathTarget } from "../../types/xpathCompletion";
import type { XmlChildView } from "../../../xml-editor";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

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

function mockInvoke(handlers: { parse?: XmlChildView[]; serialize?: string }) {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "parse_patch_value_xml") return Promise.resolve(handlers.parse ?? []);
    if (cmd === "serialize_patch_value_fragment") return Promise.resolve(handlers.serialize ?? "");
    return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
  });
}

const unsupportedTarget: XPathTarget = { kind: "unsupported" };
const wallTarget: XPathTarget = { kind: "def", defType: "ThingDef", defName: "Wall" };

function resolvedField(fieldName: string, field: SchemaCatalog["defTypes"]["ThingDef"]["fields"][string]): XPathResolvedField {
  return { defType: "ThingDef", fieldName, field };
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("PatchValueEditor", () => {
  it("defaults to raw XML when the xpath has no resolvable target", () => {
    mockInvoke({});

    render(
      <PatchValueEditor
        valueXml="<foo>bar</foo>"
        readOnly={false}
        catalog={catalog}
        target={unsupportedTarget}
        resolvedField={null}
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    expect((screen.getByRole("button", { name: "Structured" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByDisplayValue("<foo>bar</foo>")).toBeTruthy();
    // XML is machine-readable syntax, not natural-language prose -- this must stay LTR even once
    // a future RTL locale flips `dir` on `<html>` (docs/i18n/issues/08-editor-and-patch-ui-
    // migration.md's "keep code editor/XML/XPath controls dir=ltr by semantic policy").
    expect(screen.getByDisplayValue("<foo>bar</foo>").getAttribute("dir")).toBe("ltr");
  });

  it("adds a scalar field payload in structured mode", async () => {
    mockInvoke({ parse: [], serialize: "<label>Wall</label>\n" });

    const onChange = vi.fn();
    render(
      <PatchValueEditor
        valueXml={null}
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        resolvedField={resolvedField("label", catalog.defTypes.ThingDef.fields.label)}
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

  it("uses a nested resolvedField's own terminal field, not its root container", async () => {
    // The backend resolves a multi-level path (e.g. `graphicData/texPath`) to the *terminal*
    // field's own name/schema, not the top-level container it's nested under (see
    // `patches::xpath`'s unlimited-depth schema cursor). PatchValueEditor must trust that
    // resolution as-is: it should build the structured subform -- and the serialized XML element
    // -- around "texPath", never "graphicData".
    mockInvoke({ parse: [], serialize: "<texPath>Things/Wall</texPath>\n" });

    const onChange = vi.fn();
    render(
      <PatchValueEditor
        valueXml={null}
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        // Reuses the fixture's plain scalar `label` field schema under a different fieldName --
        // its shape (scalar string, xml: element) is all that matters here, not its origin.
        resolvedField={resolvedField("texPath", catalog.defTypes.ThingDef.fields.label)}
        operationType="add"
        label="Value"
        onChange={onChange}
      />,
    );

    const subLabel = await screen.findByText("texPath");
    expect(screen.queryByText("graphicData")).toBeNull();
    const input = subLabel.parentElement!.querySelector("input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Things/Wall" } });

    await waitFor(() => expect(onChange).toHaveBeenLastCalledWith("<texPath>Things/Wall</texPath>\n"));
    expect(invokeMock).toHaveBeenCalledWith("serialize_patch_value_fragment", {
      elements: [{ name: "texPath", value: "Things/Wall" }],
    });
  });

  it("edits a replace scalar field payload parsed from existing XML", async () => {
    mockInvoke({
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
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        resolvedField={resolvedField("label", catalog.defTypes.ThingDef.fields.label)}
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
    mockInvoke({ parse: [], serialize: "<comps>\n  <li>Foo</li>\n</comps>\n" });

    const onChange = vi.fn();
    render(
      <PatchValueEditor
        valueXml={null}
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        resolvedField={resolvedField("comps", { ...catalog.defTypes.ThingDef.fields.comps, items: undefined })}
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
    mockInvoke({ parse: [], serialize: '<comps>\n  <li Class="CompProperties_Foo" />\n</comps>\n' });

    const onChange = vi.fn();
    render(
      <PatchValueEditor
        valueXml={null}
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        resolvedField={resolvedField("comps", catalog.defTypes.ThingDef.fields.comps)}
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
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        resolvedField={resolvedField("label", catalog.defTypes.ThingDef.fields.label)}
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await screen.findByText(/doesn't match the target field/);
    expect((screen.getByDisplayValue("<comps></comps>") as HTMLTextAreaElement).disabled).toBe(false);
  });

  it("dedents raw XML captured with the source file's original indentation for display", async () => {
    mockInvoke({});

    const rawFromSource =
      '\n                        <li Class="aRandomKiwi.PPP.CompProperties_LocalWirelessPowerReceptor">\n' +
      "                            <compClass>aRandomKiwi.PPP.CompLocalWirelessPowerReceptor</compClass>\n" +
      "                        </li>\n                    ";
    const onChange = vi.fn();

    const { container } = render(
      <PatchValueEditor
        valueXml={rawFromSource}
        readOnly={false}
        catalog={catalog}
        target={unsupportedTarget}
        resolvedField={null}
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
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        resolvedField={resolvedField("label", catalog.defTypes.ThingDef.fields.label)}
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
    mockInvoke({});

    render(
      <PatchValueEditor
        valueXml={null}
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        resolvedField={null}
        operationType="addModExtension"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    expect((screen.getByRole("button", { name: "Structured" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("re-parses when valueXml changes externally (e.g. undo) while structured mode stays active", async () => {
    mockInvoke({
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
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        resolvedField={resolvedField("label", catalog.defTypes.ThingDef.fields.label)}
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
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        resolvedField={resolvedField("label", catalog.defTypes.ThingDef.fields.label)}
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await screen.findByDisplayValue("UndoneWall");
    expect(screen.queryByDisplayValue("Wall")).toBeNull();
  });

  it("clears the structured target and falls back to raw XML once the shared xpath result becomes unavailable", async () => {
    mockInvoke({
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
        readOnly={false}
        catalog={catalog}
        target={wallTarget}
        resolvedField={resolvedField("label", catalog.defTypes.ThingDef.fields.label)}
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await screen.findByDisplayValue("Wall");

    // The shared completion result becomes unavailable (e.g. the xpath itself was cleared, or the
    // project context disappeared) -- PatchOperationForm passes `null`/`null` down in that case.
    rerender(
      <PatchValueEditor
        valueXml="<label>Wall</label>"
        readOnly={false}
        catalog={catalog}
        target={null}
        resolvedField={null}
        operationType="replace"
        label="Value"
        onChange={vi.fn()}
      />,
    );

    await waitFor(() =>
      expect((screen.getByRole("button", { name: "Structured" }) as HTMLButtonElement).disabled).toBe(true),
    );
    expect(screen.getByDisplayValue("<label>Wall</label>")).toBeTruthy();
  });
});
