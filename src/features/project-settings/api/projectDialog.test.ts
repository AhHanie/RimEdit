import { open } from "@tauri-apps/plugin-dialog";
import { upsertLocation } from "./projectSettings";
import { pickSourceFolder } from "./projectDialog";
import type { ProjectSettings, RegisteredLocation } from "../types";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("./projectSettings", () => ({
  upsertLocation: vi.fn(),
}));

const openMock = vi.mocked(open);
const upsertMock = vi.mocked(upsertLocation);

function makeLocation(overrides: Partial<RegisteredLocation> = {}): RegisteredLocation {
  return {
    id: "loc-1",
    displayName: "Test",
    rootPath: "/test/path",
    kind: "source",
    sourceType: "folder",
    readOnly: true,
    createdAt: "",
    updatedAt: "",
    ...overrides,
  };
}

function makeSettings(locations: RegisteredLocation[] = []): ProjectSettings {
  return { schemaVersion: 3, gameVersion: "1.6", locale: "en", locations };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("pickSourceFolder", () => {
  it("returns null when the dialog is cancelled", async () => {
    openMock.mockResolvedValue(null);
    const result = await pickSourceFolder(null);
    expect(result).toBeNull();
    expect(upsertMock).not.toHaveBeenCalled();
  });

  it("calls upsertLocation with kind:source and sourceType:folder", async () => {
    openMock.mockResolvedValue("C:\\mods\\CoreMod");
    upsertMock.mockResolvedValue(
      makeSettings([makeLocation({ id: "src-1", rootPath: "C:/mods/CoreMod" })]),
    );

    await pickSourceFolder(makeSettings());

    expect(upsertMock).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "source", sourceType: "folder" }),
    );
  });

  it("resolves the location by canonical path match", async () => {
    openMock.mockResolvedValue("C:\\mods\\CoreMod");
    const added = makeLocation({ id: "src-1", rootPath: "C:/mods/CoreMod" });
    upsertMock.mockResolvedValue(makeSettings([added]));

    const result = await pickSourceFolder(makeSettings());

    expect(result?.locationId).toBe("src-1");
  });

  it("resolves by new source ID when canonical path differs from selected path", async () => {
    // Simulates a symlink or OS canonicalization that changes the stored path
    openMock.mockResolvedValue("/symlink/mod");
    const added = makeLocation({ id: "src-new", rootPath: "/real/mod" });
    upsertMock.mockResolvedValue(makeSettings([added]));

    const result = await pickSourceFolder(makeSettings()); // no existing sources

    expect(result?.locationId).toBe("src-new");
  });

  it("does not fall back to a pre-existing source when no new source was added", async () => {
    // Simulates a cross-kind collision: the path is already registered as a project.
    // upsert_location returns unchanged settings (no source added).
    openMock.mockResolvedValue("C:\\projects\\mod");
    const existingProject = makeLocation({
      id: "proj-1",
      rootPath: "C:\\projects\\mod",
      kind: "project",
      readOnly: false,
    });
    const unrelatedSource = makeLocation({ id: "src-existing", rootPath: "/other/source" });
    const before = makeSettings([existingProject, unrelatedSource]);
    upsertMock.mockResolvedValue(before); // unchanged settings returned

    const result = await pickSourceFolder(before);

    expect(result).toBeNull();
  });

  it("returns null when no source matches and no new source was added", async () => {
    openMock.mockResolvedValue("/some/path");
    const existingSource = makeLocation({ id: "src-old", rootPath: "/other/path" });
    const before = makeSettings([existingSource]);
    upsertMock.mockResolvedValue(before); // unchanged

    const result = await pickSourceFolder(before);

    expect(result).toBeNull();
  });
});
