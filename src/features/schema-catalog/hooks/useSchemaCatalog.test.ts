import { renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useSchemaCatalog } from "./useSchemaCatalog";
import type { SchemaCatalog } from "../types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

function catalogWithLabel(label: string): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {
      ThingDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: [],
        fields: {},
        label,
      },
    },
    objectTypes: {},
  };
}

describe("useSchemaCatalog", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("passes the active locale through to the load_schema_catalog command", async () => {
    invokeMock.mockResolvedValue({ catalog: catalogWithLabel("Thing"), diagnostics: [] });

    renderHook(() => useSchemaCatalog(["root"], "1.6", "en"));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("load_schema_catalog", {
        extraSchemaRoots: ["root"],
        gameVersion: "1.6",
        locale: "en",
      }),
    );
  });

  it("reloads the catalog when the locale changes", async () => {
    invokeMock.mockImplementation((_cmd, args) => {
      const a = args as { locale?: string };
      return Promise.resolve({
        catalog: catalogWithLabel(a.locale === "en" ? "Thing (en)" : "Thing (other)"),
        diagnostics: [],
      });
    });

    const { result, rerender } = renderHook(
      (props: { locale: string }) => useSchemaCatalog(undefined, "1.6", props.locale),
      { initialProps: { locale: "en" } },
    );

    await waitFor(() =>
      expect(result.current.catalog?.defTypes.ThingDef.label).toBe("Thing (en)"),
    );

    rerender({ locale: "other" });

    await waitFor(() =>
      expect(result.current.catalog?.defTypes.ThingDef.label).toBe("Thing (other)"),
    );
  });

  it("discards a slow, superseded response instead of applying it after a newer switch resolves first", async () => {
    let resolveFirst!: (value: { catalog: SchemaCatalog; diagnostics: [] }) => void;
    const firstPromise = new Promise<{ catalog: SchemaCatalog; diagnostics: [] }>((resolve) => {
      resolveFirst = resolve;
    });

    invokeMock.mockImplementation((_cmd, args) => {
      const a = args as { locale?: string };
      if (a.locale === "en") return firstPromise;
      return Promise.resolve({ catalog: catalogWithLabel("Thing (other)"), diagnostics: [] });
    });

    const { result, rerender } = renderHook(
      (props: { locale: string }) => useSchemaCatalog(undefined, "1.6", props.locale),
      { initialProps: { locale: "en" } },
    );

    // Switch locale before the first request resolves.
    rerender({ locale: "other" });
    await waitFor(() =>
      expect(result.current.catalog?.defTypes.ThingDef.label).toBe("Thing (other)"),
    );

    // The first, now-stale request finally resolves -- it must not overwrite the newer result.
    resolveFirst({ catalog: catalogWithLabel("Thing (en)"), diagnostics: [] });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(result.current.catalog?.defTypes.ThingDef.label).toBe("Thing (other)");
  });

  it("falls back to an error state without leaking a stale catalog when the command rejects", async () => {
    invokeMock.mockRejectedValue(new Error("boom"));

    const { result } = renderHook(() => useSchemaCatalog(undefined, "1.6", "en"));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.catalog).toBeNull();
    expect(result.current.error).not.toBeNull();
  });
});
