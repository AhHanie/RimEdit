import { fireEvent, screen } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { AboutEditorPane } from "./AboutEditorPane";
import type { AboutMetadataView } from "../../../xml-editor/types/xmlDocument";

function makeAboutView(overrides: Partial<AboutMetadataView["fields"]> = {}): AboutMetadataView {
  return {
    rootNodeId: 1,
    fields: {
      packageId: { value: "foo.bar" },
      name: { value: "Foo Mod" },
      shortName: { value: null },
      author: { value: "Foo Author" },
      authors: { items: [], present: false },
      modIconPath: { value: null },
      modVersion: { value: null },
      url: { value: null },
      description: { value: null },
      steamAppId: { value: null },
      targetVersion: { value: null },
      supportedVersions: { items: ["1.6"], present: true },
      loadBefore: { items: [], present: false },
      loadAfter: { items: [], present: false },
      forceLoadBefore: { items: [], present: false },
      forceLoadAfter: { items: [], present: false },
      incompatibleWith: { items: [], present: false },
      modDependencies: [],
      descriptionsByVersion: [],
      modDependenciesByVersion: [],
      loadBeforeByVersion: [],
      loadAfterByVersion: [],
      incompatibleWithByVersion: [],
      ...overrides,
    },
    unknownChildren: [],
  };
}

