#!/usr/bin/env bun
/**
 * sync-brand-to-tokens.ts
 *
 * Syncs brand-guidelines.md colors → design-tokens.json → design-tokens.css
 *
 * Usage:
 *   bun scripts/sync-brand-to-tokens.ts
 *   bun scripts/sync-brand-to-tokens.ts --dry-run
 *   bun scripts/sync-brand-to-tokens.ts --force
 */

import { $ } from "bun";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";

const BRAND_GUIDELINES = "docs/brand-guidelines.md";
const DESIGN_TOKENS_JSON = "assets/design-tokens.json";
const DESIGN_TOKENS_CSS = "assets/design-tokens.css";
const CSS_TOKEN_SOURCES = [
  "src/index.css",
  "src/main.css",
  "src/app.css",
  "src/global.css",
  "src/globals.css",
  "src/styles/global.css",
  "src/styles/globals.css",
  "src/styles/index.css",
  "src/styles/main.css",
  "src/styles/tokens.css",
  "src/app/global.css",
  "src/app/globals.css",
  "app/global.css",
  "app/globals.css",
  "styles/global.css",
  "styles/globals.css",
  "styles/tokens.css",
];
// Directories shallow-scanned for any *.css that already declares tokens.
// A hardcoded filename list alone misses real projects by a single letter
// (e.g. styles/global.css vs styles/globals.css), which silently defeats the
// overwrite guard.
const CSS_SCAN_DIRS = ["src", "src/styles", "src/css", "src/assets/styles", "app", "app/styles", "styles", "css"];
const TAILWIND_CONFIGS = ["tailwind.config.js", "tailwind.config.cjs", "tailwind.config.mjs", "tailwind.config.ts"];
// Sibling sub-skill, resolved from this file's location so it works in every
// install context (plugin cache, project or --global CLI install).
const GENERATE_TOKENS_SCRIPT = resolve(import.meta.dir, "..", "..", "design-system", "scripts", "generate-tokens.ts");

interface ColorRole {
  name: string;
  base?: string;
  dark?: string;
  light?: string;
}

export interface BrandColorRoles {
  primary: ColorRole;
  secondary: ColorRole;
  accent: ColorRole;
}

type TokenTree = Record<string, any>;

async function isFile(path: string): Promise<boolean> {
  return Bun.file(path).exists();
}

function listCssFiles(absoluteDir: string): string[] {
  try {
    return [...new Bun.Glob("*").scanSync({ cwd: absoluteDir, onlyFiles: true })].filter((name) =>
      name.toLowerCase().endsWith(".css"),
    );
  } catch {
    return [];
  }
}

