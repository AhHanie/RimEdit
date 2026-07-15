import { screen } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import userEvent from "@testing-library/user-event";
import { vi, describe, it, expect } from "vitest";
import { GraphicDataPreview } from "./GraphicDataPreview";
import * as hookModule from "../../hooks/useGraphicDataPreview";
import type { GraphicDataPreviewState } from "../../hooks/useGraphicDataPreview";
import type { GraphicPreviewVariant } from "../../types/graphicPreview";
import { makeGraphicPreviewVariant, makeGraphicPreviewResult } from "../../__fixtures__/graphicData";

vi.mock("../../hooks/useGraphicDataPreview");
vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: vi.fn((filePath: string, protocol = "asset") =>
    `converted://${protocol}/${encodeURIComponent(filePath)}`,
  ),
}));

const mockUsePreview = vi.mocked(hookModule.useGraphicDataPreview);

function makeVariant(overrides?: Partial<GraphicPreviewVariant>): GraphicPreviewVariant {
  return {
    id: "v1",
    label: { kind: "direction", direction: "south" },
    role: "south",
    sourceLocationId: "loc1",
    sourceLocationName: "Project",
    relativeTexturePath: "Things/Wall_south.png",
    assetUrl: "rimedit-asset://localhost/token1",
    ...overrides,
  };
}

function makeState(overrides?: Partial<GraphicDataPreviewState>): GraphicDataPreviewState {
  return {
    status: "idle",
    result: null,
    selectedIndex: 0,
    selectedVariant: null,
    warnings: [],
    error: null,
    canGoPrevious: false,
    canGoNext: false,
    goPrevious: vi.fn(),
    goNext: vi.fn(),
    selectVariant: vi.fn(),
    ...overrides,
  };
}

