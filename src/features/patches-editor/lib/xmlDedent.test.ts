import { describe, expect, it } from "vitest";
import { dedentXmlFragment } from "./xmlDedent";

describe("dedentXmlFragment", () => {
  it("leaves an already-clean single-line value untouched", () => {
    expect(dedentXmlFragment("<label>Wall</label>")).toBe("<label>Wall</label>");
  });

  it("strips common leading indentation captured verbatim from a source file", () => {
    const raw =
      '\n                        <li Class="aRandomKiwi.PPP.CompProperties_LocalWirelessPowerReceptor">\n' +
      "                            <compClass>aRandomKiwi.PPP.CompLocalWirelessPowerReceptor</compClass>\n" +
      "                        </li>\n                    ";

    expect(dedentXmlFragment(raw)).toBe(
      '<li Class="aRandomKiwi.PPP.CompProperties_LocalWirelessPowerReceptor">\n' +
        "    <compClass>aRandomKiwi.PPP.CompLocalWirelessPowerReceptor</compClass>\n" +
        "</li>",
    );
  });

  it("preserves relative indentation between nested lines", () => {
    const raw = "  <statBases>\n    <MaxHitPoints>300</MaxHitPoints>\n  </statBases>";
    expect(dedentXmlFragment(raw)).toBe("<statBases>\n  <MaxHitPoints>300</MaxHitPoints>\n</statBases>");
  });

  it("treats blank interior lines as empty rather than factoring them into the minimum indent", () => {
    const raw = "  <a>\n\n  <b />\n  </a>";
    expect(dedentXmlFragment(raw)).toBe("<a>\n\n<b />\n</a>");
  });

  it("returns an empty string for blank or empty input", () => {
    expect(dedentXmlFragment("")).toBe("");
    expect(dedentXmlFragment("   \n   \n")).toBe("");
  });
});
