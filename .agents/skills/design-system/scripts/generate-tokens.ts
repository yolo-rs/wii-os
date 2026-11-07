#!/usr/bin/env bun
/**
 * Generate CSS variables from design tokens JSON.
 *
 * Usage:
 *   bun scripts/generate-tokens.ts --config tokens.json -o tokens.css
 *   bun scripts/generate-tokens.ts --config tokens.json --format tailwind     # Tailwind v4 @theme
 *   bun scripts/generate-tokens.ts --config tokens.json --format tailwind-v3  # legacy JS colors object
 */

import { resolve } from "node:path";
import { parseArgs } from "node:util";

type TokenTree = Record<string, any>;

const HELP = `
Usage: bun scripts/generate-tokens.ts [options]

Options:
  -c, --config <file>   Input JSON token file (required)
  -o, --output <file>   Output file (default: stdout)
  -f, --format <type>   Output format: css | tailwind | tailwind-v3 (default: css)
                        tailwind    = Tailwind v4 CSS-first "@theme inline" block
                        tailwind-v3 = legacy theme.extend.colors object
  -h, --help            Show this help
`;

/** Resolve token references like {primitive.color.blue.600}. */
export function resolveReference(value: unknown, tokens: TokenTree): unknown {
  if (typeof value !== "string" || !value.startsWith("{")) return value;
  let result: any = tokens;
  for (const key of value.slice(1, -1).split(".")) result = result?.[key];
  if (result?.$value) return resolveReference(result.$value, tokens);
  return result || value;
}

export function toCssVarName(path: string[]): string {
  return "--" + path.join("-").replace(/\./g, "-");
}

/** Flatten tokens into CSS variables. */
export function flattenTokens(
  obj: TokenTree,
  tokens: TokenTree,
  prefix: string[] = [],
  result: Record<string, unknown> = {},
): Record<string, unknown> {
  for (const [key, value] of Object.entries(obj)) {
    const currentPath = [...prefix, key];
    if (value && typeof value === "object") {
      if (value.$value !== undefined) {
        result[toCssVarName(currentPath)] = resolveReference(value.$value, tokens);
      } else {
        flattenTokens(value, tokens, currentPath, result);
      }
    }
  }
  return result;
}

const declarations = (vars: Record<string, unknown>): string =>
  Object.entries(vars)
    .map(([k, v]) => `  ${k}: ${v};`)
    .join("\n");

export function generateCSS(tokens: TokenTree): string {
  const primitive = flattenTokens(tokens.primitive || {}, tokens, ["primitive"]);
  const semantic = flattenTokens(tokens.semantic || {}, tokens, []);
  const component = flattenTokens(tokens.component || {}, tokens, []);
  const darkSemantic = flattenTokens(tokens.dark?.semantic || {}, tokens, []);

  let css = `/* Design Tokens - Auto-generated */
/* Do not edit directly - modify tokens.json instead */

/* === PRIMITIVES === */
:root {
${declarations(primitive)}
}

/* === SEMANTIC === */
:root {
${declarations(semantic)}
}

/* === COMPONENTS === */
:root {
${declarations(component)}
}
`;

  if (Object.keys(darkSemantic).length > 0) {
    css += `
/* === DARK MODE === */
.dark {
${declarations(darkSemantic)}
}
`;
  }
  return css;
}

function semanticColorVars(tokens: TokenTree): string[] {
  return Object.keys(flattenTokens(tokens.semantic || {}, tokens, [])).filter((key) => key.includes("color"));
}

/**
 * Tailwind v4 CSS-first output: expose the semantic color variables as theme
 * colors without copying values, so utilities such as `bg-primary` follow
 * light/dark switching of the generated tokens.
 */
export function generateTailwind(tokens: TokenTree): string {
  const lines = semanticColorVars(tokens).map((key) => `  --color-${key.replace(/^--color-/, "")}: var(${key});`);
  return `/* Tailwind v4 theme - Auto-generated */
/* Import after design-tokens.css in your Tailwind entry stylesheet:
   @import "tailwindcss" source("..");
   @import "./design-tokens.css";
   @import "./design-tokens.theme.css";
   (Bun workspaces: @aphrody/bun-plugin-tailwind processes this entry.) */

@theme inline {
${lines.join("\n")}
}
`;
}

/** Legacy Tailwind v3 colors object for theme.extend.colors. */
export function generateTailwindV3(tokens: TokenTree): string {
  const colors: Record<string, string> = {};
  for (const key of semanticColorVars(tokens)) {
    colors[key.replace("--color-", "").replace(/-/g, ".")] = `var(${key})`;
  }
  return `// Tailwind color config - Auto-generated
// Add to tailwind.config.ts theme.extend.colors

module.exports = {
  colors: ${JSON.stringify(colors, null, 2).replace(/"/g, "'")}
};
`;
}

async function main(): Promise<void> {
  const { values } = parseArgs({
    args: process.argv.slice(2),
    options: {
      config: { type: "string", short: "c" },
      output: { type: "string", short: "o" },
      format: { type: "string", short: "f", default: "css" },
      help: { type: "boolean", short: "h" },
    },
    strict: false,
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }
  if (typeof values.config !== "string") {
    console.error("Error: --config is required");
    process.exit(1);
  }

  const configPath = resolve(process.cwd(), values.config);
  const configFile = Bun.file(configPath);
  if (!(await configFile.exists())) {
    console.error(`Error: Config file not found: ${configPath}`);
    process.exit(1);
  }

  const tokens = (await configFile.json()) as TokenTree;
  const output =
    values.format === "tailwind"
      ? generateTailwind(tokens)
      : values.format === "tailwind-v3"
        ? generateTailwindV3(tokens)
        : generateCSS(tokens);

  if (typeof values.output === "string") {
    const outputPath = resolve(process.cwd(), values.output);
    await Bun.write(outputPath, output);
    console.log(`Generated: ${outputPath}`);
  } else {
    console.log(output);
  }
}

if (import.meta.main) await main();
