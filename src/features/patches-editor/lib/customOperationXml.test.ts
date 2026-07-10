import { describe, expect, it } from "vitest";
import {
  buildCustomOperationXml,
  escapeAttr,
  escapeText,
  extractOperationForSlot,
  wrapAsPatchFileXml,
  wrapOperationForSlot,
} from "./customOperationXml";
import type { PatchOperationMetadata } from "../../schema-catalog/types";
import type { PatchFile, PatchOperationNode } from "../types/patchFile";

function metadata(overrides: Partial<PatchOperationMetadata> = {}): PatchOperationMetadata {
  return {
    className: "MyMod.PatchOperationAddOrReplace",
    fieldOrder: ["xpath", "value"],
    fields: {
      xpath: {
        type: { kind: "string" },
        required: true,
        examples: [],
        repeatable: false,
        xml: "element",
        flags: false,
        role: "xpath",
      },
      value: {
        type: { kind: "object" },
        required: true,
        examples: [],
        repeatable: false,
        xml: "object",
        flags: false,
        role: "xmlValue",
      },
    },
    preview: { kind: "unsupported" },
    ...overrides,
  };
}

describe("escapeText / escapeAttr", () => {
  it("escapes & before other entities", () => {
    expect(escapeText("A & B < C > D")).toBe("A &amp; B &lt; C &gt; D");
  });

  it("escapes attribute values without touching >", () => {
    expect(escapeAttr('A & "B" < C > D')).toBe("A &amp; &quot;B&quot; &lt; C > D");
  });
});

