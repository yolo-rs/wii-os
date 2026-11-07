#!/usr/bin/env bun
/**
 * Icon Generation Script using the Gemini 3.1 Pro Preview API (REST generateContent).
 * Generates SVG icons via text generation (SVG is XML text format).
 *
 * Model: gemini-3.1-pro-preview - best thinking, token efficiency, factual consistency
 *
 * Usage:
 *   bun scripts/icon/generate.ts --prompt "settings gear icon" --style outlined
 *   bun scripts/icon/generate.ts --prompt "shopping cart" --style filled --color "#6366F1"
 *   bun scripts/icon/generate.ts --name "dashboard" --category navigation --style duotone
 *   bun scripts/icon/generate.ts --prompt "cloud upload" --batch 4 --output-dir ./icons
 *   bun scripts/icon/generate.ts --prompt "user profile" --sizes "16,24,32,48"
 */

import { join } from "node:path";
import { parseArgs } from "node:util";
import { geminiText, loadEnv, runtime, timestamp, type GeminiResponse } from "../lib/providers.ts";

await loadEnv();

// ============ CONFIGURATION ============
export const config = { GEMINI_API_KEY: process.env.GEMINI_API_KEY as string | undefined };
export const MODEL = "gemini-3.1-pro-preview";

export const ICON_STYLES: Record<string, string> = {
  outlined: "outlined stroke icons, 2px stroke width, no fill, clean open paths",
  filled: "solid filled icons, no stroke, flat color fills, bold shapes",
  duotone: "duotone style with primary color at full opacity and secondary color at 30% opacity, layered shapes",
  thin: "thin line icons, 1px or 1.5px stroke width, delicate minimalist lines",
  bold: "bold thick line icons, 3px stroke width, heavy weight, impactful",
  rounded: "rounded icons with round line caps and joins, soft corners, friendly feel",
  sharp: "sharp angular icons, square line caps and mitered joins, precise edges",
  flat: "flat design icons, solid fills, no gradients or shadows, geometric simplicity",
  gradient: "linear or radial gradient fills, modern vibrant color transitions",
  glassmorphism: "glassmorphism style with semi-transparent fills, blur backdrop effect simulation, frosted glass",
  pixel: "pixel art style icons on a grid, retro 8-bit aesthetic, crisp edges",
  "hand-drawn": "hand-drawn sketch style, slightly irregular strokes, organic feel, imperfect lines",
  isometric: "isometric 3D projection, 30-degree angles, dimensional depth",
  glyph: "simple glyph style, single solid shape, minimal detail, pictogram",
  "animated-ready": "animated-ready SVG with named groups and IDs for CSS/JS animation targets",
};

export const ICON_CATEGORIES: Record<string, string> = {
  navigation: "arrows, menus, hamburger, chevrons, home, back, forward, breadcrumb",
  action: "edit, delete, save, download, upload, share, copy, paste, print, search",
  communication: "email, chat, phone, video call, notification, bell, message bubble",
  media: "play, pause, stop, skip, volume, microphone, camera, image, gallery",
  file: "document, folder, archive, attachment, cloud, database, storage",
  user: "person, group, avatar, profile, settings, lock, key, shield",
  commerce: "cart, bag, wallet, credit card, receipt, tag, gift, store",
  data: "chart, graph, analytics, dashboard, table, filter, sort, calendar",
  development: "code, terminal, bug, git, API, server, database, deploy",
  social: "heart, star, thumbs up, bookmark, flag, trophy, badge, crown",
  weather: "sun, moon, cloud, rain, snow, wind, thunder, temperature",
  map: "pin, location, compass, globe, route, directions, map marker",
};

const SVG_PROMPT_TEMPLATE = (p: { prompt: string; viewbox: number; styleInstructions: string; colorInstructions: string; sizeInstructions: string }) =>
  `Generate a clean, production-ready SVG icon.

Requirements:
- Output ONLY valid SVG code, nothing else
- ViewBox: "0 0 ${p.viewbox} ${p.viewbox}"
- Use currentColor for strokes/fills (inherits CSS color)
- No embedded fonts or text elements unless specifically requested
- No raster images or external references
- Optimized paths with minimal nodes
- Accessible: include <title> element with icon description
${p.styleInstructions}
${p.colorInstructions}
${p.sizeInstructions}

Icon to generate: ${p.prompt}

Output the SVG code only, wrapped in \`\`\`svg\`\`\` code block.`;