describe("AboutEditorPane", () => {
  it("renders header and identity field values from the About view", () => {
    const applyFormEdit = vi.fn().mockResolvedValue("");
    render(<AboutEditorPane about={makeAboutView()} diagnostics={[]} readOnly={false} applyFormEdit={applyFormEdit} />);

    expect(screen.getByText("Foo Mod")).toBeTruthy();
    expect(screen.getByText("foo.bar")).toBeTruthy();
    expect(screen.getByDisplayValue("Foo Author")).toBeTruthy();
  });

  it("commits a scalar field edit as setChildElementText on blur", () => {
    const applyFormEdit = vi.fn().mockResolvedValue("");
    render(<AboutEditorPane about={makeAboutView()} diagnostics={[]} readOnly={false} applyFormEdit={applyFormEdit} />);

    const input = screen.getByDisplayValue("Foo Mod");
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.blur(input);

    expect(applyFormEdit).toHaveBeenCalledWith({
      type: "setChildElementText",
      parentNodeId: 1,
      childName: "name",
      value: "New Name",
    });
  });

  it("commits an empty scalar field as removeChildElement on blur", () => {
    const applyFormEdit = vi.fn().mockResolvedValue("");
    render(<AboutEditorPane about={makeAboutView()} diagnostics={[]} readOnly={false} applyFormEdit={applyFormEdit} />);

    const input = screen.getByDisplayValue("Foo Author");
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.blur(input);

    expect(applyFormEdit).toHaveBeenCalledWith({
      type: "removeChildElement",
      parentNodeId: 1,
      childName: "author",
    });
  });

  it("adding a supported version commits setListItems with the appended item", () => {
    const applyFormEdit = vi.fn().mockResolvedValue("");
    render(<AboutEditorPane about={makeAboutView()} diagnostics={[]} readOnly={false} applyFormEdit={applyFormEdit} />);

    const addInput = screen.getByPlaceholderText("1.6");
    fireEvent.change(addInput, { target: { value: "1.5" } });
    fireEvent.click(screen.getByLabelText("Add to Supported Versions"));

    expect(applyFormEdit).toHaveBeenCalledWith({
      type: "setListItems",
      parentNodeId: 1,
      childName: "supportedVersions",
      items: ["1.6", "1.5"],
    });
  });

  it("adding a dependency commits insertObjectListItem", async () => {
    const applyFormEdit = vi.fn().mockResolvedValue("");
    render(<AboutEditorPane about={makeAboutView()} diagnostics={[]} readOnly={false} applyFormEdit={applyFormEdit} />);

    fireEvent.click(screen.getByText("Add dependency"));
    fireEvent.change(screen.getByPlaceholderText("packageId"), { target: { value: "brrainz.harmony" } });
    fireEvent.change(screen.getByPlaceholderText("displayName"), { target: { value: "Harmony" } });
    fireEvent.click(screen.getByText("Add"));

    expect(applyFormEdit).toHaveBeenCalledWith(
      expect.objectContaining({
        type: "insertObjectListItem",
        parentNodeId: 1,
        listName: "modDependencies",
        initialChildFields: [
          { name: "packageId", value: "brrainz.harmony" },
          { name: "displayName", value: "Harmony" },
        ],
      }),
    );
  });

  it("removing a dependency commits removeObjectListItem", () => {
    const applyFormEdit = vi.fn().mockResolvedValue("");
    const about = makeAboutView({
      modDependencies: [
        {
          nodeId: 42,
          packageId: "brrainz.harmony",
          alternativePackageIds: [],
          displayName: "Harmony",
          downloadUrl: null,
          steamWorkshopUrl: null,
        },
      ],
    });
    render(<AboutEditorPane about={about} diagnostics={[]} readOnly={false} applyFormEdit={applyFormEdit} />);

    fireEvent.click(screen.getByLabelText("Remove dependency"));

    expect(applyFormEdit).toHaveBeenCalledWith({
      type: "removeObjectListItem",
      listItemNodeId: 42,
      pruneEmptyAncestors: true,
    });
  });

  it("editing a dependency's alternative package IDs commits setListItems directly on the li item", () => {
    const applyFormEdit = vi.fn().mockResolvedValue("");
    const about = makeAboutView({
      modDependencies: [
        {
          nodeId: 42,
          packageId: "brrainz.harmony",
          alternativePackageIds: ["old.id"],
          displayName: "Harmony",
          downloadUrl: null,
          steamWorkshopUrl: null,
        },
      ],
    });
    render(<AboutEditorPane about={about} diagnostics={[]} readOnly={false} applyFormEdit={applyFormEdit} />);

    const addInput = screen.getByPlaceholderText("old.package.id");
    fireEvent.change(addInput, { target: { value: "another.old.id" } });
    fireEvent.click(screen.getByLabelText("Add to Alternative package IDs"));

    // Not setNestedListItems -- that op requires a non-empty objectPath, but
    // alternativePackageIds is a direct child of the dependency <li> item.
    expect(applyFormEdit).toHaveBeenCalledWith({
      type: "setListItems",
      parentNodeId: 42,
      childName: "alternativePackageIds",
      items: ["old.id", "another.old.id"],
    });
  });

  it("does not commit edits and hides mutating controls when read-only", () => {
    const applyFormEdit = vi.fn().mockResolvedValue("");
    render(<AboutEditorPane about={makeAboutView()} diagnostics={[]} readOnly applyFormEdit={applyFormEdit} />);

    const input = screen.getByDisplayValue("Foo Mod") as HTMLInputElement;
    expect(input.readOnly).toBe(true);
    fireEvent.change(input, { target: { value: "Should not commit" } });
    fireEvent.blur(input);

    expect(applyFormEdit).not.toHaveBeenCalled();
    expect(screen.queryByText("Add dependency")).toBeNull();
  });

  it("registers a flush that resolves once the last pending edit settles", async () => {
    let resolveEdit: (value: string) => void = () => undefined;
    const applyFormEdit = vi.fn().mockReturnValue(
      new Promise<string>((resolve) => {
        resolveEdit = resolve;
      }),
    );
    const flushes: Array<() => Promise<void>> = [];
    render(
      <AboutEditorPane
        about={makeAboutView()}
        diagnostics={[]}
        readOnly={false}
        applyFormEdit={applyFormEdit}
        registerFlush={(flush) => flushes.push(flush)}
      />,
    );

    const input = screen.getByDisplayValue("Foo Mod");
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.blur(input);

    let flushed = false;
    const flushPromise = flushes[flushes.length - 1]().then(() => {
      flushed = true;
    });
    expect(flushed).toBe(false);
    resolveEdit("");
    await flushPromise;
    expect(flushed).toBe(true);
  });

  it("flush commits a still-focused, not-yet-blurred field draft instead of losing it", async () => {
    const applyFormEdit = vi.fn().mockResolvedValue("");
    const flushes: Array<() => Promise<void>> = [];
    render(
      <AboutEditorPane
        about={makeAboutView()}
        diagnostics={[]}
        readOnly={false}
        applyFormEdit={applyFormEdit}
        registerFlush={(flush) => flushes.push(flush)}
      />,
    );

    const input = screen.getByDisplayValue("Foo Mod");
    input.focus();
    fireEvent.change(input, { target: { value: "Typed But Not Blurred" } });

    // Simulates a save triggered (e.g. via keyboard shortcut) while the field is
    // still focused: no onBlur has fired yet, so nothing has been sent to
    // applyFormEdit -- the flush itself must force the draft to commit.
    expect(applyFormEdit).not.toHaveBeenCalled();
    await flushes[flushes.length - 1]();

    expect(applyFormEdit).toHaveBeenCalledWith({
      type: "setChildElementText",
      parentNodeId: 1,
      childName: "name",
      value: "Typed But Not Blurred",
    });
  });
});