describe("buildCustomOperationXml", () => {
  it("serializes text fields escaped and xml fields verbatim, in fieldOrder", () => {
    const xml = buildCustomOperationXml(
      metadata(),
      {
        xpath: { kind: "text", value: 'Defs/ThingDef[defName="A & B"]' },
        value: { kind: "xml", value: "<statBases><MoveSpeed>1</MoveSpeed></statBases>" },
      },
      [],
      "normal",
    );

    expect(xml).toContain('<Operation Class="MyMod.PatchOperationAddOrReplace">');
    expect(xml).toContain("<xpath>Defs/ThingDef[defName=\"A &amp; B\"]</xpath>");
    expect(xml).toContain("<value><statBases><MoveSpeed>1</MoveSpeed></statBases></value>");
    expect(xml.indexOf("<xpath>")).toBeLessThan(xml.indexOf("<value>"));
    expect(xml.endsWith("</Operation>")).toBe(true);
  });

  it("omits fields absent from values", () => {
    const xml = buildCustomOperationXml(metadata(), { xpath: { kind: "text", value: "Defs/ThingDef" } }, [], "normal");
    expect(xml).toContain("<xpath>");
    expect(xml).not.toContain("<value>");
  });

  it("writes attribute-shaped fields on the opening tag, not as child elements", () => {
    const meta = metadata({
      fieldOrder: ["xpath", "MayRequireCustom"],
      fields: {
        xpath: metadata().fields.xpath,
        MayRequireCustom: {
          type: { kind: "string" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "attribute",
          flags: false,
        },
      },
    });
    const xml = buildCustomOperationXml(
      meta,
      { xpath: { kind: "text", value: "Defs/ThingDef" }, MayRequireCustom: { kind: "text", value: "Some & Co" } },
      [],
      "normal",
    );
    expect(xml).toContain('MayRequireCustom="Some &amp; Co"');
    expect(xml).not.toContain("<MayRequireCustom>");
  });

  it("writes a <success> element only when success is not normal", () => {
    const withDefault = buildCustomOperationXml(metadata(), {}, [], "normal");
    expect(withDefault).not.toContain("<success>");

    const withInvert = buildCustomOperationXml(metadata(), {}, [], "invert");
    expect(withInvert).toContain("<success>Invert</success>");
  });

  it("includes extra attributes (e.g. MayRequire) on the opening tag", () => {
    const xml = buildCustomOperationXml(metadata(), {}, [{ name: "MayRequire", value: "SomeMod" }], "normal");
    expect(xml).toContain('MayRequire="SomeMod"');
  });
});

describe("wrapAsPatchFileXml", () => {
  it("wraps operation XML in a synthetic <Patch> root", () => {
    const wrapped = wrapAsPatchFileXml('<Operation Class="X"></Operation>');
    expect(wrapped).toBe('<Patch>\n<Operation Class="X"></Operation>\n</Patch>\n');
  });
});

describe("buildCustomOperationXml with slot", () => {
  it("tags the fragment <li> for a sequence child", () => {
    const xml = buildCustomOperationXml(metadata(), {}, [], "normal", "sequenceChild");
    expect(xml.startsWith('<li Class="MyMod.PatchOperationAddOrReplace">')).toBe(true);
    expect(xml.endsWith("</li>")).toBe(true);
  });

  it("tags the fragment <match>/<nomatch> for those slots", () => {
    const match = buildCustomOperationXml(metadata(), {}, [], "normal", "match");
    expect(match.startsWith('<match Class="MyMod.PatchOperationAddOrReplace">')).toBe(true);
    expect(match.endsWith("</match>")).toBe(true);

    const nomatch = buildCustomOperationXml(metadata(), {}, [], "normal", "nomatch");
    expect(nomatch.startsWith('<nomatch Class="MyMod.PatchOperationAddOrReplace">')).toBe(true);
    expect(nomatch.endsWith("</nomatch>")).toBe(true);
  });
});

function unknownNode(id: number, rawXml: string): PatchOperationNode {
  return {
    id,
    className: "MyMod.PatchOperationAddOrReplace",
    success: "normal",
    attributes: [],
    kind: { type: "unknown", data: { rawXml } },
    span: null,
  };
}

function patchFileWith(operations: PatchOperationNode[]): PatchFile {
  return { relativePath: "", xmlDeclaration: null, operations, diagnostics: [], hadFatalParseError: false };
}

describe("wrapOperationForSlot / extractOperationForSlot", () => {
  it("round-trips a top-level fragment", () => {
    const fragment = '<Operation Class="X"></Operation>';
    expect(wrapOperationForSlot(fragment, "top")).toBe(wrapAsPatchFileXml(fragment));
    const file = patchFileWith([unknownNode(0, fragment)]);
    expect(extractOperationForSlot(file, "top")).toBe(file.operations[0]);
  });

  it("wraps a sequenceChild fragment inside a synthetic PatchOperationSequence and extracts it back", () => {
    const fragment = '<li Class="MyMod.Custom"></li>';
    const wrapped = wrapOperationForSlot(fragment, "sequenceChild");
    expect(wrapped).toContain('<Operation Class="PatchOperationSequence">');
    expect(wrapped).toContain("<operations>");
    expect(wrapped).toContain(fragment);

    const child = unknownNode(1, fragment);
    const file = patchFileWith([
      {
        id: 0,
        className: "PatchOperationSequence",
        success: "normal",
        attributes: [],
        kind: { type: "sequence", data: [child] },
        span: null,
      },
    ]);
    expect(extractOperationForSlot(file, "sequenceChild")).toBe(child);
  });

  it("wraps match/nomatch fragments inside a synthetic PatchOperationFindMod and extracts them back", () => {
    const fragment = '<match Class="MyMod.Custom"></match>';
    const wrapped = wrapOperationForSlot(fragment, "match");
    expect(wrapped).toContain('<Operation Class="PatchOperationFindMod">');
    expect(wrapped).toContain("<mods>");
    expect(wrapped).toContain(fragment);

    const matchOp = unknownNode(1, fragment);
    const file = patchFileWith([
      {
        id: 0,
        className: "PatchOperationFindMod",
        success: "normal",
        attributes: [],
        kind: { type: "findMod", data: { mods: [], matchOp, nomatchOp: null } },
        span: null,
      },
    ]);
    expect(extractOperationForSlot(file, "match")).toBe(matchOp);
    expect(extractOperationForSlot(file, "nomatch")).toBeNull();
  });

  it("returns null when the parsed shape doesn't match the requested slot", () => {
    const file = patchFileWith([unknownNode(0, "<Operation></Operation>")]);
    expect(extractOperationForSlot(file, "sequenceChild")).toBeNull();
    expect(extractOperationForSlot(file, "match")).toBeNull();
    expect(extractOperationForSlot(patchFileWith([]), "top")).toBeNull();
  });
});
