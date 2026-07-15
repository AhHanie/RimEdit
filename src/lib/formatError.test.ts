import { describe, it, expect } from "vitest";
import { formatError } from "./formatError";

describe("formatError", () => {
  it("renders a structured AppError rejection through the diagnostic renderer", () => {
    const rejection = {
      code: "location_not_found",
      message: "Location not found: loc-1",
      details: null,
      args: { path: "loc-1" },
    };
    expect(formatError(rejection)).toBe('The location "loc-1" could not be found.');
  });

  it("falls back to the raw message for a rejection with no code", () => {
    expect(formatError({ message: "boom" })).toBe("boom");
  });

  it("stringifies a plain thrown value", () => {
    expect(formatError("nope")).toBe("nope");
  });
});
