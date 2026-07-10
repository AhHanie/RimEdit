export type LineEnding = "lf" | "crlf";

export function detectLineEnding(text: string): LineEnding {
  return text.includes("\r\n") ? "crlf" : "lf";
}

export function normalizeLineEndings(text: string): string {
  return text.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
}

export function applyLineEnding(text: string, lineEnding: LineEnding): string {
  const normalized = normalizeLineEndings(text);
  return lineEnding === "crlf" ? normalized.replace(/\n/g, "\r\n") : normalized;
}
