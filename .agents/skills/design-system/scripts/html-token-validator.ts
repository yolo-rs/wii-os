#!/usr/bin/env bun
/**
 * HTML Design Token Validator
 * Ensures all HTML assets (slides, infographics, etc.) use design tokens.
 * Source of truth: assets/design-tokens.css
 *
 * Usage:
 *   bun scripts/html-token-validator.ts                     # Validate all HTML assets
 *   bun scripts/html-token-validator.ts --type slides       # Validate only slides
 *   bun scripts/html-token-validator.ts --type infographics # Validate only infographics
 *   bun scripts/html-token-validator.ts path/to/file.html   # Validate specific file
 *   bun scripts/html-token-validator.ts --colors            # Show CSS variables from tokens
 */

import { basename, join } from "node:path";
import { parseArgs } from "node:util";
import { findProjectRoot } from "./lib/project-root.ts";

const FORBIDDEN_PATTERNS: Array<[RegExp, string]> = [
  [/#[0-9A-Fa-f]{3,8}\b/g, "hex color"],
  [/rgb\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*\)/g, "rgb color"],
  [/rgba\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*,\s*[\d.]+\s*\)/g, "rgba color"],
  [/hsl\([^)]+\)/g, "hsl color"],
  [/font-family:\s*'[^v][^a][^r][^']*',/g, "hardcoded font"], // Exclude var()
  [/font-family:\s*"[^v][^a][^r][^"]*",/g, "hardcoded font"],
];

