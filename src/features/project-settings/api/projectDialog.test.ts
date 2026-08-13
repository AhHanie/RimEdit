import { open } from "@tauri-apps/plugin-dialog";
import { classifySourceFolder, upsertLocation } from "./projectSettings";
import { pickSourceFolder } from "./projectDialog";
import type { ProjectSettings, RegisteredLocation, SourceFolderClassification } from "../types";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("./projectSettings", () => ({
  upsertLocation: vi.fn(),
  classifySourceFolder: vi.fn(),
}));

const openMock = vi.mocked(open);
const upsertMock = vi.mocked(upsertLocation);
const classifyMock = vi.mocked(classifySourceFolder);

const notWorkshop: SourceFolderClassification = {
  suggestedSourceType: "folder",
  highConfidence: false,
  numericItemCount: 0,
};

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
  classifyMock.mockResolvedValue(notWorkshop);
});

describe("pickSourceFolder", () => {
  it("returns null when the dialog is cancelled", async () => {
    openMock.mockResolvedValue(null);
    const result = await pickSourceFolder(null);
    expect(result).toBeNull();
    expect(upsertMock).not.toHaveBeenCalled();
  });

  it("calls upsertLocation with kind:source and sourceType:folder for an ordinary mod folder", async () => {
    openMock.mockResolvedValue("C:\\mods\\CoreMod");
    upsertMock.mockResolvedValue(
      makeSettings([makeLocation({ id: "src-1", rootPath: "C:/mods/CoreMod" })]),
    );

    await pickSourceFolder(makeSettings());

    expect(classifyMock).toHaveBeenCalledWith("C:\\mods\\CoreMod");
    expect(upsertMock).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "source", sourceType: "folder" }),
    );
  });

  it("uses sourceType:steamWorkshop for a high-confidence Workshop collection root", async () => {
    openMock.mockResolvedValue("C:\\SteamLibrary\\steamapps\\workshop\\content\\294100");
    classifyMock.mockResolvedValue({
      suggestedSourceType: "steamWorkshop",
      highConfidence: true,
      numericItemCount: 42,
    });
    upsertMock.mockResolvedValue(
      makeSettings([
        makeLocation({
          id: "workshop-1",
          sourceType: "steamWorkshop",
          rootPath: "C:/SteamLibrary/steamapps/workshop/content/294100",
        }),
      ]),
    );

    const result = await pickSourceFolder(makeSettings());

    expect(upsertMock).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "source", sourceType: "steamWorkshop" }),
    );
    expect(result?.ambiguousWorkshopRoot).toBe(false);
  });

  it("keeps sourceType:folder and flags ambiguousWorkshopRoot for a low-confidence match", async () => {
    openMock.mockResolvedValue("C:\\mods\\WeirdFolder");
    classifyMock.mockResolvedValue({
      suggestedSourceType: "folder",
      highConfidence: false,
      numericItemCount: 1,
    });
    upsertMock.mockResolvedValue(
      makeSettings([makeLocation({ id: "src-1", rootPath: "C:/mods/WeirdFolder" })]),
    );

    const result = await pickSourceFolder(makeSettings());

    expect(upsertMock).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "source", sourceType: "folder" }),
    );
    expect(result?.ambiguousWorkshopRoot).toBe(true);
  });

  it("does not flag ambiguousWorkshopRoot for an ordinary mod folder with no numeric children", async () => {
    openMock.mockResolvedValue("C:\\mods\\CoreMod");
    upsertMock.mockResolvedValue(
      makeSettings([makeLocation({ id: "src-1", rootPath: "C:/mods/CoreMod" })]),
    );

    const result = await pickSourceFolder(makeSettings());

    expect(result?.ambiguousWorkshopRoot).toBe(false);
  });

  it("falls back to sourceType:folder when classification fails (e.g. no Tauri backend)", async () => {
    openMock.mockResolvedValue("C:\\mods\\CoreMod");
    classifyMock.mockRejectedValue(new Error("no backend"));
    upsertMock.mockResolvedValue(
      makeSettings([makeLocation({ id: "src-1", rootPath: "C:/mods/CoreMod" })]),
    );

    const result = await pickSourceFolder(makeSettings());

    expect(upsertMock).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "source", sourceType: "folder" }),
    );
    expect(result?.ambiguousWorkshopRoot).toBe(false);
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
