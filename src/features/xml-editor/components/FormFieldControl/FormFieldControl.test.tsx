import { render, screen } from "@testing-library/react";
import { vi, describe, it, expect } from "vitest";
import { FormFieldControl } from "./FormFieldControl";
import { XmlEditorContextProvider } from "../../context/XmlEditorContext";
import type { XmlFormApi } from "../../hooks/useXmlFormController";
import type {
  FormFieldState,
  ObjectListItemValue,
} from "../../types/editorForm";
import type { SchemaCatalog } from "../../../schema-catalog";
import {
  FormFieldStore,
  type StoredFieldState,
} from "../../lib/formFieldStore";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

function makeToolCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {},
    objectTypes: {
      Tool: {
        fieldOrder: ["label", "power", "cooldownTime"],
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
          cooldownTime: {
            label: "Cooldown time",
            type: { kind: "float" },
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

function makeToolsField(items: ObjectListItemValue[]): FormFieldState {
  return {
    model: {
      id: "ThingDef:Test:objectList:tools",
      key: "tools",
      label: "Tools",
      control: "objectList",
      path: { kind: "objectList", objectPath: [], fieldName: "tools" },
      fieldPath: ["tools"],
      sourceNodeId: 42,
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
    initialValue: { kind: "objectList", items },
    dirty: false,
    touched: false,
    focused: false,
    pending: false,
    error: null,
    validationErrors: [],
    clearRequested: false,
  };
}

function makeStore(field: FormFieldState): FormFieldStore {
  const store = new FormFieldStore();
  const stored: StoredFieldState = {
    model: field.model,
    value: field.value,
    initialValue: field.initialValue,
    dirty: field.dirty,
    touched: field.touched,
    focused: field.focused,
    pending: field.pending,
    error: field.error,
    cachedValidationErrors: field.validationErrors,
    clearRequested: field.clearRequested,
  };
  store.initialize([field.model], new Map([[field.model.id, stored]]));
  return store;
}

function makeFormApi(field: FormFieldState): XmlFormApi {
  const actions = {
    setFieldValue: vi.fn(),
    focusField: vi.fn(),
    blurField: vi.fn(),
    resetField: vi.fn(),
    clearField: vi.fn(),
    discardDrafts: vi.fn(),
    flushField: vi.fn().mockResolvedValue(null),
    flushAll: vi.fn().mockResolvedValue(null),
  };
  return {
    snapshot: { defNodeId: 1, fields: [field] },
    store: makeStore(field),
    actions,
    hasDraftChanges: false,
    hasPendingCommits: false,
    hasBlockingErrors: false,
    formError: null,
    setFieldValue: actions.setFieldValue,
    focusField: actions.focusField,
    blurField: actions.blurField,
    resetField: actions.resetField,
    clearField: actions.clearField,
    discardDrafts: actions.discardDrafts,
    flushField: actions.flushField,
    flushAll: actions.flushAll,
  };
}

const toolItem: ObjectListItemValue = {
  nodeId: 10,
  className: "",
  schemaRef: "Tool",
  fields: {
    label: { kind: "scalar", value: "stock" },
    power: { kind: "scalar", value: "9" },
    cooldownTime: { kind: "scalar", value: "2" },
  },
  initialUnknownFieldCount: 0,
};

describe("FormFieldControl - read-only objectList rendering", () => {
  it("renders structured item details instead of a '(n items)' summary when the file is read-only", () => {
    const field = makeToolsField([toolItem]);
    const formApi = makeFormApi(field);

    render(
      <XmlEditorContextProvider
        value={{ readOnly: true, catalog: makeToolCatalog() }}
      >
        <FormFieldControl
          fieldId={field.model.id}
          store={formApi.store}
          formApi={formApi}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByText("(1 item)")).toBeNull();
    screen.getByText("Power");
    screen.getByText("Cooldown time");
    screen.getByText("stock");
  });

  it("hides add/remove controls and disables inputs in the read-only view", () => {
    const field = makeToolsField([toolItem]);
    const formApi = makeFormApi(field);

    render(
      <XmlEditorContextProvider
        value={{ readOnly: true, catalog: makeToolCatalog() }}
      >
        <FormFieldControl
          fieldId={field.model.id}
          store={formApi.store}
          formApi={formApi}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByRole("button", { name: /add item/i })).toBeNull();
    expect(
      screen.queryByRole("button", { name: /remove/i }),
    ).toBeNull();
    const labelInput = screen.getByDisplayValue("stock") as HTMLInputElement;
    expect(labelInput.disabled).toBe(true);
  });

  it("still allows expand/collapse in the read-only view", () => {
    const field = makeToolsField([toolItem]);
    const formApi = makeFormApi(field);

    render(
      <XmlEditorContextProvider
        value={{ readOnly: true, catalog: makeToolCatalog() }}
      >
        <FormFieldControl
          fieldId={field.model.id}
          store={formApi.store}
          formApi={formApi}
        />
      </XmlEditorContextProvider>,
    );

    const collapseBtn = screen.getByRole(
      "button",
      { name: /collapse/i },
    ) as HTMLButtonElement;
    expect(collapseBtn.disabled).toBe(false);
  });
});

describe("FormFieldControl - editable objectList rendering (regression)", () => {
  it("keeps add controls and calls formApi.setFieldValue on edit when not read-only", () => {
    const field = makeToolsField([toolItem]);
    const formApi = makeFormApi(field);

    render(
      <XmlEditorContextProvider
        value={{ readOnly: false, catalog: makeToolCatalog() }}
      >
        <FormFieldControl
          fieldId={field.model.id}
          store={formApi.store}
          formApi={formApi}
        />
      </XmlEditorContextProvider>,
    );

    screen.getByRole("button", { name: /add item/i });
    screen.getByText("stock");
  });
});

function makeStuffCategoriesField(items: string[]): FormFieldState {
  return {
    model: {
      id: "ThingDef:Test:list:stuffCategories",
      key: "stuffCategories",
      label: "Stuff categories",
      control: "list",
      path: { kind: "objectList", objectPath: [], fieldName: "stuffCategories" },
      fieldPath: ["stuffCategories"],
      sourceNodeId: 7,
      defNodeId: 1,
      order: 0,
      readonly: false,
      required: false,
      repeatable: false,
      xmlShape: "listOfLi",
      examples: [],
      diagnostics: [],
      sectionDefaults: [],
      reference: {
        defType: "StuffCategoryDef",
        allowAbstract: false,
        scope: "allSources",
      },
    },
    value: { kind: "list", items },
    initialValue: { kind: "list", items },
    dirty: false,
    touched: false,
    focused: false,
    pending: false,
    error: null,
    validationErrors: [],
    clearRequested: false,
  };
}

describe("FormFieldControl - read-only reference list rendering", () => {
  it("hides the Add item and per-row remove buttons for a read-only defReference list", () => {
    const field = makeStuffCategoriesField(["Metallic"]);
    const formApi = makeFormApi(field);

    render(
      <XmlEditorContextProvider
        value={{ readOnly: true, projectId: "test-project" }}
      >
        <FormFieldControl
          fieldId={field.model.id}
          store={formApi.store}
          formApi={formApi}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByRole("button", { name: /add item/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /remove/i })).toBeNull();
    screen.getByDisplayValue("Metallic");
  });

  it("keeps the Add item and remove buttons for an editable defReference list (regression)", () => {
    const field = makeStuffCategoriesField(["Metallic"]);
    const formApi = makeFormApi(field);

    render(
      <XmlEditorContextProvider
        value={{ readOnly: false, projectId: "test-project" }}
      >
        <FormFieldControl
          fieldId={field.model.id}
          store={formApi.store}
          formApi={formApi}
        />
      </XmlEditorContextProvider>,
    );

    screen.getByRole("button", { name: /add item/i });
    screen.getByRole("button", { name: /remove item 1/i });
  });
});
