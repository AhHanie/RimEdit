import { describe, it, expect, afterEach } from "vitest";
import { mkdtempSync, writeFileSync, rmSync, readdirSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { findBareMessageOnlyDiagnostics } from "./check-diagnostic-args.mjs";

const repoRoot = join(fileURLToPath(import.meta.url), "..", "..", "..");
const srcRoot = join(repoRoot, "src-tauri", "src");

function listRustFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) {
      out.push(...listRustFiles(full));
    } else if (entry.endsWith(".rs")) {
      out.push(full);
    }
  }
  return out;
}

let tempDirs = [];

function writeFixture(contents) {
  const dir = mkdtempSync(join(tmpdir(), "diagnostic-args-check-"));
  tempDirs.push(dir);
  const file = join(dir, "fixture.rs");
  writeFileSync(file, contents, "utf8");
  return file;
}

afterEach(() => {
  for (const dir of tempDirs) {
    rmSync(dir, { recursive: true, force: true });
  }
  tempDirs = [];
});

describe("check-diagnostic-args", () => {
  it("finds no message-only diagnostic/error structs in the real src-tauri tree", () => {
    const violations = findBareMessageOnlyDiagnostics(listRustFiles(srcRoot));
    expect(violations).toEqual([]);
  });

  it("flags a struct with code/message but no args field", () => {
    const file = writeFixture(`
pub struct FakeDiagnostic {
    pub code: String,
    pub message: String,
}
`);
    const violations = findBareMessageOnlyDiagnostics([file]);
    expect(violations).toHaveLength(1);
    expect(violations[0].name).toBe("FakeDiagnostic");
  });

  it("does not flag a struct that already declares an args field", () => {
    const file = writeFixture(`
pub struct FakeDiagnostic {
    pub code: String,
    pub message: String,
    pub args: crate::diagnostics::DiagnosticArgs,
}
`);
    expect(findBareMessageOnlyDiagnostics([file])).toEqual([]);
  });

  it("does not flag the shared diagnostics mechanism structs themselves", () => {
    const file = writeFixture(`
pub struct DiagnosticRef {
    pub code: String,
}
`);
    expect(findBareMessageOnlyDiagnostics([file])).toEqual([]);
  });
});
