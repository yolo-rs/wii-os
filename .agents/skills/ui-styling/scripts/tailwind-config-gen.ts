#!/usr/bin/env bun
/**
 * Tailwind CSS v4 Theme Generator
 *
 * Generates a CSS-first Tailwind v4 entry stylesheet instead of a
 * tailwind.config.js: `@import "tailwindcss" source(...)`, `@source`,
 * `@plugin`, `@custom-variant dark` and an `@theme { ... }` block for
 * colors, fonts, spacing and breakpoints.
 *
 * Bun builds process the file with a Tailwind Bun plugin; the Aphrody
 * workspace uses `@aphrody/bun-plugin-tailwind`.
 *
 * Examples:
 *   bun scripts/tailwind-config-gen.ts --framework nextjs
 *   bun scripts/tailwind-config-gen.ts --colors brand:#3b82f6 accent:#8b5cf6
 *   bun scripts/tailwind-config-gen.ts --palette brand:#3b82f6
 *   bun scripts/tailwind-config-gen.ts --fonts display:"Playfair Display,serif"
 *   bun scripts/tailwind-config-gen.ts --spacing navbar:4rem --breakpoints 3xl:1920px
 *   bun scripts/tailwind-config-gen.ts --source ../../packages/ui/src --plugins
 */

import { stat } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";

export const FRAMEWORKS = ["react", "vue", "svelte", "nextjs"] as const;
export type Framework = (typeof FRAMEWORKS)[number];

/** Legacy JavaScript configs: Tailwind v4 ignores them unless an @config directive loads one. */
export const LEGACY_CONFIG_NAMES = ["tailwind.config.js", "tailwind.config.cjs", "tailwind.config.mjs", "tailwind.config.ts"];

// Valid npm package name: optional @scope/, then name with optional subpath. No quotes, parens or semicolons.
const VALID_PACKAGE_NAME = /^(@[a-zA-Z0-9_-]+\/)?[a-zA-Z0-9_-]+(\/[a-zA-Z0-9_.-]+)*$/;
// Theme token names: CSS identifiers without spaces or punctuation that could end the declaration.
const VALID_TOKEN_NAME = /^[a-zA-Z0-9_-]+$/;
// Packages that ship CSS and are imported rather than registered with @plugin.
const CSS_IMPORT_PACKAGES = new Set(["tw-animate-css"]);
const GENERIC_FONT_FAMILIES = new Set([
  "serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui", "ui-serif", "ui-sans-serif",
  "ui-monospace", "ui-rounded", "math", "emoji", "fangsong",
]);

const DEFAULT_ENTRY: Record<Framework, string> = {
  react: "src/index.css",
  vue: "src/style.css",
  svelte: "src/app.css",
  nextjs: "app/globals.css",
};

const SHADES: Array<[string, string]> = [
  ["50", "color-mix(in oklch, BASE 10%, white)"],
  ["100", "color-mix(in oklch, BASE 20%, white)"],
  ["200", "color-mix(in oklch, BASE 35%, white)"],
  ["300", "color-mix(in oklch, BASE 50%, white)"],
  ["400", "color-mix(in oklch, BASE 75%, white)"],
  ["500", "BASE"],
  ["600", "color-mix(in oklch, BASE 85%, black)"],
  ["700", "color-mix(in oklch, BASE 70%, black)"],
  ["800", "color-mix(in oklch, BASE 55%, black)"],
  ["900", "color-mix(in oklch, BASE 40%, black)"],
  ["950", "color-mix(in oklch, BASE 25%, black)"],
];