describe("GraphicDataPreview", () => {
  it("renders nothing when texPath and graphicClass are both empty", () => {
    mockUsePreview.mockReturnValue(makeState());
    const { container } = render(
      <GraphicDataPreview projectId="p1" texPath="" graphicClass="" />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("shows image with correct src and alt for a supported variant", () => {
    const variant = makeVariant();
    mockUsePreview.mockReturnValue(
      makeState({
        status: "ready",
        selectedVariant: variant,
        result: {
          texPath: "Things/Wall",
          graphicClass: "Graphic_Single",
          variants: [variant],
          warnings: [],
        },
      }),
    );
    render(
      <GraphicDataPreview
        projectId="p1"
        texPath="Things/Wall"
        graphicClass="Graphic_Single"
      />,
    );
    const img = screen.getByRole("img");
    expect(img.getAttribute("src")).toBe("rimedit-asset://localhost/token1");
    expect(img.getAttribute("alt")).toBe("Graphic_Single South preview");
  });

  it("converts asset tokens through Tauri before rendering the image", () => {
    const variant = makeVariant({ assetToken: "token1" });
    mockUsePreview.mockReturnValue(
      makeState({
        status: "ready",
        selectedVariant: variant,
        result: {
          texPath: "Things/Wall",
          graphicClass: "Graphic_Single",
          variants: [variant],
          warnings: [],
        },
      }),
    );
    render(
      <GraphicDataPreview
        projectId="p1"
        texPath="Things/Wall"
        graphicClass="Graphic_Single"
      />,
    );
    expect(screen.getByRole("img").getAttribute("src")).toBe("converted://rimedit-asset/token1");
  });

  it("shows South 3/4 style carousel label for multi variants", () => {
    const variants = [
      makeVariant({ id: "n", label: { kind: "direction", direction: "north" }, role: "north" }),
      makeVariant({ id: "e", label: { kind: "direction", direction: "east" }, role: "east" }),
      makeVariant({
        id: "s",
        label: { kind: "direction", direction: "south" },
        role: "south",
        assetUrl: "rimedit-asset://localhost/t3",
      }),
      makeVariant({ id: "w", label: { kind: "direction", direction: "west" }, role: "west" }),
    ];
    mockUsePreview.mockReturnValue(
      makeState({
        status: "ready",
        selectedVariant: variants[2],
        selectedIndex: 2,
        result: {
          texPath: "Things/Wall",
          graphicClass: "Graphic_Multi",
          variants,
          warnings: [],
        },
        canGoPrevious: true,
        canGoNext: true,
      }),
    );
    render(
      <GraphicDataPreview
        projectId="p1"
        texPath="Things/Wall"
        graphicClass="Graphic_Multi"
      />,
    );
    expect(screen.getByText("South 3/4")).not.toBeNull();
  });

  it("calls goPrevious and goNext when carousel buttons are clicked", async () => {
    const user = userEvent.setup();
    const goPrev = vi.fn();
    const goNext = vi.fn();
    const variants = [
      makeVariant({ id: "a" }),
      makeVariant({ id: "b", label: { kind: "variant", index: 2 } }),
    ];
    mockUsePreview.mockReturnValue(
      makeState({
        status: "ready",
        selectedVariant: variants[0],
        result: { texPath: "p", graphicClass: "g", variants, warnings: [] },
        canGoPrevious: true,
        canGoNext: true,
        goPrevious: goPrev,
        goNext: goNext,
      }),
    );
    render(<GraphicDataPreview projectId="p1" texPath="p" graphicClass="g" />);
    await user.click(screen.getByRole("button", { name: /previous texture variant/i }));
    expect(goPrev).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: /next texture variant/i }));
    expect(goNext).toHaveBeenCalled();
  });

  it("shows missing texture message without rendering a broken image", () => {
    const variant = makeVariant({ missing: true });
    mockUsePreview.mockReturnValue(
      makeState({
        status: "ready",
        selectedVariant: variant,
        result: { texPath: "p", graphicClass: "g", variants: [variant], warnings: [] },
      }),
    );
    render(<GraphicDataPreview projectId="p1" texPath="p" graphicClass="g" />);
    expect(screen.getByText("Texture not found")).not.toBeNull();
    expect(screen.queryByRole("img")).toBeNull();
  });

  it("shows unsupported format message for dds variant without rendering an image", () => {
    const variant = makeVariant({
      relativeTexturePath: "Things/Wall.dds",
      assetUrl: "rimedit-asset://localhost/t1",
    });
    mockUsePreview.mockReturnValue(
      makeState({
        status: "ready",
        selectedVariant: variant,
        result: { texPath: "p", graphicClass: "g", variants: [variant], warnings: [] },
      }),
    );
    render(<GraphicDataPreview projectId="p1" texPath="p" graphicClass="g" />);
    expect(screen.getByText(/DDS format not supported/i)).not.toBeNull();
    expect(screen.queryByRole("img")).toBeNull();
  });

  it("renders warnings inline below the preview region", () => {
    const variant = makeVariant();
    mockUsePreview.mockReturnValue(
      makeState({
        status: "ready",
        selectedVariant: variant,
        result: {
          texPath: "p",
          graphicClass: "g",
          variants: [variant],
          warnings: [{ code: "test_missing_mask_texture", message: "Missing mask texture" }],
        },
        warnings: [{ code: "test_missing_mask_texture", message: "Missing mask texture" }],
      }),
    );
    render(<GraphicDataPreview projectId="p1" texPath="p" graphicClass="g" />);
    expect(screen.getByText("Missing mask texture")).not.toBeNull();
  });

  it("renders fixture multi result and changes active variant", async () => {
    const user = userEvent.setup();
    const variantNorth = makeGraphicPreviewVariant({
      id: "north",
      label: { kind: "direction", direction: "north" },
      role: "north",
    });
    const variantSouth = makeGraphicPreviewVariant({
      id: "south",
      label: { kind: "direction", direction: "south" },
      role: "south",
    });
    const result = makeGraphicPreviewResult({
      graphicClass: "Graphic_Multi",
      variants: [variantNorth, variantSouth],
    });
    const goNext = vi.fn();

    mockUsePreview.mockReturnValue(
      makeState({
        status: "ready",
        selectedVariant: variantNorth,
        selectedIndex: 0,
        result,
        canGoNext: true,
        canGoPrevious: false,
        goNext,
      }),
    );

    render(
      <GraphicDataPreview
        projectId="fixture-proj"
        texPath="Things/Fixture/Multi/FixtureMulti"
        graphicClass="Graphic_Multi"
      />,
    );

    expect(screen.getByText(/North/)).not.toBeNull();

    const nextButton = screen.getByRole("button", { name: /next/i });
    await user.click(nextButton);
    expect(goNext).toHaveBeenCalled();
  });
});
