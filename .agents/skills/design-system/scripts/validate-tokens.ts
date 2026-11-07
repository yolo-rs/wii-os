#!/usr/bin/env bun
/**
 * Validate token usage in codebase.
 * Finds hardcoded values that should use design tokens.
 *
 * Usage:
 *   bun scripts/validate-tokens.ts --dir src/
 *   bun scripts/validate-tokens.ts --dir src/ --fix
 */

import { readdir, stat } from "node:fs/promises";
import { extname, join, resolve } from "node:path";
import { parseArgs } from "node:util";

const HELP = `
Usage: bun scripts/validate-tokens.ts [options]

Options:
  -d, --dir <path>      Directory to scan (required)
  --fix                 Show suggested fixes (no auto-fix)
  -i, --ignore <dir>    Additional directories to ignore (repeatable)
  -h, --help            Show this help

Checks for:
  - Hardcoded hex colors (#RGB, #RRGGBB)
  - Hardcoded pixel values (except 0, 1px)
  - Hardcoded rem values in CSS
`;

export const patterns = {
  hexColor: {
    regex: /#([0-9A-Fa-f]{3}){1,2}\b/g,
    message: "Hardcoded hex color",
    suggestion: "Use var(--color-*) token",
  },
  rgbColor: {
    regex: /rgb\s*\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*\)/gi,
    message: "Hardcoded RGB color",
    suggestion: "Use var(--color-*) token",
  },
  pixelValue: {
    regex: /:\s*(\d{2,})px/g, // 2+ digit px values
    message: "Hardcoded pixel value",
    suggestion: "Use var(--space-*) or var(--radius-*) token",
  },
  remValue: {
    regex: /:\s*\d+\.?\d*rem(?![^{]*\$value)/g, // rem not in token definition
    message: "Hardcoded rem value",
    suggestion: "Use var(--space-*) or var(--font-size-*) token",
  },
} as const;

type PatternName = keyof typeof patterns;

export interface Violation {
  file: string;
  line: number;
  column: number;
  value: string;
  type: PatternName;
  message: string;
  suggestion: string;
  context: string;
}

const EXTENSIONS = [".css", ".scss", ".tsx", ".jsx", ".ts", ".js", ".vue", ".svelte"];
const SKIP_PATTERNS = [/\.min\.(css|js)$/, /tailwind\.config/, /globals\.css/, /tokens\.(css|json)/];
const DEFAULT_IGNORE = ["node_modules", ".git", "dist", "build", ".next"];

export async function getFiles(dir: string, ignore: string[], files: string[] = []): Promise<string[]> {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (!ignore.includes(entry.name)) await getFiles(fullPath, ignore, files);
    } else if (entry.isFile() && EXTENSIONS.includes(extname(entry.name))) {
      files.push(fullPath);
    }
  }
  return files;
}

export function shouldSkip(filePath: string): boolean {
  return SKIP_PATTERNS.some((pattern) => pattern.test(filePath));
}

export function scanContent(filePath: string, content: string): Violation[] {
  const violations: Violation[] = [];
  content.split("\n").forEach((line, index) => {
    const trimmed = line.trim();
    if (trimmed.startsWith("//") || trimmed.startsWith("/*")) return;
    for (const [name, pattern] of Object.entries(patterns) as Array<[PatternName, (typeof patterns)[PatternName]]>) {
      for (const match of line.match(pattern.regex) ?? []) {
        if (name === "hexColor" && ["#000", "#FFF", "#000000", "#FFFFFF"].includes(match.toUpperCase())) continue;
        violations.push({
          file: filePath,
          line: index + 1,
          column: line.indexOf(match) + 1,
          value: match,
          type: name,
          message: pattern.message,
          suggestion: pattern.suggestion,
          context: trimmed.substring(0, 80),
        });
      }
    }
  });
  return violations;
}

export function formatReport(violations: Violation[]): string {
  if (violations.length === 0) return "✅ No token violations found";

  let report = `⚠️  Found ${violations.length} potential token violations:\n\n`;
  const byFile = new Map<string, Violation[]>();
  for (const v of violations) byFile.set(v.file, [...(byFile.get(v.file) ?? []), v]);
  for (const [file, fileViolations] of byFile) {
    report += `📁 ${file}\n`;
    for (const v of fileViolations) {
      report += `   Line ${v.line}: ${v.message}\n`;
      report += `   Found: ${v.value}\n`;
      report += `   Suggestion: ${v.suggestion}\n`;
      report += `   Context: ${v.context}\n\n`;
    }
  }

  const byType = new Map<PatternName, number>();
  for (const v of violations) byType.set(v.type, (byType.get(v.type) ?? 0) + 1);
  report += `\n📊 Summary:\n`;
  for (const [type, count] of byType) report += `   ${patterns[type].message}: ${count}\n`;
  return report;
}

async function main(): Promise<void> {
  const { values } = parseArgs({
    args: process.argv.slice(2),
    options: {
      dir: { type: "string", short: "d" },
      fix: { type: "boolean" },
      ignore: { type: "string", short: "i", multiple: true },
      help: { type: "boolean", short: "h" },
    },
    strict: false,
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }
  if (typeof values.dir !== "string") {
    console.error("Error: --dir is required");
    process.exit(1);
  }

  const dirPath = resolve(process.cwd(), values.dir);
  const isDir = await stat(dirPath).then(
    (s) => s.isDirectory(),
    () => false,
  );
  if (!isDir) {
    console.error(`Error: Directory not found: ${dirPath}`);
    process.exit(1);
  }

  console.log(`Scanning ${dirPath} for token violations...\n`);
  const ignore = [...DEFAULT_IGNORE, ...((values.ignore as string[] | undefined) ?? [])];
  const allViolations: Violation[] = [];
  for (const file of await getFiles(dirPath, ignore)) {
    if (shouldSkip(file)) continue;
    allViolations.push(...scanContent(file, await Bun.file(file).text()));
  }

  console.log(formatReport(allViolations));
  if (allViolations.length > 0) process.exit(1);
}

if (import.meta.main) await main();
