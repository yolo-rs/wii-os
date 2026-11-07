#!/usr/bin/env bun
/**
 * embed-tokens.ts
 * Reads design-tokens.css and outputs embeddable inline CSS.
 * Use when generating standalone HTML files (infographics, slides, etc.)
 *
 * Usage:
 *   bun scripts/embed-tokens.ts           # Output full CSS
 *   bun scripts/embed-tokens.ts --minimal # Output only commonly used tokens
 *   bun scripts/embed-tokens.ts --style   # Wrap in <style> tags
 */

import { dirname, join } from "node:path";

/** Walk up until dirname stops changing (Windows roots are 'C:\'). */
export async function findProjectRoot(startDir: string): Promise<string | null> {
  let dir = startDir;
  for (;;) {
    if (await Bun.file(join(dir, "assets", "design-tokens.css")).exists()) return dir;
    const parent = dirname(dir);
    if (parent === dir) return null;
    dir = parent;
  }
}

// Minimal tokens commonly used in infographics/slides
export const MINIMAL_TOKENS = [
  "--primitive-spacing-",
  "--primitive-fontSize-",
  "--primitive-fontWeight-",
  "--primitive-lineHeight-",
  "--primitive-radius-",
  "--primitive-shadow-glow-",
  "--primitive-gradient-",
  "--primitive-duration-",
  "--color-primary",
  "--color-secondary",
  "--color-accent",
  "--color-background",
  "--color-surface",
  "--color-foreground",
  "--color-border",
  "--typography-font-",
  "--card-",
];

export function extractTokens(css: string, minimal = false): string {
  const rootBlocks = css.match(/:root\s*\{([^}]+)\}/g);
  if (!rootBlocks) return "";
  let allVars = rootBlocks.flatMap((block) => block.match(/--[\w-]+:\s*[^;]+;/g) ?? []);
  if (minimal) allVars = allVars.filter((v) => MINIMAL_TOKENS.some((token) => v.includes(token)));
  allVars = [...new Set(allVars)];
  return `:root {\n  ${allVars.join("\n  ")}\n}`;
}

async function main(): Promise<void> {
  const projectRoot = await findProjectRoot(process.cwd());
  if (!projectRoot) {
    console.error("Error: Could not find assets/design-tokens.css");
    process.exit(1);
  }
  const args = process.argv.slice(2);
  const minimal = args.includes("--minimal");
  const wrapStyle = args.includes("--style");
  try {
    const css = await Bun.file(join(projectRoot, "assets", "design-tokens.css")).text();
    const tokens = extractTokens(css, minimal);
    console.log(
      wrapStyle
        ? `<style>\n/* Design Tokens (embedded for standalone HTML) */\n${tokens}\n</style>`
        : `/* Design Tokens (embedded for standalone HTML) */\n${tokens}`,
    );
  } catch (err) {
    console.error(`Error reading tokens: ${(err as Error).message}`);
    process.exit(1);
  }
}

if (import.meta.main) await main();
