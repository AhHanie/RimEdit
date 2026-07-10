import { describe, it, expect } from "vitest";
import { parseColorValue } from "./FormFieldControl";

describe("parseColorValue", () => {
  it("parses integer RGB tuple", () => {
    expect(parseColorValue("(118, 49, 57)")).toBe("rgba(118, 49, 57, 1.000)");
  });

  it("parses float RGB tuple", () => {
    expect(parseColorValue("(0.1, 0.1, 0.1)")).toBe("rgba(26, 26, 26, 1.000)");
  });

  it("parses float RGBA tuple", () => {
    expect(parseColorValue("(0.68, 0.68, 0.68, 0.4)")).toBe("rgba(173, 173, 173, 0.400)");
  });

  it("parses integer RGBA tuple", () => {
    expect(parseColorValue("(118, 49, 57, 200)")).toBe("rgba(118, 49, 57, 0.784)");
  });

  it("returns null for a malformed string", () => {
    expect(parseColorValue("not a color")).toBeNull();
  });

  it("returns null for missing parens", () => {
    expect(parseColorValue("1, 1, 1")).toBeNull();
  });

  it("returns null for wrong component count", () => {
    expect(parseColorValue("(1, 1)")).toBeNull();
    expect(parseColorValue("(1, 1, 1, 1, 1)")).toBeNull();
  });

  it("returns null for non-numeric components", () => {
    expect(parseColorValue("(red, green, blue)")).toBeNull();
  });

  it("returns null for empty string", () => {
    expect(parseColorValue("")).toBeNull();
  });

  it("returns null for empty component from double comma", () => {
    expect(parseColorValue("(0,, 0)")).toBeNull();
  });
});