// Brand colors with transparency (rgba is a CSS necessity there).
const ALLOWED_RGBA_PATTERNS = [
  /^rgba\(\s*59\s*,\s*130\s*,\s*246/, // --color-primary (#3B82F6)
  /^rgba\(\s*245\s*,\s*158\s*,\s*11/, // --color-secondary (#F59E0B)
  /^rgba\(\s*16\s*,\s*185\s*,\s*129/, // --color-accent (#10B981)
  /^rgba\(\s*20\s*,\s*184\s*,\s*166/, // --color-accent alt (#14B8A6)
  /^rgba\(\s*0\s*,\s*0\s*,\s*0/, // black transparency
  /^rgba\(\s*255\s*,\s*255\s*,\s*255/, // white transparency
  /^rgba\(\s*15\s*,\s*23\s*,\s*42/, // --color-surface (#0F172A)
  /^rgba\(\s*7\s*,\s*11\s*,\s*20/, // --color-background (#070B14)
];

const ALLOWED_EXCEPTIONS = [
  "pexels.com",
  "unsplash.com",
  "youtube.com",
  "ytimg.com",
  "googlefonts",
  "fonts.googleapis.com",
  "fonts.gstatic.com",
];

export class ValidationResult {
  errors: string[] = [];
  warnings: string[] = [];
  passed = true;
  constructor(readonly filePath: string) {}
  addError(msg: string): void {
    this.errors.push(msg);
    this.passed = false;
  }
  addWarning(msg: string): void {
    this.warnings.push(msg);
  }
}

export async function loadCssVariables(tokensCssPath: string): Promise<Map<string, string>> {
  const variables = new Map<string, string>();
  const file = Bun.file(tokensCssPath);
  if (await file.exists()) {
    for (const match of (await file.text()).matchAll(/(--[\w-]+):\s*([^;]+);/g)) {
      variables.set(match[1]!, match[2]!.trim());
    }
  }
  return variables;
}

function isInsideBlock(content: string, pos: number, openTag: string, closeTag: string): boolean {
  const pre = content.slice(0, pos);
  return pre.lastIndexOf(openTag) > pre.lastIndexOf(closeTag);
}

function getContext(content: string, pos: number, chars = 100): string {
  return content.slice(Math.max(0, pos - chars), Math.min(content.length, pos + chars));
}

const escapeRegExp = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/**
 * Validate HTML content for design token compliance:
 * design-tokens.css import, no hardcoded colors/fonts in CSS (except
 * <script> blocks for Chart.js), and enough var(--token) usage.
 */
export function validateHtml(content: string, filePath: string, verbose = false): ValidationResult {
  const result = new ValidationResult(filePath);

  if (!content.includes("design-tokens.css")) result.addError("Missing design-tokens.css import");

  for (const [pattern, description] of FORBIDDEN_PATTERNS) {
    for (const match of content.matchAll(pattern)) {
      const matchText = match[0];
      const matchPos = match.index!;
      const context = getContext(content, matchPos);

      if (isInsideBlock(content, matchPos, "<script", "</script>")) {
        if (verbose) result.addWarning(`Allowed in <script>: ${matchText}`);
        continue;
      }
      if (ALLOWED_EXCEPTIONS.some((exc) => context.toLowerCase().includes(exc))) {
        if (verbose) result.addWarning(`Allowed external: ${matchText}`);
        continue;
      }
      if (description === "rgba color" && ALLOWED_RGBA_PATTERNS.some((p) => p.test(matchText))) {
        if (verbose) result.addWarning(`Allowed brand rgba: ${matchText}`);
        continue;
      }
      if (context.includes("var(") && context.includes(matchText)) {
        if (new RegExp(`var\\([^)]*${escapeRegExp(matchText)}[^)]*\\)`).test(context)) continue;
      }

      if (isInsideBlock(content, matchPos, "<style", "</style>")) {
        result.addError(`Hardcoded ${description} in <style>: ${matchText}`);
      } else if (context.includes('style="')) {
        result.addError(`Hardcoded ${description} in inline style: ${matchText}`);
      }
    }
  }

  const tokenPatterns = [/var\(--color-/g, /var\(--primitive-/g, /var\(--typography-/g, /var\(--card-/g, /var\(--button-/g];
  const tokenCount = tokenPatterns.reduce((sum, p) => sum + (content.match(p)?.length ?? 0), 0);
  if (tokenCount < 5) {
    result.addWarning(`Low token usage (${tokenCount} var() references). Consider using more design tokens.`);
  }
  return result;
}

export async function validateFile(filePath: string, verbose = false): Promise<ValidationResult> {
  const file = Bun.file(filePath);
  if (!(await file.exists())) {
    const result = new ValidationResult(filePath);
    result.addError("File not found");
    return result;
  }
  return validateHtml(await file.text(), filePath, verbose);
}

export async function validateDirectory(dirPath: string, verbose = false): Promise<ValidationResult[]> {
  let files: string[];
  try {
    files = [...new Bun.Glob("*.html").scanSync({ cwd: dirPath, onlyFiles: true })].sort();
  } catch {
    return [];
  }
  return Promise.all(files.map((name) => validateFile(join(dirPath, name), verbose)));
}

function printResult(result: ValidationResult, verbose = false): void {
  console.log(`  ${result.passed ? "✓" : "✗"} ${basename(result.filePath)}`);
  for (const error of result.errors.slice(0, 5)) console.log(`      ├─ ${error}`);
  if (result.errors.length > 5) console.log(`      └─ ... and ${result.errors.length - 5} more errors`);
  if (verbose) for (const warning of result.warnings.slice(0, 3)) console.log(`      [warn] ${warning}`);
}

function printSummary(allResults: Record<string, ValidationResult[]>): boolean {
  let totalFiles = 0;
  let totalPassed = 0;
  let totalErrors = 0;

  console.log("\n" + "=".repeat(60));
  console.log("HTML DESIGN TOKEN VALIDATION SUMMARY");
  console.log("=".repeat(60));

  for (const [assetType, results] of Object.entries(allResults)) {
    if (results.length === 0) continue;
    const passed = results.filter((r) => r.passed).length;
    const failed = results.length - passed;
    totalFiles += results.length;
    totalPassed += passed;
    totalErrors += results.reduce((sum, r) => sum + r.errors.length, 0);
    console.log(`\n${failed === 0 ? "✓" : "✗"} ${assetType.toUpperCase()}: ${passed}/${results.length} passed`);
    for (const result of results) if (!result.passed) printResult(result);
  }

  console.log("\n" + "-".repeat(60));
  if (totalErrors === 0) {
    console.log(`✓ ALL PASSED: ${totalPassed}/${totalFiles} files valid`);
  } else {
    console.log(`✗ FAILED: ${totalFiles - totalPassed}/${totalFiles} files have issues (${totalErrors} total errors)`);
  }
  console.log("-".repeat(60));
  return totalErrors === 0;
}

async function main(): Promise<void> {
  const { values, positionals } = parseArgs({
    args: process.argv.slice(2),
    allowPositionals: true,
    options: {
      type: { type: "string", short: "t", default: "all" },
      verbose: { type: "boolean", short: "v" },
      colors: { type: "boolean" },
      fix: { type: "boolean" },
    },
  });

  const type = String(values.type);
  if (!["slides", "infographics", "all"].includes(type)) {
    console.error(`error: argument -t/--type: invalid choice: '${type}' (choose from slides, infographics, all)`);
    process.exit(2);
  }

  const projectRoot = await findProjectRoot();
  const tokensCssPath = join(projectRoot, "assets", "design-tokens.css");
  const assetDirs: Record<string, string> = {
    slides: join(projectRoot, "assets", "designs", "slides"),
    infographics: join(projectRoot, "assets", "infographics"),
  };

  if (values.colors) {
    const variables = [...(await loadCssVariables(tokensCssPath))].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
    console.log("\nDesign Tokens (from design-tokens.css):");
    console.log("-".repeat(40));
    for (const [name, value] of variables.slice(0, 30)) console.log(`  ${name}: ${value}`);
    if (variables.length > 30) console.log(`  ... and ${variables.length - 30} more`);
    return;
  }

  const verbose = Boolean(values.verbose);
  const allResults: Record<string, ValidationResult[]> = {};
  if (positionals.length > 0) {
    allResults.specified = await Promise.all(positionals.map((p) => validateFile(p, verbose)));
  } else {
    for (const assetType of type === "all" ? Object.keys(assetDirs) : [type]) {
      allResults[assetType] = await validateDirectory(assetDirs[assetType]!, verbose);
    }
  }

  if (!printSummary(allResults)) process.exit(1);
}

if (import.meta.main) await main();
