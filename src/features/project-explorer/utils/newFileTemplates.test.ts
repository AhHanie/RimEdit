import { describe, it, expect } from "vitest";
import {
  DEFS_FILE_TEMPLATE,
  PATCH_FILE_TEMPLATE,
  ensureXmlExtension,
  nextAvailableFileName,
} from "./newFileTemplates";

describe("templates", () => {
  it("Defs template has a Defs root element", () => {
    expect(DEFS_FILE_TEMPLATE).toContain("<Defs>");
    expect(DEFS_FILE_TEMPLATE).toContain("</Defs>");
  });

  it("Patch template has a Patch root element", () => {
    expect(PATCH_FILE_TEMPLATE).toContain("<Patch>");
    expect(PATCH_FILE_TEMPLATE).toContain("</Patch>");
  });
});

describe("nextAvailableFileName", () => {
  it("returns the plain name when there is no collision", () => {
    expect(nextAvailableFileName("NewDefs", "xml", [])).toBe("NewDefs.xml");
    expect(nextAvailableFileName("NewDefs", "xml", ["Other.xml"])).toBe("NewDefs.xml");
  });

  it("appends the lowest available suffix on collision", () => {
    expect(nextAvailableFileName("NewDefs", "xml", ["NewDefs.xml"])).toBe("NewDefs1.xml");
    expect(nextAvailableFileName("NewDefs", "xml", ["NewDefs.xml", "NewDefs1.xml"])).toBe(
      "NewDefs2.xml",
    );
  });

  it("fills the first gap rather than always incrementing from the highest suffix", () => {
    expect(nextAvailableFileName("NewDefs", "xml", ["NewDefs.xml", "NewDefs2.xml"])).toBe(
      "NewDefs1.xml",
    );
  });

  it("compares names case-insensitively", () => {
    expect(nextAvailableFileName("NewDefs", "xml", ["newdefs.xml"])).toBe("NewDefs1.xml");
  });
});

describe("ensureXmlExtension", () => {
  it("appends .xml when missing", () => {
    expect(ensureXmlExtension("MyPatch")).toBe("MyPatch.xml");
  });

  it("leaves an existing .xml extension untouched", () => {
    expect(ensureXmlExtension("MyPatch.xml")).toBe("MyPatch.xml");
  });

  it("recognizes .xml case-insensitively", () => {
    expect(ensureXmlExtension("MyPatch.XML")).toBe("MyPatch.XML");
  });

  it("does not treat .xml appearing mid-name as the extension", () => {
    expect(ensureXmlExtension("my.xml.bak")).toBe("my.xml.bak.xml");
  });
});
