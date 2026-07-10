import { detectLineEnding, normalizeLineEndings, applyLineEnding } from "./lineEndings";

describe("detectLineEnding", () => {
  it("detects CRLF when input contains \\r\\n", () => {
    expect(detectLineEnding("line1\r\nline2\r\n")).toBe("crlf");
  });

  it("detects LF for LF-only input", () => {
    expect(detectLineEnding("line1\nline2\n")).toBe("lf");
  });

  it("detects LF for empty string", () => {
    expect(detectLineEnding("")).toBe("lf");
  });

  it("detects CRLF even when only one occurrence is present", () => {
    expect(detectLineEnding("line1\nline2\r\nline3\n")).toBe("crlf");
  });
});

describe("normalizeLineEndings", () => {
  it("converts CRLF to LF", () => {
    expect(normalizeLineEndings("a\r\nb\r\n")).toBe("a\nb\n");
  });

  it("converts bare CR to LF", () => {
    expect(normalizeLineEndings("a\rb\r")).toBe("a\nb\n");
  });

  it("handles mixed CR, CRLF, and LF", () => {
    expect(normalizeLineEndings("a\r\nb\rc\n")).toBe("a\nb\nc\n");
  });

  it("leaves LF-only strings unchanged", () => {
    expect(normalizeLineEndings("a\nb\n")).toBe("a\nb\n");
  });
});

describe("applyLineEnding", () => {
  it("applies CRLF to LF-normalized text", () => {
    expect(applyLineEnding("a\nb\n", "crlf")).toBe("a\r\nb\r\n");
  });

  it("applies LF style (no-op for already-LF text)", () => {
    expect(applyLineEnding("a\nb\n", "lf")).toBe("a\nb\n");
  });

  it("does not add a trailing newline when none was present", () => {
    const result = applyLineEnding("a\nb", "crlf");
    expect(result).toBe("a\r\nb");
    expect(result.endsWith("\r\n")).toBe(false);
  });

  it("preserves a trailing newline that was present", () => {
    const result = applyLineEnding("a\nb\n", "crlf");
    expect(result.endsWith("\r\n")).toBe(true);
  });

  it("normalizes mixed line endings before applying style", () => {
    expect(applyLineEnding("a\r\nb\rc\n", "lf")).toBe("a\nb\nc\n");
    expect(applyLineEnding("a\r\nb\rc\n", "crlf")).toBe("a\r\nb\r\nc\r\n");
  });

  it("round-trips: apply CRLF then apply LF returns original LF text", () => {
    const original = "x\ny\nz\n";
    const crlf = applyLineEnding(original, "crlf");
    expect(applyLineEnding(crlf, "lf")).toBe(original);
  });
});
