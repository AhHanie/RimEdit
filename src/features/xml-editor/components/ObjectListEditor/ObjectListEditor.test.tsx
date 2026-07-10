import { fireEvent, render, screen } from "@testing-library/react";
import { ObjectListEditor } from "./ObjectListEditor";
import { XmlEditorContextProvider } from "../../context/XmlEditorContext";
import type { SchemaCatalog } from "../../../schema-catalog";
import type {
  FormFieldState,
  ObjectListItemValue,
} from "../../types/editorForm";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

// --- Fixtures ---

function makeColorForStuffCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {},
    objectTypes: {
      ColorForStuff: {
        fieldOrder: ["stuff", "color"],
        fields: {
          stuff: {
            label: "Stuff",
            type: { kind: "defReference" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
            reference: {
              defType: "ThingDef",
              allowAbstract: false,
              scope: "allSources",
            },
          },
          color: {
            label: "Color",
            type: { kind: "string" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
          },
        },
      },
    },
  };
}

function makeColorPerStuffField(
  items: ObjectListItemValue[] = [],
): FormFieldState {
  return {
    model: {
      id: "ThingDef:Test:objectList:colorPerStuff",
      key: "colorPerStuff",
      label: "Color Per Stuff",
      control: "objectList",
      path: { kind: "objectList", objectPath: [], fieldName: "colorPerStuff" },
      fieldPath: ["colorPerStuff"],
      sourceNodeId: null,
      defNodeId: 1,
      order: 0,
      readonly: false,
      required: false,
      repeatable: false,
      xmlShape: "listOfLi",
      examples: [],
      diagnostics: [],
      sectionDefaults: [],
      itemSchemaRef: "ColorForStuff",
    },
    value: { kind: "objectList", items },
    initialValue: { kind: "objectList", items: [] },
    dirty: false,
    touched: false,
    focused: false,
    pending: false,
    error: null,
    validationErrors: [],
    clearRequested: false,
  };
}

const mockFormApi = {
  setFieldValue: vi.fn(),
  focusField: vi.fn(),
  blurField: vi.fn(),
};

function renderEditor(
  catalog: SchemaCatalog,
  field: FormFieldState,
  projectId?: string,
) {
  return render(
    <XmlEditorContextProvider
      value={{ projectId, readOnly: false, catalog, onNavigateDef: vi.fn() }}
    >
      <ObjectListEditor field={field} formApi={mockFormApi as never} />
    </XmlEditorContextProvider>,
  );
}

function renderReadOnlyEditor(
  catalog: SchemaCatalog,
  field: FormFieldState,
  projectId?: string,
) {
  return render(
    <XmlEditorContextProvider
      value={{ projectId, readOnly: true, catalog, onNavigateDef: vi.fn() }}
    >
      <ObjectListEditor field={field} formApi={mockFormApi as never} readOnly />
    </XmlEditorContextProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

// --- Plain schema (no discriminator) ---

describe("ObjectListEditor – plain schema (no discriminator)", () => {
  it("shows 'Add item' instead of comp-class UI for plain schemas", () => {
    renderEditor(makeColorForStuffCatalog(), makeColorPerStuffField());

    // getByRole throws if not found, so this IS the positive assertion
    screen.getByRole("button", { name: /add item/i });
    expect(screen.queryByText(/Add known comp/)).toBeNull();
    expect(screen.queryByText(/Add custom comp/)).toBeNull();
  });

  it("renders field labels for a ColorForStuff item", () => {
    const item: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "ColorForStuff",
      fields: {
        stuff: { kind: "scalar", value: "WoodLog" },
        color: { kind: "scalar", value: "(1, 0.5, 0.5, 1)" },
      },
      initialUnknownFieldCount: 0,
    };
    renderEditor(makeColorForStuffCatalog(), makeColorPerStuffField([item]));

    screen.getByText("Stuff");
    screen.getByText("Color");
  });

  it("does not render a Class input for plain schema items", () => {
    const item: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "ColorForStuff",
      fields: {},
      initialUnknownFieldCount: 0,
    };
    renderEditor(makeColorForStuffCatalog(), makeColorPerStuffField([item]));

    expect(screen.queryByPlaceholderText("CompProperties_…")).toBeNull();
  });

  it("labels plain schema items by 1-based index", () => {
    const items: ObjectListItemValue[] = [
      {
        nodeId: 10,
        className: "",
        schemaRef: "ColorForStuff",
        fields: {},
        initialUnknownFieldCount: 0,
      },
      {
        nodeId: 11,
        className: "",
        schemaRef: "ColorForStuff",
        fields: {},
        initialUnknownFieldCount: 0,
      },
    ];
    renderEditor(makeColorForStuffCatalog(), makeColorPerStuffField(items));

    screen.getByText("Item 1");
    screen.getByText("Item 2");
  });

  it("adds a new item with the base schemaRef when Add item is clicked", () => {
    const field = makeColorPerStuffField();
    renderEditor(makeColorForStuffCatalog(), field);

    fireEvent.click(screen.getByRole("button", { name: /add item/i }));

    expect(mockFormApi.setFieldValue).toHaveBeenCalledWith(
      field.model.id,
      expect.objectContaining({
        kind: "objectList",
        items: expect.arrayContaining([
          expect.objectContaining({
            className: "",
            schemaRef: "ColorForStuff",
          }),
        ]),
      }),
    );
  });
});

