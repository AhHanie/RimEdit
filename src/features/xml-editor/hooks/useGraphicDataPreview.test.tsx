import { renderHook, act } from "@testing-library/react";
import { vi, describe, it, expect, beforeEach, afterEach } from "vitest";
import { useGraphicDataPreview } from "./useGraphicDataPreview";
import { resolveGraphicPreviewAssets } from "../api/xmlDocument";
import type {
  GraphicPreviewAssetResult,
  GraphicPreviewVariant,
} from "../types/graphicPreview";

vi.mock("../api/xmlDocument", () => ({
  resolveGraphicPreviewAssets: vi.fn(),
}));

const mockResolve = vi.mocked(resolveGraphicPreviewAssets);

function makeVariant(
  overrides?: Partial<GraphicPreviewVariant>,
): GraphicPreviewVariant {
  return {
    id: "v1",
    label: "Single",
    role: "single",
    sourceLocationId: "loc1",
    sourceLocationName: "Project",
    relativeTexturePath: "Things/Wall.png",
    assetUrl: "rimedit-asset://localhost/token1",
    ...overrides,
  };
}

function makeResult(
  overrides?: Partial<GraphicPreviewAssetResult>,
): GraphicPreviewAssetResult {
  return {
    texPath: "Things/Wall",
    graphicClass: "Graphic_Single",
    variants: [makeVariant()],
    warnings: [],
    ...overrides,
  };
}

describe("useGraphicDataPreview", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockResolve.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("stays idle and issues no call when texPath is empty", () => {
    const { result } = renderHook(() =>
      useGraphicDataPreview("proj1", "", "Graphic_Single"),
    );
    expect(result.current.status).toBe("idle");
    expect(mockResolve).not.toHaveBeenCalled();
  });

  it("stays idle and issues no call when graphicClass is empty", () => {
    const { result } = renderHook(() =>
      useGraphicDataPreview("proj1", "Things/Wall", ""),
    );
    expect(result.current.status).toBe("idle");
    expect(mockResolve).not.toHaveBeenCalled();
  });

  it("stays idle and issues no call when projectId is missing", () => {
    const { result } = renderHook(() =>
      useGraphicDataPreview(undefined, "Things/Wall", "Graphic_Single"),
    );
    expect(result.current.status).toBe("idle");
    expect(mockResolve).not.toHaveBeenCalled();
  });

  it("debounces multiple rapid texPath changes into one call", async () => {
    mockResolve.mockResolvedValue(makeResult());

    const { result, rerender } = renderHook(
      ({ texPath }: { texPath: string }) =>
        useGraphicDataPreview("proj1", texPath, "Graphic_Single"),
      { initialProps: { texPath: "Things/A" } },
    );

    rerender({ texPath: "Things/B" });
    rerender({ texPath: "Things/C" });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    expect(result.current.status).toBe("ready");
    expect(mockResolve).toHaveBeenCalledTimes(1);
    expect(mockResolve).toHaveBeenCalledWith(
      "proj1",
      "Things/C",
      "Graphic_Single",
      undefined,
    );
  });

  it("ignores stale responses from earlier requests", async () => {
    let resolveFirst!: (v: GraphicPreviewAssetResult) => void;
    let resolveSecond!: (v: GraphicPreviewAssetResult) => void;

    mockResolve
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveFirst = res;
        }),
      )
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveSecond = res;
        }),
      );

    const { result, rerender } = renderHook(
      ({ texPath }: { texPath: string }) =>
        useGraphicDataPreview("proj1", texPath, "Graphic_Single"),
      { initialProps: { texPath: "Things/A" } },
    );

    // Trigger first debounce
    act(() => {
      vi.advanceTimersByTime(300);
    });

    rerender({ texPath: "Things/B" });

    // Trigger second debounce
    act(() => {
      vi.advanceTimersByTime(300);
    });

    // Resolve second (newer) first
    await act(async () => {
      resolveSecond(makeResult({ texPath: "Things/B" }));
    });

    expect(result.current.result?.texPath).toBe("Things/B");

    // Resolve first (stale) - should be ignored
    await act(async () => {
      resolveFirst(makeResult({ texPath: "Things/A" }));
    });

    expect(result.current.result?.texPath).toBe("Things/B");
  });

  it("selects first non-missing variant, skipping missing ones at the start", async () => {
    mockResolve.mockResolvedValue(
      makeResult({
        variants: [
          makeVariant({
            id: "n",
            label: "North",
            role: "north",
            missing: true,
          }),
          makeVariant({ id: "e", label: "East", role: "east" }),
        ],
      }),
    );

    const { result } = renderHook(() =>
      useGraphicDataPreview("proj1", "Things/Foo", "Graphic_Multi"),
    );

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    expect(result.current.selectedIndex).toBe(1);
    expect(result.current.selectedVariant?.label).toBe("East");
  });

  it("exposes resolver warnings without throwing", async () => {
    mockResolve.mockResolvedValue(
      makeResult({ warnings: ["Missing south texture"] }),
    );

    const { result } = renderHook(() =>
      useGraphicDataPreview("proj1", "Things/Foo", "Graphic_Single"),
    );

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    expect(result.current.status).toBe("ready");
    expect(result.current.warnings).toEqual(["Missing south texture"]);
  });

  it("exposes error as preview-only state without throwing", async () => {
    mockResolve.mockRejectedValue({ message: "Resolver failed" });

    const { result } = renderHook(() =>
      useGraphicDataPreview("proj1", "Things/Foo", "Graphic_Single"),
    );

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    expect(result.current.status).toBe("error");
    expect(result.current.error).toBe("Resolver failed");
    expect(result.current.result).toBeNull();
  });

  it("does not let an in-flight response overwrite idle state after inputs are cleared", async () => {
    let resolveRequest!: (v: GraphicPreviewAssetResult) => void;
    mockResolve.mockReturnValueOnce(
      new Promise((res) => {
        resolveRequest = res;
      }),
    );

    const { result, rerender } = renderHook(
      ({ texPath }: { texPath: string }) =>
        useGraphicDataPreview("proj1", texPath, "Graphic_Single"),
      { initialProps: { texPath: "Things/Wall" } },
    );

    // Fire the debounce - request is now in-flight
    act(() => {
      vi.advanceTimersByTime(300);
    });

    // Clear the input - should invalidate the in-flight request
    rerender({ texPath: "" });

    expect(result.current.status).toBe("idle");

    // The old promise resolves after the input was cleared
    await act(async () => {
      resolveRequest(makeResult());
    });

    // Must remain idle; the stale response must not be applied
    expect(result.current.status).toBe("idle");
    expect(result.current.result).toBeNull();
  });

  it("triggers a new request when graphicClass changes", async () => {
    mockResolve.mockResolvedValue(makeResult());

    const { rerender } = renderHook(
      ({ graphicClass }: { graphicClass: string }) =>
        useGraphicDataPreview("proj1", "Things/Foo", graphicClass),
      { initialProps: { graphicClass: "Graphic_Single" } },
    );

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    rerender({ graphicClass: "Graphic_Multi" });

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    expect(mockResolve).toHaveBeenCalledTimes(2);
    expect(mockResolve).toHaveBeenLastCalledWith(
      "proj1",
      "Things/Foo",
      "Graphic_Multi",
      undefined,
    );
  });
});