const SVG_BATCH_PROMPT_TEMPLATE = (p: { prompt: string; count: number; viewbox: number; styleInstructions: string; colorInstructions: string }) =>
  `Generate ${p.count} distinct SVG icon variations for: ${p.prompt}

Requirements for EACH icon:
- Output ONLY valid SVG code
- ViewBox: "0 0 ${p.viewbox} ${p.viewbox}"
- Use currentColor for strokes/fills (inherits CSS color)
- No embedded fonts, raster images, or external references
- Optimized paths with minimal nodes
- Include <title> element with icon description
${p.styleInstructions}
${p.colorInstructions}

Generate ${p.count} different visual interpretations. Output each SVG in a separate \`\`\`svg\`\`\` code block.
Label each variation (e.g., "Variation 1: [brief description]").`;

const BARE_SVG = /(<svg[^>]*>[\s\S]*?<\/svg>)/;

/** Extract SVG code blocks from a model response. */
export function extractSvgs(text: string): string[] {
  const all = (re: RegExp) => [...text.matchAll(re)].map((m) => m[1]!);
  let svgs = all(/```svg\s*\n([\s\S]*?)```/g);
  if (!svgs.length) svgs = all(/```xml\s*\n([\s\S]*?)```/g);
  if (!svgs.length) svgs = all(/(<svg[^>]*>[\s\S]*?<\/svg>)/g);

  const cleaned: string[] = [];
  for (const raw of svgs) {
    let svg = raw.trim();
    if (!svg.startsWith("<svg")) {
      const match = svg.match(BARE_SVG);
      if (!match) continue;
      svg = match[1]!;
    }
    cleaned.push(svg);
  }
  return cleaned;
}

/** Replace currentColor with a specific color if provided. */
export function applyColor(svgCode: string, color?: string | null): string {
  if (!color) return svgCode;
  let out = svgCode.replaceAll("currentColor", color);
  if (!out.includes(color)) out = out.replace("<svg", `<svg color="${color}"`);
  return out;
}

