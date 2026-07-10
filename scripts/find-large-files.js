#!/usr/bin/env node
// Lists source files under src/ and src-tauri/ whose line count is >= a threshold.
// Usage: node scripts/find-large-files.js [threshold]

import { readdirSync, statSync, readFileSync } from "node:fs";
import { join, extname, relative } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(fileURLToPath(import.meta.url), "..", "..");

const threshold = Number(process.argv[2]) || 1000;

const targets = [
  { dir: join(repoRoot, "src"), extensions: [".ts", ".tsx"] },
  { dir: join(repoRoot, "src-tauri", "src"), extensions: [".rs"] },
];

const skipDirs = new Set(["node_modules", "target", "gen", "dist", ".git"]);

function walk(dir, extensions, results) {
  for (const entry of readdirSync(dir)) {
    if (skipDirs.has(entry)) continue;
    const fullPath = join(dir, entry);
    const stat = statSync(fullPath);
    if (stat.isDirectory()) {
      walk(fullPath, extensions, results);
    } else if (extensions.includes(extname(entry))) {
      const lineCount = readFileSync(fullPath, "utf8").split("\n").length;
      results.push({ path: fullPath, lineCount });
    }
  }
}

const results = [];
for (const { dir, extensions } of targets) {
  walk(dir, extensions, results);
}

const large = results
  .filter((file) => file.lineCount >= threshold)
  .sort((a, b) => b.lineCount - a.lineCount);

if (large.length === 0) {
  console.log(`No files with >= ${threshold} lines found.`);
} else {
  console.log(`Files with >= ${threshold} lines:\n`);
  for (const file of large) {
    console.log(`${file.lineCount.toString().padStart(6)}  ${relative(repoRoot, file.path)}`);
  }
}