function assertCssValue(kind: string, value: string): void {
  if (/[;{}"\n\r]/.test(value)) throw new Error(`Invalid ${kind} value: ${JSON.stringify(value)}`);
}

function assertTokenName(kind: string, name: string): void {
  if (!VALID_TOKEN_NAME.test(name)) throw new Error(`Invalid ${kind} name: ${JSON.stringify(name)}`);
}

const fontStack = (families: string[]) =>
  families.map((f) => (GENERIC_FONT_FAMILIES.has(f) || /^var\(/.test(f) ? f : `"${f}"`)).join(", ");

const toPosix = (p: string) => p.split(sep).join("/");

export class TailwindThemeGenerator {
  readonly outputPath: string;
  readonly projectRoot: string;
  readonly theme: {
    colors: Record<string, string>;
    fonts: Record<string, string[]>;
    spacing: Record<string, string>;
    breakpoints: Record<string, string>;
  } = { colors: {}, fonts: {}, spacing: {}, breakpoints: {} };
  readonly plugins: string[] = [];
  readonly sources: string[] = [];

  constructor(
    readonly framework: Framework = "react",
    outputPath?: string | null,
    readonly force = false,
    projectRoot = process.cwd(),
  ) {
    this.projectRoot = resolve(projectRoot);
    this.outputPath = resolve(this.projectRoot, outputPath ?? DEFAULT_ENTRY[framework] ?? DEFAULT_ENTRY.react);
  }

  /** `source(...)` base: the project root, relative to the stylesheet. */
  sourceBase(): string {
    return toPosix(relative(dirname(this.outputPath), this.projectRoot)) || ".";
  }

  addColors(colors: Record<string, string>): void {
    for (const [name, value] of Object.entries(colors)) {
      assertTokenName("color", name);
      assertCssValue("color", value);
      this.theme.colors[name] = value;
    }
  }

  /** Add a 50-950 scale derived from one base color with color-mix (Baseline widely available). */
  addColorPalette(name: string, baseColor: string): void {
    assertTokenName("color", name);
    assertCssValue("color", baseColor);
    for (const [shade, recipe] of SHADES) this.theme.colors[`${name}-${shade}`] = recipe.replace("BASE", baseColor);
  }

  addFonts(fonts: Record<string, string[]>): void {
    for (const [name, families] of Object.entries(fonts)) {
      assertTokenName("font", name);
      for (const family of families) assertCssValue("font", family);
      this.theme.fonts[name] = families;
    }
  }

  addSpacing(spacing: Record<string, string>): void {
    for (const [name, value] of Object.entries(spacing)) {
      assertTokenName("spacing", name);
      assertCssValue("spacing", value);
      this.theme.spacing[name] = value;
    }
  }

  addBreakpoints(breakpoints: Record<string, string>): void {
    for (const [name, value] of Object.entries(breakpoints)) {
      assertTokenName("breakpoint", name);
      assertCssValue("breakpoint", value);
      this.theme.breakpoints[name] = value;
    }
  }

  /** Register plugin/CSS packages; names are validated so they cannot inject CSS (CWE-94 analogue). */
  addPlugins(plugins: string[]): void {
    for (const plugin of plugins) {
      if (!VALID_PACKAGE_NAME.test(plugin)) {
        throw new Error(`Invalid plugin name: ${JSON.stringify(plugin)}. Plugin names must be valid npm package names (e.g. '@tailwindcss/typography').`);
      }
      if (!this.plugins.includes(plugin)) this.plugins.push(plugin);
    }
  }

  /** Extra content sources outside the automatic detection (e.g. a workspace UI package). */
  addSources(paths: string[]): void {
    for (const path of paths) {
      assertCssValue("@source", path);
      if (!this.sources.includes(path)) this.sources.push(path);
    }
  }

  recommendPlugins(): string[] {
    const recommendations = ["tw-animate-css"];
    if (this.framework === "nextjs") recommendations.push("@tailwindcss/typography");
    return recommendations;
  }

  validate(): [boolean, string] {
    const { colors, fonts, spacing, breakpoints } = this.theme;
    const empty = [colors, fonts, spacing, breakpoints].every((group) => Object.keys(group).length === 0);
    return empty ? [true, "Warning: No theme tokens defined"] : [true, "Configuration valid"];
  }

  generate(): string {
    const lines = [
      "/* Tailwind CSS v4 entry - generated by ui-styling/scripts/tailwind-config-gen.ts",
      "   CSS-first configuration: tokens live in @theme, sources in source()/@source.",
      "   Bun builds: process this file with @aphrody/bun-plugin-tailwind (Aphrody workspace). */",
      "",
      `@import "tailwindcss" source("${this.sourceBase()}");`,
    ];
    for (const plugin of this.plugins) if (CSS_IMPORT_PACKAGES.has(plugin)) lines.push(`@import "${plugin}";`);
    for (const plugin of this.plugins) if (!CSS_IMPORT_PACKAGES.has(plugin)) lines.push(`@plugin "${plugin}";`);
    for (const source of this.sources) lines.push(`@source "${source}";`);
    lines.push("", "@custom-variant dark (&:where(.dark, .dark *));");

    const tokens = [
      ...Object.entries(this.theme.colors).map(([n, v]) => `  --color-${n}: ${v};`),
      ...Object.entries(this.theme.fonts).map(([n, v]) => `  --font-${n}: ${fontStack(v)};`),
      ...Object.entries(this.theme.spacing).map(([n, v]) => `  --spacing-${n}: ${v};`),
      ...Object.entries(this.theme.breakpoints).map(([n, v]) => `  --breakpoint-${n}: ${v};`),
    ];
    if (tokens.length) lines.push("", "@theme {", ...tokens, "}");
    return lines.join("\n") + "\n";
  }

  private async exists(path: string): Promise<boolean> {
    return stat(path).then(
      () => true,
      () => false,
    );
  }

  async write(): Promise<[boolean, string]> {
    try {
      if (!this.force) {
        if (await this.exists(this.outputPath)) {
          return [false, `Tailwind entry already exists: ${this.outputPath}. Refusing to overwrite it; re-run with --force only if this is intentional.`];
        }
        const legacy: string[] = [];
        for (const name of LEGACY_CONFIG_NAMES) {
          const candidate = join(this.projectRoot, name);
          if (await this.exists(candidate)) legacy.push(candidate);
        }
        if (legacy.length) {
          return [
            false,
            `Tailwind configuration already exists: ${legacy.join(", ")}. Tailwind v4 reads CSS-first configuration and ignores it unless an @config directive loads it; migrate its theme into @theme, or re-run with --force only if a second entry is intentional.`,
          ];
        }
      }
      await Bun.write(this.outputPath, this.generate());
      return [true, `Configuration written to ${this.outputPath}`];
    } catch (error) {
      return [false, `Failed to write config: ${(error as Error).message}`];
    }
  }
}

function parsePairs(kind: string, specs: string[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const spec of specs) {
    const index = spec.indexOf(":");
    if (index < 0) throw new Error(`Invalid ${kind} spec: ${spec}`);
    out[spec.slice(0, index)] = spec.slice(index + 1);
  }
  return out;
}

/** argparse nargs="*" style: collect values after a flag until the next --flag. */
function collect(argv: string[]): { flags: Set<string>; lists: Map<string, string[]>; single: Map<string, string> } {
  const flags = new Set<string>();
  const lists = new Map<string, string[]>();
  const single = new Map<string, string>();
  const listFlags = new Set(["--colors", "--palette", "--fonts", "--spacing", "--breakpoints", "--source"]);
  const valueFlags = new Set(["--framework", "--output"]);
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]!;
    if (listFlags.has(arg)) {
      const values = lists.get(arg) ?? [];
      while (i + 1 < argv.length && !argv[i + 1]!.startsWith("--")) values.push(argv[++i]!);
      lists.set(arg, values);
    } else if (valueFlags.has(arg)) {
      if (i + 1 >= argv.length) throw new Error(`argument ${arg}: expected one argument`);
      single.set(arg, argv[++i]!);
    } else if (arg.startsWith("--")) {
      flags.add(arg);
    } else {
      throw new Error(`unrecognized arguments: ${arg}`);
    }
  }
  return { flags, lists, single };
}

const HELP = `usage: tailwind-config-gen.ts [--framework {react,vue,svelte,nextjs}] [--output OUTPUT]
                              [--colors NAME:VALUE ...] [--palette NAME:BASE ...] [--fonts TYPE:FAMILY ...]
                              [--spacing NAME:VALUE ...] [--breakpoints NAME:WIDTH ...] [--source PATH ...]
                              [--plugins] [--validate-only] [--force]

Generate a Tailwind CSS v4 CSS-first entry (@import "tailwindcss" source(), @source, @theme)`;

/** CLI; returns the process exit status. */
export async function main(argv = process.argv.slice(2), cwd = process.cwd()): Promise<number> {
  let parsed: ReturnType<typeof collect>;
  try {
    parsed = collect(argv);
  } catch (error) {
    console.error(`tailwind-config-gen.ts: error: ${(error as Error).message}`);
    return 2;
  }
  const { flags, lists, single } = parsed;
  if (flags.has("--help")) {
    console.log(HELP);
    return 0;
  }
  const framework = (single.get("--framework") ?? "react") as Framework;
  if (!FRAMEWORKS.includes(framework)) {
    console.error(`tailwind-config-gen.ts: error: argument --framework: invalid choice: '${framework}'`);
    return 2;
  }

  const generator = new TailwindThemeGenerator(framework, single.get("--output"), flags.has("--force"), cwd);
  try {
    if (lists.has("--colors")) generator.addColors(parsePairs("color", lists.get("--colors")!));
    for (const [name, base] of Object.entries(parsePairs("palette", lists.get("--palette") ?? []))) generator.addColorPalette(name, base);
    if (lists.has("--fonts")) {
      const fonts: Record<string, string[]> = {};
      for (const [name, family] of Object.entries(parsePairs("font", lists.get("--fonts")!))) {
        fonts[name] = family.split(",").map((f) => f.trim().replace(/^['"]+|['"]+$/g, ""));
      }
      generator.addFonts(fonts);
    }
    if (lists.has("--spacing")) generator.addSpacing(parsePairs("spacing", lists.get("--spacing")!));
    if (lists.has("--breakpoints")) generator.addBreakpoints(parsePairs("breakpoint", lists.get("--breakpoints")!));
    if (lists.has("--source")) generator.addSources(lists.get("--source")!);
    if (flags.has("--plugins")) {
      const recommended = generator.recommendPlugins();
      generator.addPlugins(recommended);
      console.log(`Added recommended plugins: ${recommended.join(", ")}`);
      console.log("\nInstall with:");
      console.log(`  bun add -d tailwindcss ${recommended.join(" ")}`);
    }
  } catch (error) {
    console.error((error as Error).message);
    return 1;
  }

  const [valid, message] = generator.validate();
  if (!valid) {
    console.error(`Validation failed: ${message}`);
    return 1;
  }
  if (message.startsWith("Warning")) console.log(message);

  if (flags.has("--validate-only")) {
    console.log("Configuration valid");
    console.log("\nGenerated config:");
    console.log(generator.generate());
    return 0;
  }

  const [success, writeMessage] = await generator.write();
  (success ? console.log : console.error)(writeMessage);
  return success ? 0 : 1;
}

if (import.meta.main) process.exit(await main());
