import { renderHook, waitFor, act } from "@testing-library/react";
import { useProjectSettings } from "./useProjectSettings";
import {
  getProjectSettings,
  updateProjectGameVersion,
  listInstalledSchemaGameVersions,
} from "../api/projectSettings";
import type { ProjectSettings } from "../types";

// Issue 09 finding 2: game-version discovery/selection must search every registered location's
// root as a candidate external-schema-pack root, exactly like `AppShell`'s `extraSchemaRoots`
// derivation and the backend's `schema_pack::schema_pack_roots`. Before this fix,
// `listInstalledSchemaGameVersions()`/`updateProjectGameVersion(version)` were always called
// with no roots at all, so a project whose only source of some game version was a mod-embedded
// schema pack could never see (or select) that version.
vi.mock("../api/projectSettings", () => ({
  getProjectSettings: vi.fn(),
  upsertLocation: vi.fn(),
  removeLocation: vi.fn(),
  setActiveProject: vi.fn(),
  updateLocation: vi.fn(),
  updateProjectGameVersion: vi.fn(),
  listInstalledSchemaGameVersions: vi.fn(),
}));

const getProjectSettingsMock = vi.mocked(getProjectSettings);
const updateProjectGameVersionMock = vi.mocked(updateProjectGameVersion);
const listInstalledSchemaGameVersionsMock = vi.mocked(listInstalledSchemaGameVersions);

function makeSettings(overrides: Partial<ProjectSettings> = {}): ProjectSettings {
  return {
    schemaVersion: 2,
    gameVersion: "1.6",
    locations: [
      {
        id: "loc1",
        displayName: "My Mod",
        rootPath: "C:\\Mods\\MyMod",
        kind: "project",
        sourceType: "folder",
        readOnly: false,
        createdAt: "2024-01-01T00:00:00Z",
        updatedAt: "2024-01-01T00:00:00Z",
      },
    ],
    activeProjectId: "loc1",
    ...overrides,
  };
}

describe("useProjectSettings - external schema root threading (issue 09)", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("fetches installed schema versions using every registered location's root path", async () => {
    getProjectSettingsMock.mockResolvedValue({ settings: makeSettings() });
    listInstalledSchemaGameVersionsMock.mockResolvedValue(["1.5", "1.6"]);

    const { result } = renderHook(() => useProjectSettings());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(listInstalledSchemaGameVersionsMock).toHaveBeenCalledWith(["C:\\Mods\\MyMod"]);
    expect(result.current.installedSchemaVersions).toEqual(["1.5", "1.6"]);
  });

  // Issue 09 review round 2, finding 2: initial mount with non-empty configured roots must
  // fetch installed schema versions exactly ONCE (the initial-load effect's own fetch), not
  // twice -- a naive "is this the reactive effect's first run" guard fires on the reactive
  // effect's pre-load run (roots still empty) instead of the run that actually corresponds to
  // settings/locations populating, letting a real second fetch slip through once settings load.
  it("fetches installed schema versions exactly once on initial mount with non-empty configured roots", async () => {
    getProjectSettingsMock.mockResolvedValue({ settings: makeSettings() });
    listInstalledSchemaGameVersionsMock.mockResolvedValue(["1.5", "1.6"]);

    const { result } = renderHook(() => useProjectSettings());
    await waitFor(() => expect(result.current.loading).toBe(false));
    // Flush any further pending microtasks/effects so a delayed second fetch would have fired.
    await act(async () => {
      await Promise.resolve();
    });

    expect(listInstalledSchemaGameVersionsMock).toHaveBeenCalledTimes(1);
  });

  it("passes the same registered-location roots to updateGameVersion", async () => {
    getProjectSettingsMock.mockResolvedValue({ settings: makeSettings() });
    listInstalledSchemaGameVersionsMock.mockResolvedValue(["1.6"]);
    updateProjectGameVersionMock.mockResolvedValue(makeSettings({ gameVersion: "1.5" }));

    const { result } = renderHook(() => useProjectSettings());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.updateGameVersion("1.5");
    });

    expect(updateProjectGameVersionMock).toHaveBeenCalledWith("1.5", ["C:\\Mods\\MyMod"]);
  });

  it("re-fetches installed schema versions when the registered-location roots actually change", async () => {
    getProjectSettingsMock.mockResolvedValue({ settings: makeSettings({ locations: [] }) });
    listInstalledSchemaGameVersionsMock.mockResolvedValue(["1.6"]);

    const { result, rerender } = renderHook(() => useProjectSettings());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(listInstalledSchemaGameVersionsMock).toHaveBeenCalledTimes(1);
    expect(listInstalledSchemaGameVersionsMock).toHaveBeenLastCalledWith([]);

    // Simulate a location being added later (e.g. via `addSourceFolder`) by resolving a new
    // `updateLocation`-style settings object through `replaceSettings` -- the public surface a
    // real caller (AppShell's `handleAddSourceFolder`) uses.
    listInstalledSchemaGameVersionsMock.mockResolvedValue(["1.5", "1.6"]);
    act(() => {
      result.current.replaceSettings(makeSettings());
    });
    rerender();

    await waitFor(() =>
      expect(listInstalledSchemaGameVersionsMock).toHaveBeenLastCalledWith(["C:\\Mods\\MyMod"]),
    );
    await waitFor(() =>
      expect(result.current.installedSchemaVersions).toEqual(["1.5", "1.6"]),
    );
  });
});