/** Find project files that already act as design-token sources. */
export async function findExistingTokenSources(projectRoot: string): Promise<string[]> {
  const sources = new Set<string>();

  const scanCssSource = async (absolutePath: string, visited = new Set<string>()): Promise<void> => {
    const normalizedPath = resolve(absolutePath);
    const relativePath = relative(projectRoot, normalizedPath);
    if (
      visited.has(normalizedPath) ||
      relativePath.startsWith("..") ||
      isAbsolute(relativePath) ||
      !(await isFile(normalizedPath))
    ) {
      return;
    }

    visited.add(normalizedPath);
    const content = await Bun.file(normalizedPath).text();
    const uncommented = content.replace(/\/\*[\s\S]*?\*\//g, "");
    const hasRootTokens = /:root\b[^{}]*\{[^}]*--[A-Za-z0-9_-]+\s*:/.test(uncommented);
    const hasTailwindTheme = /@theme(?:\s+[A-Za-z-]+)?\s*\{[^}]*--[A-Za-z0-9_-]+\s*:/.test(uncommented);
    if (hasRootTokens || hasTailwindTheme) {
      sources.add(relativePath.split(sep).join("/"));
    }

    const importPattern = /@import\s+(?:url\(\s*)?(['"])([^'"]+)\1\s*\)?[^;]*;/g;
    for (const match of uncommented.matchAll(importPattern)) {
      const importTarget = match[2]!.split(/[?#]/, 1)[0]!;
      let importedPath: string;
      if (importTarget.startsWith(".")) {
        importedPath = resolve(dirname(normalizedPath), importTarget);
      } else if (importTarget.startsWith("/")) {
        importedPath = resolve(projectRoot, `.${importTarget}`);
      } else {
        continue;
      }
      await scanCssSource(importedPath, visited);
    }
  };

  for (const relativePath of [DESIGN_TOKENS_JSON, DESIGN_TOKENS_CSS]) {
    if (await isFile(resolve(projectRoot, relativePath))) sources.add(relativePath);
  }

  const candidates = new Set(CSS_TOKEN_SOURCES);
  for (const dir of CSS_SCAN_DIRS) {
    for (const name of listCssFiles(resolve(projectRoot, dir))) candidates.add(`${dir}/${name}`);
  }
  for (const relativePath of candidates) {
    await scanCssSource(resolve(projectRoot, relativePath));
  }

  for (const relativePath of TAILWIND_CONFIGS) {
    const absolutePath = resolve(projectRoot, relativePath);
    if (!(await isFile(absolutePath))) continue;
    const content = await Bun.file(absolutePath).text();
    const hasInlineColors = /\btheme\s*:\s*\{[\s\S]*?\bcolors\s*:/.test(content);
    const hasPreset = /\bpresets\s*:/.test(content);
    const hasThemeSpread = /\btheme\s*:\s*\{[\s\S]*?\.\.\.[A-Za-z_$]/.test(content);
    if (hasInlineColors || hasPreset || hasThemeSpread) sources.add(relativePath);
  }

  return [...sources];
}

/** Extract color info from brand guidelines markdown. */
export function extractColorsFromMarkdown(content: string): BrandColorRoles {
  const colors: BrandColorRoles = {
    primary: { name: "primary" },
    secondary: { name: "secondary" },
    accent: { name: "accent" },
  };

  // Match a "| Label | #hex |" markdown table row. Bold around the label
  // (**Label**) is optional.
  const rowRe = /\|\s*\*{0,2}([^*|]+?)\*{0,2}\s*\|\s*#([A-Fa-f0-9]{6})\b/g;

  // 1) Quick Reference table — hex only.
  const quickRef: Record<keyof BrandColorRoles, RegExp> = {
    primary: /Primary Color\s*\|\s*#([A-Fa-f0-9]{6})/i,
    secondary: /Secondary Color\s*\|\s*#([A-Fa-f0-9]{6})/i,
    accent: /Accent Color\s*\|\s*#([A-Fa-f0-9]{6})/i,
  };
  for (const key of Object.keys(quickRef) as Array<keyof BrandColorRoles>) {
    const m = content.match(quickRef[key]);
    if (m) colors[key].base = `#${m[1]}`;
  }

  // 2) Dedicated "### <Role> Colors" tables — base/dark/light by row label keyword.
  const assignFromSection = (heading: string, target: ColorRole): void => {
    const section = content.match(new RegExp(`### ${heading}[\\s\\S]*?(?=\\n###|$)`, "i"));
    if (!section) return;
    for (const m of section[0].matchAll(rowRe)) {
      const label = m[1]!.trim().toLowerCase();
      const hex = `#${m[2]}`;
      if (label.includes("dark")) target.dark = hex;
      else if (label.includes("light")) target.light = hex;
      else if (!target.base) target.base = hex;
    }
  };
  assignFromSection("Primary Colors", colors.primary);
  assignFromSection("Secondary Colors", colors.secondary);
  assignFromSection("Accent Colors", colors.accent);

  // 3) Fallback: an accent swatch may live in another table.
  if (!colors.accent.base) {
    for (const m of content.matchAll(rowRe)) {
      if (m[1]!.trim().toLowerCase().includes("accent")) {
        colors.accent.base = `#${m[2]}`;
        break;
      }
    }
  }
  return colors;
}

/**
 * Adjust hex color brightness by blending each channel proportionally toward
 * white (percent > 0) or black (percent < 0), so dark brand colors keep a
 * graded scale instead of collapsing to #000000.
 */
export function adjustBrightness(hex: unknown, percent: number): string {
  if (typeof hex !== "string") return "#000000";
  const num = parseInt(hex.replace("#", ""), 16);
  const channel = (value: number): number => {
    const adjusted = percent >= 0 ? value + (255 - value) * percent : value * (1 + percent);
    return Math.min(255, Math.max(0, Math.round(adjusted)));
  };
  const r = channel((num >> 16) & 0xff);
  const g = channel((num >> 8) & 0xff);
  const b = channel(num & 0xff);
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, "0").toUpperCase()}`;
}

/** Generate a color scale from a base color. */
export function generateColorScale(baseHex: string, darkHex?: string, lightHex?: string): TokenTree {
  const color = (value: string) => ({ $value: value, $type: "color" });
  return {
    "50": color(lightHex || adjustBrightness(baseHex, 0.9)),
    "100": color(lightHex || adjustBrightness(baseHex, 0.8)),
    "200": color(adjustBrightness(baseHex, 0.6)),
    "300": color(adjustBrightness(baseHex, 0.4)),
    "400": color(adjustBrightness(baseHex, 0.2)),
    "500": color(baseHex),
    "600": color(darkHex || adjustBrightness(baseHex, -0.15)),
    "700": color(adjustBrightness(baseHex, -0.3)),
    "800": color(adjustBrightness(baseHex, -0.45)),
    "900": color(adjustBrightness(baseHex, -0.6)),
  };
}

/** Update design tokens JSON with the extracted brand colors. */
export function updateDesignTokens(tokens: TokenTree, colors: BrandColorRoles): TokenTree {
  tokens.brand = `ClaudeKit Marketing - ${colors.primary.name
    .split("-")
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ")}`;

  tokens.primitive = tokens.primitive || {};
  const primitiveColors: TokenTree = tokens.primitive.color || {};
  delete primitiveColors.coral;
  delete primitiveColors.purple;
  delete primitiveColors.mint;

  for (const role of ["primary", "secondary", "accent"] as const) {
    const c = colors[role];
    if (!c.base) {
      console.warn(`⚠️  No base hex found for ${role} color — skipping its token scale.`);
      continue;
    }
    primitiveColors[c.name] = generateColorScale(c.base, c.dark, c.light);
  }
  tokens.primitive.color = primitiveColors;

  if (tokens.semantic?.color) {
    const sem = tokens.semantic.color;
    const p = colors.primary.name;
    const s = colors.secondary.name;
    const a = colors.accent.name;
    const ref = (name: string, shade: string) => ({ $value: `{primitive.color.${name}.${shade}}`, $type: "color" });

    sem.primary = ref(p, "500");
    sem["primary-hover"] = ref(p, "600");
    sem["primary-active"] = ref(p, "700");
    sem["primary-light"] = ref(p, "400");
    sem["primary-lighter"] = ref(p, "100");
    sem["primary-dark"] = ref(p, "600");

    sem.secondary = ref(s, "500");
    sem["secondary-hover"] = ref(s, "600");
    sem["secondary-light"] = ref(s, "300");
    sem["secondary-dark"] = ref(s, "600");

    sem.accent = ref(a, "500");
    sem["accent-hover"] = ref(a, "600");
    sem["accent-light"] = ref(a, "300");

    sem.success = ref(a, "500");
    sem["success-light"] = ref(a, "300");
    sem.error = ref(p, "500");
    sem["error-light"] = ref(p, "300");
    sem.info = ref(s, "500");
    sem["info-light"] = ref(s, "300");
  }

  if (tokens.component?.button?.secondary && colors.primary.base) {
    tokens.component.button.secondary["bg-hover"] = { $value: `${colors.primary.base}1A`, $type: "color" };
  }
  return tokens;
}

async function main(): Promise<void> {
  const dryRun = process.argv.includes("--dry-run");
  const force = process.argv.includes("--force");
  const projectRoot = process.cwd();

  console.log("🔄 Syncing brand guidelines → design tokens\n");

  const guidelinesPath = resolve(projectRoot, BRAND_GUIDELINES);
  if (!(await isFile(guidelinesPath))) {
    console.error(`❌ Brand guidelines not found: ${guidelinesPath}`);
    process.exit(1);
  }
  const guidelinesContent = await Bun.file(guidelinesPath).text();

  const existingSources = await findExistingTokenSources(projectRoot);
  if (existingSources.length > 0 && !force) {
    const details = existingSources.map((source) => `   - ${source}`).join("\n");
    const message =
      `Existing design-token source${existingSources.length === 1 ? "" : "s"} detected:\n${details}\n` +
      "Refusing to create or replace token files. Review the detected source and re-run with --force only if replacement is intentional.";
    if (dryRun) {
      console.warn(`⚠️  ${message}\n`);
    } else {
      console.error(`❌ ${message}`);
      process.exit(1);
    }
  }

  const colors = extractColorsFromMarkdown(guidelinesContent);
  console.log("📊 Extracted colors:");
  console.log(`   Primary: ${colors.primary.name} (${colors.primary.base})`);
  console.log(`   Secondary: ${colors.secondary.name} (${colors.secondary.base})`);
  console.log(`   Accent: ${colors.accent.name} (${colors.accent.base})\n`);

  const tokensPath = resolve(projectRoot, DESIGN_TOKENS_JSON);
  const tokensFile = Bun.file(tokensPath);
  let tokens: TokenTree = (await tokensFile.exists()) ? await tokensFile.json() : {};
  tokens = updateDesignTokens(tokens, colors);

  if (dryRun) {
    console.log("📋 Would update design-tokens.json:");
    console.log(JSON.stringify(tokens.primitive.color, null, 2).slice(0, 500) + "...");
    console.log("\n⏭️  Dry run - no files changed");
    return;
  }

  // Bun.write creates missing parent directories.
  await Bun.write(tokensPath, JSON.stringify(tokens, null, 2));
  console.log(`✅ Updated: ${DESIGN_TOKENS_JSON}`);

  if (await isFile(GENERATE_TOKENS_SCRIPT)) {
    try {
      await $`${process.execPath} ${GENERATE_TOKENS_SCRIPT} --config ${DESIGN_TOKENS_JSON} -o ${DESIGN_TOKENS_CSS}`.cwd(
        projectRoot,
      );
      console.log(`✅ Regenerated: ${DESIGN_TOKENS_CSS}`);
    } catch (e) {
      console.error("⚠️  Failed to regenerate CSS:", (e as Error).message);
    }
  } else {
    console.warn(`⚠️  design-system sub-skill not found at ${GENERATE_TOKENS_SCRIPT}; ${DESIGN_TOKENS_CSS} not regenerated`);
  }

  console.log("\n✨ Brand sync complete!");
}

if (import.meta.main) await main();