// --- §1.1 extended behavior: nested object, decorations, dirty, readonly, collapse ---

describe("ObjectListEditor – nested and extended behavior", () => {
  function makeExtendedCatalog(): SchemaCatalog {
    return {
      formatVersion: 1,
      packs: [],
      defTypes: {},
      objectTypes: {
        SubSoundDef: {
          fieldOrder: ["name", "grains", "params"],
          fields: {
            name: {
              label: "Name",
              type: { kind: "string" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
            grains: {
              label: "Grains",
              type: { kind: "list" },
              required: false,
              repeatable: false,
              xml: "listOfLi",
              examples: [],
              flags: false,
              items: { kind: "object", schemaRef: "AudioGrain_Clip" },
            },
            params: {
              label: "Sound Params",
              type: { kind: "object", schemaRef: "SoundParams" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
              defaultCollapsed: false,
            },
          },
        },
        AudioGrain_Clip: {
          fieldOrder: ["clipPath"],
          fields: {
            clipPath: {
              label: "Clip Path",
              type: { kind: "string" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
          },
        },
        SoundParams: {
          fieldOrder: ["volume"],
          fields: {
            volume: {
              label: "Volume",
              type: { kind: "string" },
              required: true,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
              description: "Sets the volume of this sound.",
            },
          },
        },
      },
    };
  }

  function makeSubSoundField(
    items: ObjectListItemValue[] = [],
  ): FormFieldState {
    return {
      model: {
        id: "ThingDef:Test:objectList:subSounds",
        key: "subSounds",
        label: "Sub sounds",
        control: "objectList",
        path: { kind: "objectList", objectPath: [], fieldName: "subSounds" },
        fieldPath: ["subSounds"],
        sourceNodeId: null,
        defNodeId: 1,
        order: 0,
        readonly: false,
        required: false,
        repeatable: false,
        xmlShape: "listOfLi",
        examples: [],
        diagnostics: [],
        sectionDefaults: [],
        itemSchemaRef: "SubSoundDef",
      },
      value: { kind: "objectList", items },
      initialValue: { kind: "objectList", items: [] },
      dirty: false,
      touched: false,
      focused: false,
      pending: false,
      error: null,
      validationErrors: [],
      clearRequested: false,
    };
  }

  // §1.1.1: Nested objectList renders inside an item
  it("§1.1.1 renders nested objectList panel label and its item schema fields", () => {
    const grain: ObjectListItemValue = {
      nodeId: null,
      className: "",
      schemaRef: "AudioGrain_Clip",
      fields: { clipPath: { kind: "scalar", value: "gunshot/bang.ogg" } },
      initialUnknownFieldCount: 0,
    };
    const outerItem: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: {
        grains: {
          kind: "objectList",
          itemSchemaRef: "AudioGrain_Clip",
          items: [grain],
        },
      },
      initialUnknownFieldCount: 0,
    };
    renderEditor(makeExtendedCatalog(), makeSubSoundField([outerItem]));

    // Nested section label
    screen.getByText("Grains");
    // AudioGrain_Clip field label visible inside the nested panel
    screen.getByText("Clip Path");
  });

  // §1.1.2: ObjectFieldControl decorations - description paragraph and required asterisk
  it("§1.1.2 renders description paragraph and required asterisk for annotated fields", () => {
    const item: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: {
        params: {
          kind: "object",
          schemaRef: "SoundParams",
          fields: {},
          nodeId: null,
          initialUnknownFieldCount: 0,
          fieldOrder: ["volume"],
        },
      },
      initialUnknownFieldCount: 0,
    };
    renderEditor(makeExtendedCatalog(), makeSubSoundField([item]));

    // Description paragraph from SoundParams.volume field schema
    screen.getByText("Sets the volume of this sound.");
    // Required asterisk rendered by ObjectFieldControl
    screen.getByText("*");
  });

  it("§1.1.2b does not render a description paragraph when field schema has no description", () => {
    // ColorForStuff fields have no description - use it to isolate the no-description case
    const item: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "ColorForStuff",
      fields: {
        stuff: { kind: "scalar", value: "Steel" },
        color: { kind: "scalar", value: "(1,1,1,1)" },
      },
      initialUnknownFieldCount: 0,
    };
    renderEditor(makeColorForStuffCatalog(), makeColorPerStuffField([item]));

    screen.getByText("Stuff"); // label is present
    screen.getByText("Color"); // label is present
    // No description on any field → no <p class="description"> element
    expect(document.querySelector("p")).toBeNull();
  });

  // §1.1.3: Dirty tracking shows reset button after a field changes from its initial value
  it("§1.1.3 shows reset button when a field changes from its initial value", () => {
    const initial: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: { name: { kind: "scalar", value: "Orig" } },
      initialUnknownFieldCount: 0,
    };
    // First render: ObjectListEditor snapshots initialItems = [initial]
    const { rerender } = renderEditor(
      makeExtendedCatalog(),
      makeSubSoundField([initial]),
    );

    // Rerender with a changed name - ObjectListEditor's initialItems snapshot stays [initial]
    const updated: ObjectListItemValue = {
      ...initial,
      fields: { name: { kind: "scalar", value: "Changed" } },
    };
    rerender(
      <XmlEditorContextProvider
        value={{
          projectId: undefined,
          readOnly: false,
          catalog: makeExtendedCatalog(),
          onNavigateDef: vi.fn(),
        }}
      >
        <ObjectListEditor
          field={makeSubSoundField([updated])}
          formApi={mockFormApi as never}
        />
      </XmlEditorContextProvider>,
    );

    // Reset button appears for the dirty Name field
    screen.getByRole("button", { name: "Reset Name" });

    // Clicking reset restores the original value
    fireEvent.click(screen.getByRole("button", { name: "Reset Name" }));
    expect(mockFormApi.setFieldValue).toHaveBeenCalledWith(
      "ThingDef:Test:objectList:subSounds",
      expect.objectContaining({
        kind: "objectList",
        items: expect.arrayContaining([
          expect.objectContaining({
            fields: expect.objectContaining({
              name: { kind: "scalar", value: "Orig" },
            }),
          }),
        ]),
      }),
    );
  });

  it("§1.1.3b new items (nodeId=null) do not show a reset button", () => {
    const newItem: ObjectListItemValue = {
      nodeId: null,
      className: "",
      schemaRef: "SubSoundDef",
      fields: { name: { kind: "scalar", value: "Brand New" } },
      initialUnknownFieldCount: 0,
    };
    renderEditor(makeExtendedCatalog(), makeSubSoundField([newItem]));

    // New item has no initial snapshot entry → not dirty → no reset button
    expect(screen.queryByRole("button", { name: /reset/i })).toBeNull();
  });

  // §1.1.4: Readonly field renders reason text as non-editable span
  it("§1.1.4 renders readonly reason text as a non-editable span", () => {
    const item: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: {
        grains: { kind: "readonly", reason: "Circular schema reference." },
      },
      initialUnknownFieldCount: 0,
    };
    renderEditor(makeExtendedCatalog(), makeSubSoundField([item]));

    screen.getByText("Circular schema reference.");
    // No textarea for this field
    expect(screen.queryByPlaceholderText("One item per line…")).toBeNull();
  });

  // §1.1.5: NestedObjectSection starts expanded when fieldSchema.defaultCollapsed = false
  it("§1.1.5 starts expanded when fieldSchema.defaultCollapsed = false", () => {
    const item: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: {},
      initialUnknownFieldCount: 0,
    };
    renderEditor(makeExtendedCatalog(), makeSubSoundField([item]));

    // The params section (defaultCollapsed=false) starts expanded
    const btn = screen.getByRole("button", { name: /sound params/i });
    expect(btn.getAttribute("aria-expanded")).toBe("true");
  });
});

