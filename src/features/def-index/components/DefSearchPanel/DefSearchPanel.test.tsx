import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { DefSearchPanel } from "./DefSearchPanel";
import type { DefIndexFacetSummary, IndexedDefSearchResult } from "../../types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

const facets: DefIndexFacetSummary = {
  defTypes: [
    { defType: "ThingDef", projectCount: 2, sourceCount: 1, totalCount: 3 },
    { defType: "RecipeDef", projectCount: 1, sourceCount: 0, totalCount: 1 },
  ],
  projectDefs: 3,
  sourceDefs: 1,
  errors: 0,
};

const projectResult: IndexedDefSearchResult = {
  rank: 1,
  def: {
    key: { defType: "ThingDef", defName: "Steel" },
    defType: "ThingDef",
    defName: "Steel",
    label: "steel",
    relativePath: "Defs/Things/Steel.xml",
    nodeId: 42,
    source: {
      locationId: "proj1",
      locationName: "My Mod",
      sourceKind: "project",
      sourceType: "folder",
      readOnly: false,
    },
    fields: [],
  },
};

const sourceResult: IndexedDefSearchResult = {
  rank: 1,
  def: {
    key: { defType: "ThingDef", defName: "Steel" },
    defType: "ThingDef",
    defName: "Steel",
    label: "steel",
    relativePath: "Things/Resources/Steel.xml",
    source: {
      locationId: "core1",
      locationName: "Core",
      sourceKind: "source",
      sourceType: "baseGame",
      readOnly: true,
    },
    fields: [{ name: "stackLimit", textValue: "75" }],
  },
};

function defaultProps(
  overrides: Partial<Parameters<typeof DefSearchPanel>[0]> = {},
) {
  return {
    visible: true,
    projectId: "proj1",
    hasActiveProject: true,
    onOpenProjectDef: vi.fn(),
    onOpenSourceDef: vi.fn(),
    onOpenProject: vi.fn(),
    onAddSourceFolder: vi.fn(),
    searchInputRef: {
      current: null,
    } as React.RefObject<HTMLInputElement | null>,
    ...overrides,
  };
}

beforeEach(() => {
  invokeMock.mockImplementation((command) => {
    if (command === "get_def_index_facets") return Promise.resolve(facets);
    if (command === "search_defs") return Promise.resolve([]);
    if (command === "rebuild_def_index")
      return Promise.resolve({
        indexedDefs: 0,
        projectDefs: 0,
        sourceDefs: 0,
        errors: 0,
        builtAtUnixMs: 0,
      });
    return Promise.resolve(null);
  });
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("DefSearchPanel - no project", () => {
  it("shows open-project and add-source-folder actions when no project is active", () => {
    render(
      <DefSearchPanel
        {...defaultProps({ hasActiveProject: false, projectId: undefined })}
      />,
    );
    expect(screen.getByRole("button", { name: "Open Project" })).toBeDefined();
    expect(screen.getByText("Add Source Folder")).toBeDefined();
  });

  it("calls onOpenProject when Open Project is clicked", () => {
    const onOpenProject = vi.fn();
    render(
      <DefSearchPanel
        {...defaultProps({
          hasActiveProject: false,
          projectId: undefined,
          onOpenProject,
        })}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Open Project" }));
    expect(onOpenProject).toHaveBeenCalledOnce();
  });
});

describe("DefSearchPanel - results", () => {
  it("renders defName, label, defType, path, and project badge for a project result", async () => {
    invokeMock.mockImplementation((command) => {
      if (command === "get_def_index_facets") return Promise.resolve(facets);
      if (command === "search_defs") return Promise.resolve([projectResult]);
      return Promise.resolve(null);
    });
    render(<DefSearchPanel {...defaultProps()} />);
    await waitFor(() => screen.getByText("Steel"));
    expect(screen.getByText("steel")).toBeDefined();
    expect(screen.getByText("Project")).toBeDefined();
  });

  it("renders source badge and location name for a source result", async () => {
    invokeMock.mockImplementation((command) => {
      if (command === "get_def_index_facets") return Promise.resolve(facets);
      if (command === "search_defs") return Promise.resolve([sourceResult]);
      return Promise.resolve(null);
    });
    render(<DefSearchPanel {...defaultProps()} />);
    await waitFor(() => screen.getByText("Steel"));
    expect(screen.getByText("Read-only source")).toBeDefined();
    expect(screen.getByText("Core")).toBeDefined();
  });

  it("clicking a project result calls onOpenProjectDef with relativePath and nodeId", async () => {
    const onOpenProjectDef = vi.fn();
    invokeMock.mockImplementation((command) => {
      if (command === "get_def_index_facets") return Promise.resolve(facets);
      if (command === "search_defs") return Promise.resolve([projectResult]);
      return Promise.resolve(null);
    });
    render(<DefSearchPanel {...defaultProps({ onOpenProjectDef })} />);
    await waitFor(() => screen.getByText("Steel"));
    fireEvent.click(screen.getByTitle("Steel - Defs/Things/Steel.xml"));
    expect(onOpenProjectDef).toHaveBeenCalledWith("Defs/Things/Steel.xml", 42);
  });

  it("clicking a source result calls onOpenSourceDef and does not call onOpenProjectDef", async () => {
    const onOpenProjectDef = vi.fn();
    const onOpenSourceDef = vi.fn();
    invokeMock.mockImplementation((command) => {
      if (command === "get_def_index_facets") return Promise.resolve(facets);
      if (command === "search_defs") return Promise.resolve([sourceResult]);
      return Promise.resolve(null);
    });
    render(
      <DefSearchPanel
        {...defaultProps({ onOpenProjectDef, onOpenSourceDef })}
      />,
    );
    await waitFor(() => screen.getByText("Steel"));
    fireEvent.click(screen.getByTitle("Steel - Things/Resources/Steel.xml"));
    expect(onOpenProjectDef).not.toHaveBeenCalled();
    expect(onOpenSourceDef).toHaveBeenCalledWith(
      "core1",
      "Core",
      "Things/Resources/Steel.xml",
      undefined,
    );
    expect(screen.queryByText("Source Preview")).toBeNull();
  });
});

describe("DefSearchPanel - filters", () => {
  it("selecting a def type calls search_defs with that defType", async () => {
    render(<DefSearchPanel {...defaultProps()} />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "get_def_index_facets",
        expect.anything(),
      ),
    );
    // The facets load so we can select a type
    const select = screen.getByRole("combobox", { name: "Filter by Def type" });
    fireEvent.change(select, { target: { value: "ThingDef" } });
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "search_defs",
        expect.objectContaining({ defType: "ThingDef" }),
      ),
    );
  });

  it("unchecking include sources passes includeSources false to search_defs", async () => {
    render(<DefSearchPanel {...defaultProps()} />);
    const checkbox = screen.getByRole("checkbox");
    fireEvent.click(checkbox);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "search_defs",
        expect.objectContaining({ includeSources: false }),
      ),
    );
  });
});

describe("DefSearchPanel - rebuild", () => {
  it("rebuild button calls rebuild_def_index then reloads facets", async () => {
    render(<DefSearchPanel {...defaultProps()} />);
    fireEvent.click(screen.getByRole("button", { name: "Rebuild index" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "rebuild_def_index",
        expect.anything(),
      ),
    );
    await waitFor(() =>
      expect(
        invokeMock.mock.calls.filter((c) => c[0] === "get_def_index_facets")
          .length,
      ).toBeGreaterThan(1),
    );
  });
});
