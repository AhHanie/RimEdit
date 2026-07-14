import { act, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useCustomFormViews } from "./useCustomFormViews";
import type { CustomFormView } from "../types/formViews";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

function sampleView(overrides: Partial<CustomFormView> = {}): CustomFormView {
  return {
    id: "view1",
    target: { gameVersion: "1.6", defType: "ThingDef" },
    name: "Weapon",
    description: null,
    hiddenFieldIds: ["apparel"],
    baseSchemaView: null,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("useCustomFormViews", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("loads views for the given project/gameVersion/defType scope on mount", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") {
        return Promise.resolve({ views: [sampleView()], warning: null });
      }
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { result } = renderHook(() => useCustomFormViews("proj1", "1.6", "ThingDef"));

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(invokeMock).toHaveBeenCalledWith("list_custom_form_views", {
      projectId: "proj1",
      gameVersion: "1.6",
      defType: "ThingDef",
    });
    expect(result.current.views).toHaveLength(1);
    expect(result.current.views[0].name).toBe("Weapon");
    expect(result.current.warning).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it("surfaces a store warning without treating it as an error", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") {
        return Promise.resolve({
          views: [],
          warning: {
            code: "form_view_store_unsupported_version",
            message: "newer store version",
          },
        });
      }
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { result } = renderHook(() => useCustomFormViews("proj1", "1.6", "ThingDef"));

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.views).toEqual([]);
    expect(result.current.warning?.code).toBe("form_view_store_unsupported_version");
    expect(result.current.error).toBeNull();
  });

  it("does not call list when there is no active project", async () => {
    const { result } = renderHook(() => useCustomFormViews(null, "1.6", "ThingDef"));

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.views).toEqual([]);
  });

  it("captures a rejected list call as an error", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") {
        return Promise.reject(new Error("boom"));
      }
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { result } = renderHook(() => useCustomFormViews("proj1", "1.6", "ThingDef"));

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.error).toBe("boom");
  });

  it("createView invokes create then reloads the list", async () => {
    let listCallCount = 0;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") {
        listCallCount += 1;
        const views = listCallCount === 1 ? [] : [sampleView()];
        return Promise.resolve({ views, warning: null });
      }
      if (cmd === "create_custom_form_view") {
        return Promise.resolve(sampleView());
      }
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { result } = renderHook(() => useCustomFormViews("proj1", "1.6", "ThingDef"));
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.createView("Weapon", ["apparel"]);
    });

    expect(invokeMock).toHaveBeenCalledWith("create_custom_form_view", {
      projectId: "proj1",
      gameVersion: "1.6",
      defType: "ThingDef",
      name: "Weapon",
      hiddenFieldIds: ["apparel"],
      description: null,
      baseSchemaView: null,
    });
    expect(result.current.views).toHaveLength(1);
  });

  it("deleteView invokes delete then reloads the list", async () => {
    let listCallCount = 0;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") {
        listCallCount += 1;
        const views = listCallCount === 1 ? [sampleView()] : [];
        return Promise.resolve({ views, warning: null });
      }
      if (cmd === "delete_custom_form_view") {
        return Promise.resolve({ deletedId: "view1" });
      }
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { result } = renderHook(() => useCustomFormViews("proj1", "1.6", "ThingDef"));
    await waitFor(() => expect(result.current.views).toHaveLength(1));

    await act(async () => {
      await result.current.deleteView("view1");
    });

    expect(invokeMock).toHaveBeenCalledWith("delete_custom_form_view", {
      projectId: "proj1",
      viewId: "view1",
    });
    await waitFor(() => expect(result.current.views).toHaveLength(0));
  });

  it("rejects mutating calls when there is no active project", async () => {
    const { result } = renderHook(() => useCustomFormViews(null, "1.6", "ThingDef"));
    await waitFor(() => expect(result.current.loading).toBe(false));

    await expect(result.current.createView("Weapon", [])).rejects.toThrow(
      "No active project",
    );
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("resetStore invokes reset then reloads the list", async () => {
    let listCallCount = 0;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") {
        listCallCount += 1;
        if (listCallCount === 1) {
          return Promise.resolve({
            views: [],
            warning: { code: "form_view_store_unsupported_version", message: "newer store" },
          });
        }
        return Promise.resolve({ views: [], warning: null });
      }
      if (cmd === "reset_custom_form_view_store") {
        return Promise.resolve({ backupPath: "/tmp/form-views.corrupt-123.bak" });
      }
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { result } = renderHook(() => useCustomFormViews("proj1", "1.6", "ThingDef"));
    await waitFor(() => expect(result.current.warning).not.toBeNull());

    let resetResult;
    await act(async () => {
      resetResult = await result.current.resetStore();
    });

    expect(invokeMock).toHaveBeenCalledWith("reset_custom_form_view_store", {
      projectId: "proj1",
    });
    expect(resetResult).toEqual({ backupPath: "/tmp/form-views.corrupt-123.bak" });
    await waitFor(() => expect(result.current.warning).toBeNull());
  });

  it("rejects resetStore when there is no active project", async () => {
    const { result } = renderHook(() => useCustomFormViews(null, "1.6", "ThingDef"));
    await waitFor(() => expect(result.current.loading).toBe(false));

    await expect(result.current.resetStore()).rejects.toThrow("No active project");
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("does not let a stale list request for an abandoned scope overwrite a newer scope's views/loading state", async () => {
    const viewA = sampleView({ id: "view-a", target: { gameVersion: "1.6", defType: "ThingDef" } });
    const viewB = sampleView({
      id: "view-b",
      name: "Version B view",
      target: { gameVersion: "1.5", defType: "ThingDef" },
    });

    let resolveScopeA!: (value: { views: CustomFormView[]; warning: null }) => void;
    const scopeAPromise = new Promise<{ views: CustomFormView[]; warning: null }>((resolve) => {
      resolveScopeA = resolve;
    });

    invokeMock.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd !== "list_custom_form_views") throw new Error(`unexpected invoke: ${cmd}`);
      const a = args as { gameVersion: string };
      if (a.gameVersion === "1.6") return scopeAPromise; // Slow -- stays pending.
      return Promise.resolve({ views: [viewB], warning: null }); // 1.5 resolves promptly.
    });

    const { result, rerender } = renderHook(
      (props: { gameVersion: string }) => useCustomFormViews("proj1", props.gameVersion, "ThingDef"),
      { initialProps: { gameVersion: "1.6" } },
    );

    // Scope A's (1.6) request is in flight (never resolved yet).
    expect(result.current.loading).toBe(true);

    // Switch to scope B (1.5) while A is still pending. B's own request resolves promptly.
    rerender({ gameVersion: "1.5" });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.views.map((v) => v.id)).toEqual(["view-b"]);

    // Now the stale scope-A request finally resolves, with a DIFFERENT view list.
    resolveScopeA({ views: [viewA], warning: null });
    // Give the stale `.then` every chance to (incorrectly) apply its result.
    await new Promise((resolve) => setTimeout(resolve, 0));
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Scope B's real state must still be in effect -- not overwritten by the stale scope-A
    // result, and `loading` must not have been reset to `true`/left in a stale state either.
    expect(result.current.loading).toBe(false);
    expect(result.current.views.map((v) => v.id)).toEqual(["view-b"]);
  });

  describe("stale CRUD completions never overwrite a since-abandoned scope's state", () => {
    // Shared shape for all four cases below: scope A (1.6) starts a slow mutation, the scope
    // switches to B (1.5) -- which loads its own real view list promptly -- while A's mutation
    // is still in flight, and only then does A's mutation resolve. In every case, A's mutation's
    // OWN chained `reload()` call must never fire (or, if it did, must never apply) against
    // scope B's now-current state -- this is the exact bug the reviewer found: the scope had
    // already moved on by the time the CRUD's chained reload was issued, so a request-id check
    // *inside* `reload()` alone isn't enough (it would already read as "current" by then).
    const viewA = sampleView({ id: "view-a", target: { gameVersion: "1.6", defType: "ThingDef" } });
    const viewB = sampleView({
      id: "view-b",
      name: "Version B view",
      target: { gameVersion: "1.5", defType: "ThingDef" },
    });

    function mockListByVersion() {
      return (cmd: string, args?: unknown) => {
        if (cmd !== "list_custom_form_views") return undefined;
        const a = args as { gameVersion: string };
        return Promise.resolve({ views: a.gameVersion === "1.5" ? [viewB] : [viewA], warning: null });
      };
    }

    it("createView", async () => {
      let resolveCreate!: (view: CustomFormView) => void;
      const createPromise = new Promise<CustomFormView>((resolve) => {
        resolveCreate = resolve;
      });
      const listByVersion = mockListByVersion();
      invokeMock.mockImplementation((cmd: string, args?: unknown) => {
        const listResult = listByVersion(cmd, args);
        if (listResult) return listResult;
        if (cmd === "create_custom_form_view") return createPromise;
        throw new Error(`unexpected invoke: ${cmd}`);
      });

      const { result, rerender } = renderHook(
        (props: { gameVersion: string }) => useCustomFormViews("proj1", props.gameVersion, "ThingDef"),
        { initialProps: { gameVersion: "1.6" } },
      );
      await waitFor(() => expect(result.current.views.map((v) => v.id)).toEqual(["view-a"]));

      // Deliberately not awaited: scope A's create is left in flight while we switch scope.
      let createCall!: Promise<CustomFormView>;
      act(() => {
        createCall = result.current.createView("New view", []);
      });

      act(() => {
        rerender({ gameVersion: "1.5" });
      });
      await waitFor(() => expect(result.current.views.map((v) => v.id)).toEqual(["view-b"]));

      // Resolving and awaiting inside a single `act()` ensures every state update the (now
      // stale) create call's completion -- including its chained `reload()`, if it fires at all
      // -- would produce is fully committed before the assertion below reads `result.current`,
      // so this is a deterministic check of the FINAL state, not a race against exactly when an
      // update happens to flush.
      await act(async () => {
        resolveCreate(sampleView({ id: "view-a-new", target: { gameVersion: "1.6", defType: "ThingDef" } }));
        await createCall;
      });

      expect(result.current.views.map((v) => v.id)).toEqual(["view-b"]);
    });

    it("updateView", async () => {
      let resolveUpdate!: (view: CustomFormView) => void;
      const updatePromise = new Promise<CustomFormView>((resolve) => {
        resolveUpdate = resolve;
      });
      const listByVersion = mockListByVersion();
      invokeMock.mockImplementation((cmd: string, args?: unknown) => {
        const listResult = listByVersion(cmd, args);
        if (listResult) return listResult;
        if (cmd === "update_custom_form_view") return updatePromise;
        throw new Error(`unexpected invoke: ${cmd}`);
      });

      const { result, rerender } = renderHook(
        (props: { gameVersion: string }) => useCustomFormViews("proj1", props.gameVersion, "ThingDef"),
        { initialProps: { gameVersion: "1.6" } },
      );
      await waitFor(() => expect(result.current.views.map((v) => v.id)).toEqual(["view-a"]));

      let updateCall!: Promise<CustomFormView>;
      act(() => {
        updateCall = result.current.updateView("view-a", { name: "Renamed" });
      });

      act(() => {
        rerender({ gameVersion: "1.5" });
      });
      await waitFor(() => expect(result.current.views.map((v) => v.id)).toEqual(["view-b"]));

      await act(async () => {
        resolveUpdate(sampleView({ id: "view-a", name: "Renamed" }));
        await updateCall;
      });

      expect(result.current.views.map((v) => v.id)).toEqual(["view-b"]);
    });

    it("deleteView", async () => {
      let resolveDelete!: (result: { deletedId: string }) => void;
      const deletePromise = new Promise<{ deletedId: string }>((resolve) => {
        resolveDelete = resolve;
      });
      const listByVersion = mockListByVersion();
      invokeMock.mockImplementation((cmd: string, args?: unknown) => {
        const listResult = listByVersion(cmd, args);
        if (listResult) return listResult;
        if (cmd === "delete_custom_form_view") return deletePromise;
        throw new Error(`unexpected invoke: ${cmd}`);
      });

      const { result, rerender } = renderHook(
        (props: { gameVersion: string }) => useCustomFormViews("proj1", props.gameVersion, "ThingDef"),
        { initialProps: { gameVersion: "1.6" } },
      );
      await waitFor(() => expect(result.current.views.map((v) => v.id)).toEqual(["view-a"]));

      let deleteCall!: Promise<void>;
      act(() => {
        deleteCall = result.current.deleteView("view-a");
      });

      act(() => {
        rerender({ gameVersion: "1.5" });
      });
      await waitFor(() => expect(result.current.views.map((v) => v.id)).toEqual(["view-b"]));

      await act(async () => {
        resolveDelete({ deletedId: "view-a" });
        await deleteCall;
      });

      expect(result.current.views.map((v) => v.id)).toEqual(["view-b"]);
    });

    it("resetStore", async () => {
      let resolveReset!: (result: { backupPath: string | null }) => void;
      const resetPromise = new Promise<{ backupPath: string | null }>((resolve) => {
        resolveReset = resolve;
      });
      const listByVersion = mockListByVersion();
      invokeMock.mockImplementation((cmd: string, args?: unknown) => {
        const listResult = listByVersion(cmd, args);
        if (listResult) return listResult;
        if (cmd === "reset_custom_form_view_store") return resetPromise;
        throw new Error(`unexpected invoke: ${cmd}`);
      });

      const { result, rerender } = renderHook(
        (props: { gameVersion: string }) => useCustomFormViews("proj1", props.gameVersion, "ThingDef"),
        { initialProps: { gameVersion: "1.6" } },
      );
      await waitFor(() => expect(result.current.views.map((v) => v.id)).toEqual(["view-a"]));

      let resetCall!: Promise<{ backupPath: string | null }>;
      act(() => {
        resetCall = result.current.resetStore();
      });

      act(() => {
        rerender({ gameVersion: "1.5" });
      });
      await waitFor(() => expect(result.current.views.map((v) => v.id)).toEqual(["view-b"]));

      await act(async () => {
        resolveReset({ backupPath: null });
        await resetCall;
      });

      expect(result.current.views.map((v) => v.id)).toEqual(["view-b"]);
    });
  });

  it("both of two concurrent same-scope creates eventually appear in the UI, even if the second one's reload resolves first", async () => {
    // Simulates the backend's actual committed state, read fresh by every `list_custom_form_view`
    // call -- exactly like the real store would behave.
    let backendViews: CustomFormView[] = [];
    let resolveCreate1!: (view: CustomFormView) => void;
    const create1Promise = new Promise<CustomFormView>((resolve) => {
      resolveCreate1 = resolve;
    });
    let resolveCreate2!: (view: CustomFormView) => void;
    const create2Promise = new Promise<CustomFormView>((resolve) => {
      resolveCreate2 = resolve;
    });
    let createCallCount = 0;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") {
        return Promise.resolve({ views: [...backendViews], warning: null });
      }
      if (cmd === "create_custom_form_view") {
        createCallCount += 1;
        return createCallCount === 1 ? create1Promise : create2Promise;
      }
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { result } = renderHook(() => useCustomFormViews("proj1", "1.6", "ThingDef"));
    await waitFor(() => expect(result.current.loading).toBe(false));

    let call1!: Promise<CustomFormView>;
    let call2!: Promise<CustomFormView>;
    act(() => {
      call1 = result.current.createView("View 1", []);
      call2 = result.current.createView("View 2", []);
    });

    // The SECOND create's backend write commits and its own reload resolves FIRST -- before the
    // first create's write has landed at all.
    const view2 = sampleView({ id: "view-2", name: "View 2" });
    await act(async () => {
      backendViews = [view2];
      resolveCreate2(view2);
      await call2;
    });
    expect(result.current.views.map((v) => v.id)).toEqual(["view-2"]);

    // NOW the first (slower) create's backend write finally commits too.
    const view1 = sampleView({ id: "view-1", name: "View 1" });
    await act(async () => {
      backendViews = [view2, view1];
      resolveCreate1(view1);
      await call1;
    });

    // Both views must be visible -- the first create's own reload must not have been skipped
    // just because the second create's reload ALSO ran and bumped a shared counter in the
    // meantime (the scope itself never changed, only another same-scope operation happened).
    expect(result.current.views.map((v) => v.id).sort()).toEqual(["view-1", "view-2"]);
  });
});