// --- defReference field in CompFieldInput ---

describe("ObjectListEditor – defReference field renders ReferencePicker", () => {
  const itemWithStuff: ObjectListItemValue = {
    nodeId: 10,
    className: "",
    schemaRef: "ColorForStuff",
    fields: {
      stuff: { kind: "scalar", value: "WoodLog" },
      color: { kind: "scalar", value: "" },
    },
    initialUnknownFieldCount: 0,
  };

  it("renders a ReferencePicker navigation button for defReference when projectId is set", () => {
    renderEditor(
      makeColorForStuffCatalog(),
      makeColorPerStuffField([itemWithStuff]),
      "test-project",
    );

    // ReferencePicker renders a button with aria-label "Go to <defType>"
    screen.getByRole("button", { name: "Go to ThingDef" });
  });

  it("falls back to plain text input for defReference when projectId is absent", () => {
    renderEditor(
      makeColorForStuffCatalog(),
      makeColorPerStuffField([itemWithStuff]),
      // no projectId
    );

    // Without projectId no ReferencePicker → no "Go to" navigation button
    expect(screen.queryByRole("button", { name: /go to/i })).toBeNull();
  });
});

// --- readOnly mode ---

describe("ObjectListEditor – readOnly mode", () => {
  function makeToolLikeCatalog(): SchemaCatalog {
    return {
      formatVersion: 1,
      packs: [],
      defTypes: {},
      objectTypes: {
        Tool: {
          fieldOrder: ["label", "power", "extraMeleeDamages"],
          fields: {
            label: {
              label: "Label",
              type: { kind: "string" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
            power: {
              label: "Power",
              type: { kind: "float" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
            extraMeleeDamages: {
              label: "Extra Melee Damages",
              type: { kind: "list" },
              required: false,
              repeatable: false,
              xml: "listOfLi",
              examples: [],
              flags: false,
              items: { kind: "object", schemaRef: "ExtraDamage" },
            },
          },
        },
        ExtraDamage: {
          fieldOrder: ["amount"],
          fields: {
            amount: {
              label: "Amount",
              type: { kind: "integer" },
              required: false,
              repeatable: false,
              xml: "element",
              examples: [],
              flags: false,
            },
          },
        },
      },
    };
  }

  function makeToolsField(items: ObjectListItemValue[] = []): FormFieldState {
    return {
      model: {
        id: "ThingDef:Test:objectList:tools",
        key: "tools",
        label: "Tools",
        control: "objectList",
        path: { kind: "objectList", objectPath: [], fieldName: "tools" },
        fieldPath: ["tools"],
        sourceNodeId: null,
        defNodeId: 1,
        order: 0,
        readonly: false,
        required: false,
        repeatable: false,
        xmlShape: "listOfLi",
        examples: [],
        diagnostics: [],
        sectionDefaults: [],
        itemSchemaRef: "Tool",
      },
      value: { kind: "objectList", items },
      initialValue: { kind: "objectList", items: [] },
      dirty: false,
      touched: false,
      focused: false,
      pending: false,
      error: null,
      validationErrors: [],
      clearRequested: false,
    };
  }

  const toolItem: ObjectListItemValue = {
    nodeId: 10,
    className: "",
    schemaRef: "Tool",
    fields: {
      label: { kind: "scalar", value: "stock" },
      power: { kind: "scalar", value: "9" },
    },
    initialUnknownFieldCount: 0,
  };

  it("hides the Add item button", () => {
    renderReadOnlyEditor(makeToolLikeCatalog(), makeToolsField([toolItem]));

    expect(screen.queryByRole("button", { name: /add item/i })).toBeNull();
  });

  it("hides the per-item remove button", () => {
    renderReadOnlyEditor(makeToolLikeCatalog(), makeToolsField([toolItem]));

    expect(screen.queryByRole("button", { name: /remove/i })).toBeNull();
  });

  it("disables scalar field inputs", () => {
    renderReadOnlyEditor(makeToolLikeCatalog(), makeToolsField([toolItem]));

    const labelInput = screen.getByDisplayValue("stock") as HTMLInputElement;
    const powerInput = screen.getByDisplayValue("9") as HTMLInputElement;
    expect(labelInput.disabled).toBe(true);
    expect(powerInput.disabled).toBe(true);
  });

  it("does not call formApi.setFieldValue when a disabled input is edited", () => {
    renderReadOnlyEditor(makeToolLikeCatalog(), makeToolsField([toolItem]));

    const labelInput = screen.getByDisplayValue("stock") as HTMLInputElement;
    fireEvent.change(labelInput, { target: { value: "changed" } });

    expect(mockFormApi.setFieldValue).not.toHaveBeenCalled();
  });

  it("still allows expanding and collapsing an item", () => {
    renderReadOnlyEditor(makeToolLikeCatalog(), makeToolsField([toolItem]));

    const collapseBtn = screen.getByRole("button", { name: /collapse/i });
    fireEvent.click(collapseBtn);
    expect(screen.queryByDisplayValue("stock")).toBeNull();

    const expandBtn = screen.getByRole("button", { name: /expand/i });
    fireEvent.click(expandBtn);
    screen.getByDisplayValue("stock");
  });

  it("propagates readOnly into nested object lists: no nested Add button, inputs disabled", () => {
    const nestedDamage: ObjectListItemValue = {
      nodeId: 20,
      className: "",
      schemaRef: "ExtraDamage",
      fields: { amount: { kind: "scalar", value: "5" } },
      initialUnknownFieldCount: 0,
    };
    const itemWithNested: ObjectListItemValue = {
      ...toolItem,
      fields: {
        ...toolItem.fields,
        extraMeleeDamages: {
          kind: "objectList",
          itemSchemaRef: "ExtraDamage",
          items: [nestedDamage],
        },
      },
    };
    renderReadOnlyEditor(
      makeToolLikeCatalog(),
      makeToolsField([itemWithNested]),
    );

    // Nested item data is inspectable...
    const amountInput = screen.getByDisplayValue("5") as HTMLInputElement;
    // ...but disabled and without add/remove controls.
    expect(amountInput.disabled).toBe(true);
    expect(screen.queryByRole("button", { name: /add item/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /remove/i })).toBeNull();
  });

  it("prefers the label field over 'Item n' for non-discriminator schema items", () => {
    renderReadOnlyEditor(makeToolLikeCatalog(), makeToolsField([toolItem]));

    screen.getByText("stock");
    expect(screen.queryByText("Item 1")).toBeNull();
  });
});

// --- editable regression: editing calls formApi.setFieldValue ---

describe("ObjectListEditor – editable mode (regression)", () => {
  it("calls formApi.setFieldValue when a scalar field is edited", () => {
    const item: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "ColorForStuff",
      fields: {
        stuff: { kind: "scalar", value: "WoodLog" },
        color: { kind: "scalar", value: "(1,1,1,1)" },
      },
      initialUnknownFieldCount: 0,
    };
    renderEditor(makeColorForStuffCatalog(), makeColorPerStuffField([item]));

    const colorInput = screen.getByDisplayValue(
      "(1,1,1,1)",
    ) as HTMLInputElement;
    fireEvent.change(colorInput, { target: { value: "(0,0,0,1)" } });

    expect(mockFormApi.setFieldValue).toHaveBeenCalledWith(
      "ThingDef:Test:objectList:colorPerStuff",
      expect.objectContaining({
        kind: "objectList",
        items: expect.arrayContaining([
          expect.objectContaining({
            fields: expect.objectContaining({
              color: { kind: "scalar", value: "(0,0,0,1)" },
            }),
          }),
        ]),
      }),
    );
  });
});