// Root <svg ...> opening tag; quoted attribute values may contain ">".
const SVG_ROOT_TAG = /<svg\b(?:[^>"']|"[^"]*"|'[^']*')*>/;
// One token of an opening tag: name[=value], or a stray quoted string.
const TAG_ATTR = /([^\s=/>"']+)(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>"']+))?|"[^"]*"|'[^']*'/g;

/** Set attr on a single <svg ...> opening tag, whatever quote style it uses. */
function setRootAttr(tag: string, attr: string, value: string | number): string {
  const re = new RegExp(TAG_ATTR.source, "g");
  re.lastIndex = "<svg".length;
  for (let m = re.exec(tag); m; m = re.exec(tag)) {
    if (m[1] === attr) return tag.slice(0, m.index) + `${attr}="${value}"` + tag.slice(m.index + m[0].length);
    if (m[0].length === 0) re.lastIndex++;
  }
  return tag.replace("<svg", `<svg ${attr}="${value}"`);
}

/** Set width/height on the root <svg> tag only, leaving the viewBox alone. */
export function applyViewboxSize(svgCode: string, size?: number | null): string {
  if (!size) return svgCode;
  const match = svgCode.match(SVG_ROOT_TAG);
  if (!match) return svgCode;
  let tag = setRootAttr(match[0], "width", size);
  tag = setRootAttr(tag, "height", size);
  return svgCode.slice(0, match.index!) + tag + svgCode.slice(match.index! + match[0].length);
}

function finishReason(response: GeminiResponse | null): string {
  return String(response?.candidates?.[0]?.finishReason ?? "");
}

function reportNoSvg(response: GeminiResponse | null, responseText: string, what = "SVG"): void {
  if (finishReason(response).includes("MAX_TOKENS")) {
    console.log("Error: output hit the token limit (finish_reason MAX_TOKENS) before the SVG was complete. Try a simpler prompt or style.");
  } else if (!responseText.trim()) {
    console.log("Error: model returned an empty response.");
  } else {
    console.log(`No valid ${what} generated. Model response:`);
    console.log(responseText.slice(0, 500));
  }
}

const slugify = (text: string) => text.toLowerCase().replace(/[^a-zA-Z0-9_-]/g, "_");
const firstWord = (text: string) => text.trim().split(/\s+/)[0] ?? "";

async function callModel(prompt: string, temperature: number): Promise<GeminiResponse> {
  return runtime.geminiGenerateContent(MODEL, config.GEMINI_API_KEY!, {
    contents: [{ role: "user", parts: [{ text: prompt }] }],
    generationConfig: { temperature, maxOutputTokens: 16384 },
  });
}

export interface IconOptions {
  prompt: string;
  style?: string | null;
  category?: string | null;
  name?: string | null;
  color?: string | null;
  size?: number;
  outputPath?: string | null;
  viewbox?: number;
}

/** Generate a single SVG icon; returns the saved path or null. */
export async function generateIcon(options: IconOptions): Promise<string | null> {
  const { prompt, style, category, name, color, size = 24, viewbox = 24 } = options;
  if (!config.GEMINI_API_KEY) {
    console.log("Error: GEMINI_API_KEY not set");
    console.log("Set it with: export GEMINI_API_KEY='your-key'");
    return null;
  }

  let iconPrompt = prompt;
  if (category && ICON_CATEGORIES[category]) iconPrompt = `${prompt} (category: ${ICON_CATEGORIES[category]})`;
  if (name) iconPrompt = `'${name}' icon: ${iconPrompt}`;

  const fullPrompt = SVG_PROMPT_TEMPLATE({
    prompt: iconPrompt,
    viewbox,
    styleInstructions: style && ICON_STYLES[style] ? `- Style: ${ICON_STYLES[style]}` : "",
    colorInstructions: color ? `- Use color: ${color} for primary elements, currentColor for secondary` : "- Use currentColor for all strokes and fills",
    sizeInstructions: `- Design for ${size}px display size, optimize detail level accordingly`,
  });

  console.log(`Generating icon with ${MODEL}...`);
  console.log(`Prompt: ${prompt}`);
  if (style) console.log(`Style: ${style}`);
  console.log();

  try {
    const response = await callModel(fullPrompt, 0.7);
    const responseText = geminiText(response);
    const svgs = extractSvgs(responseText);
    if (!svgs.length) {
      reportNoSvg(response, responseText);
      return null;
    }
    const svgCode = applyViewboxSize(applyColor(svgs[0]!, color), size);
    let outputPath = options.outputPath;
    if (!outputPath) {
      const slug = slugify(prompt ? name || firstWord(prompt) : "icon");
      outputPath = `${slug}${style ? `_${style}` : ""}_${timestamp()}.svg`;
    }
    await Bun.write(outputPath, svgCode);
    console.log(`Icon saved to: ${outputPath}`);
    return outputPath;
  } catch (error) {
    console.log(`Error generating icon: ${(error as Error).message}`);
    return null;
  }
}

/** Generate multiple icon variations in one request. */
export async function generateBatch(prompt: string, count: number, outputDir: string, style?: string | null, color?: string | null, viewbox = 24, name?: string | null): Promise<string[]> {
  if (!config.GEMINI_API_KEY) {
    console.log("Error: GEMINI_API_KEY not set");
    return [];
  }
  const fullPrompt = SVG_BATCH_PROMPT_TEMPLATE({
    prompt,
    count,
    viewbox,
    styleInstructions: style && ICON_STYLES[style] ? `- Style: ${ICON_STYLES[style]}` : "",
    colorInstructions: color ? `- Use color: ${color} for primary elements` : "- Use currentColor for all strokes and fills",
  });

  console.log(`\n${"=".repeat(60)}`);
  console.log(`  BATCH ICON GENERATION`);
  console.log(`  Model: ${MODEL}`);
  console.log(`  Prompt: ${prompt}`);
  console.log(`  Variants: ${count}`);
  console.log(`  Output: ${outputDir}`);
  console.log(`${"=".repeat(60)}\n`);

  try {
    const response = await callModel(fullPrompt, 0.9);
    const responseText = geminiText(response);
    const svgs = extractSvgs(responseText);
    if (!svgs.length) {
      reportNoSvg(response, responseText, "SVGs");
      return [];
    }
    const slug = name || slugify(firstWord(prompt));
    const styleSuffix = style ? `_${style}` : "";
    const selected = svgs.slice(0, count);
    const results: string[] = [];
    for (const [i, svg] of selected.entries()) {
      const filename = `${slug}${styleSuffix}_${String(i + 1).padStart(2, "0")}.svg`;
      const filepath = join(outputDir, filename);
      await Bun.write(filepath, applyColor(svg, color));
      results.push(filepath);
      console.log(`  [${i + 1}/${selected.length}] Saved: ${filename}`);
    }
    if (results.length < count && finishReason(response).includes("MAX_TOKENS")) {
      console.log("Warning: output hit the token limit (finish_reason MAX_TOKENS); the last icon was cut off. Try fewer variants or a simpler prompt.");
    }
    console.log(`\n${"=".repeat(60)}`);
    console.log(`  BATCH COMPLETE: ${results.length}/${count} icons generated`);
    console.log(`${"=".repeat(60)}\n`);
    return results;
  } catch (error) {
    console.log(`Error generating icons: ${(error as Error).message}`);
    return [];
  }
}

/** Generate the same icon at multiple sizes. */
export async function generateSizes(prompt: string, sizes: number[], style?: string | null, color?: string | null, outputDir = ".", name?: string | null): Promise<string[]> {
  const results: string[] = [];
  const slug = name || slugify(firstWord(prompt));
  const styleSuffix = style ? `_${style}` : "";
  for (const size of sizes) {
    console.log(`Generating ${size}px variant...`);
    const result = await generateIcon({ prompt, style, color, size, outputPath: join(outputDir, `${slug}${styleSuffix}_${size}px.svg`), viewbox: size });
    if (result) results.push(result);
    await runtime.sleep(1000);
  }
  return results;
}

/** CLI; returns the process exit status. */
export async function main(argv = process.argv.slice(2)): Promise<number> {
  const { values } = parseArgs({
    args: argv,
    options: {
      prompt: { type: "string", short: "p" },
      name: { type: "string", short: "n" },
      style: { type: "string", short: "s" },
      category: { type: "string", short: "c" },
      color: { type: "string" },
      size: { type: "string", default: "24" },
      viewbox: { type: "string", default: "24" },
      output: { type: "string", short: "o" },
      "output-dir": { type: "string" },
      batch: { type: "string" },
      sizes: { type: "string" },
      "list-styles": { type: "boolean" },
      "list-categories": { type: "boolean" },
    },
  });

  if (values.style && !ICON_STYLES[values.style]) {
    console.error(`generate.ts: error: argument --style/-s: invalid choice: '${values.style}'`);
    return 2;
  }
  if (values.category && !ICON_CATEGORIES[values.category]) {
    console.error(`generate.ts: error: argument --category/-c: invalid choice: '${values.category}'`);
    return 2;
  }
  if (values["list-styles"]) {
    console.log("Available icon styles:");
    for (const [style, desc] of Object.entries(ICON_STYLES)) console.log(`  ${style}: ${desc.slice(0, 70)}...`);
    return 0;
  }
  if (values["list-categories"]) {
    console.log("Available icon categories:");
    for (const [cat, desc] of Object.entries(ICON_CATEGORIES)) console.log(`  ${cat}: ${desc}`);
    return 0;
  }
  if (!values.prompt && !values.name) {
    console.error("generate.ts: error: Either --prompt or --name is required");
    return 2;
  }

  const prompt = (values.prompt || values.name)!;
  const viewbox = Number.parseInt(String(values.viewbox), 10);
  let generated: string | string[] | null;
  if (values.sizes) {
    const sizes = values.sizes.split(",").map((s) => Number.parseInt(s.trim(), 10));
    generated = await generateSizes(prompt, sizes, values.style, values.color, values["output-dir"] || "./icons", values.name);
  } else if (values.batch && Number.parseInt(values.batch, 10)) {
    generated = await generateBatch(prompt, Number.parseInt(values.batch, 10), values["output-dir"] || "./icons", values.style, values.color, viewbox, values.name);
  } else {
    generated = await generateIcon({
      prompt,
      style: values.style,
      category: values.category,
      name: values.name,
      color: values.color,
      size: Number.parseInt(String(values.size), 10),
      outputPath: values.output,
      viewbox,
    });
  }
  // Exit 1 when nothing was generated; a partly generated batch or size set exits 0
  return generated && generated.length ? 0 : 1;
}

if (import.meta.main) process.exit(await main());
