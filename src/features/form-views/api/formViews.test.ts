import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  createCustomFormView,
  deleteCustomFormView,
  getLastSelectedFormView,
  listCustomFormViews,
  resetCustomFormViewStore,
  setLastSelectedFormView,
  updateCustomFormView,
} from "./formViews";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("listCustomFormViews", () => {
  it("maps optional scope filters to null when omitted", async () => {
    invokeMock.mockResolvedValue({ views: [], warning: null });

    await listCustomFormViews("proj1");

    expect(invokeMock).toHaveBeenCalledWith("list_custom_form_views", {
      projectId: "proj1",
      gameVersion: null,
      defType: null,
    });
  });

  it("passes through gameVersion/defType filters when given", async () => {
    invokeMock.mockResolvedValue({ views: [], warning: null });

    await listCustomFormViews("proj1", "1.6", "ThingDef");

    expect(invokeMock).toHaveBeenCalledWith("list_custom_form_views", {
      projectId: "proj1",
      gameVersion: "1.6",
      defType: "ThingDef",
    });
  });
});

describe("createCustomFormView", () => {
  it("defaults description and baseSchemaView to null when omitted", async () => {
    invokeMock.mockResolvedValue({});

    await createCustomFormView("proj1", "1.6", "ThingDef", "Weapon", ["apparel"]);

    expect(invokeMock).toHaveBeenCalledWith("create_custom_form_view", {
      projectId: "proj1",
      gameVersion: "1.6",
      defType: "ThingDef",
      name: "Weapon",
      hiddenFieldIds: ["apparel"],
      description: null,
      baseSchemaView: null,
    });
  });

  it("forwards an explicit description and baseSchemaView", async () => {
    invokeMock.mockResolvedValue({});
    const baseSchemaView = {
      viewId: "weapon",
      packId: "rimedit.rimworld.core",
      packVersion: "1.6.0",
      declaredOnDefType: "ThingDef",
    };

    await createCustomFormView(
      "proj1",
      "1.6",
      "ThingDef",
      "Weapon",
      ["apparel"],
      "My weapon view",
      baseSchemaView,
    );

    expect(invokeMock).toHaveBeenCalledWith("create_custom_form_view", {
      projectId: "proj1",
      gameVersion: "1.6",
      defType: "ThingDef",
      name: "Weapon",
      hiddenFieldIds: ["apparel"],
      description: "My weapon view",
      baseSchemaView,
    });
  });
});

describe("updateCustomFormView", () => {
  it("maps omitted update fields to null/false and does not touch description", async () => {
    invokeMock.mockResolvedValue({});

    await updateCustomFormView("proj1", "view1", {});

    expect(invokeMock).toHaveBeenCalledWith("update_custom_form_view", {
      projectId: "proj1",
      viewId: "view1",
      name: null,
      hiddenFieldIds: null,
      description: null,
      clearDescription: false,
    });
  });

  it("forwards provided name/hiddenFieldIds fields", async () => {
    invokeMock.mockResolvedValue({});

    await updateCustomFormView("proj1", "view1", {
      name: "Ranged weapon",
      hiddenFieldIds: ["race"],
    });

    expect(invokeMock).toHaveBeenCalledWith("update_custom_form_view", {
      projectId: "proj1",
      viewId: "view1",
      name: "Ranged weapon",
      hiddenFieldIds: ["race"],
      description: null,
      clearDescription: false,
    });
  });

  it("sets clearDescription when description is explicitly null", async () => {
    invokeMock.mockResolvedValue({});

    await updateCustomFormView("proj1", "view1", { description: null });

    expect(invokeMock).toHaveBeenCalledWith("update_custom_form_view", {
      projectId: "proj1",
      viewId: "view1",
      name: null,
      hiddenFieldIds: null,
      description: null,
      clearDescription: true,
    });
  });

  it("forwards a new description without clearing", async () => {
    invokeMock.mockResolvedValue({});

    await updateCustomFormView("proj1", "view1", { description: "New description" });

    expect(invokeMock).toHaveBeenCalledWith("update_custom_form_view", {
      projectId: "proj1",
      viewId: "view1",
      name: null,
      hiddenFieldIds: null,
      description: "New description",
      clearDescription: false,
    });
  });
});

describe("resetCustomFormViewStore", () => {
  it("invokes reset with just the projectId", async () => {
    invokeMock.mockResolvedValue({ backupPath: "/tmp/form-views.corrupt-123.bak" });

    const result = await resetCustomFormViewStore("proj1");

    expect(invokeMock).toHaveBeenCalledWith("reset_custom_form_view_store", {
      projectId: "proj1",
    });
    expect(result.backupPath).toBe("/tmp/form-views.corrupt-123.bak");
  });
});

describe("deleteCustomFormView", () => {
  it("invokes delete with projectId and viewId", async () => {
    invokeMock.mockResolvedValue({ deletedId: "view1" });

    await deleteCustomFormView("proj1", "view1");

    expect(invokeMock).toHaveBeenCalledWith("delete_custom_form_view", {
      projectId: "proj1",
      viewId: "view1",
    });
  });
});

describe("setLastSelectedFormView / getLastSelectedFormView", () => {
  it("sets the last-selected preference with origin and id", async () => {
    invokeMock.mockResolvedValue(undefined);

    await setLastSelectedFormView("proj1", "1.6", "ThingDef", "custom", "view1");

    expect(invokeMock).toHaveBeenCalledWith("set_last_selected_form_view", {
      projectId: "proj1",
      gameVersion: "1.6",
      defType: "ThingDef",
      origin: "custom",
      id: "view1",
    });
  });

  it("reads the last-selected preference for a scope", async () => {
    invokeMock.mockResolvedValue({ selected: null, warning: null });

    await getLastSelectedFormView("proj1", "1.6", "ThingDef");

    expect(invokeMock).toHaveBeenCalledWith("get_last_selected_form_view", {
      projectId: "proj1",
      gameVersion: "1.6",
      defType: "ThingDef",
    });
  });
});
