#!/usr/bin/env bun
/**
 * CIP Design Generator - Generate corporate identity mockups using Gemini Nano Banana.
 *
 * Uses Gemini native image generation (REST generateContent):
 * - gemini-2.5-flash-image: Fast generation, cost-effective (default)
 * - gemini-3-pro-image: Pro quality, 4K text rendering
 *
 * Set GEMINI_FLASH_IMAGE_MODEL or GEMINI_PRO_IMAGE_MODEL to use another Gemini model id.
 *
 * Image Editing (text-and-image-to-image):
 *   When --logo is provided, the logo file is sent with the prompt so the
 *   mockups use the actual logo instead of a generated one.
 *
 * Each mockup is saved with the extension of the image type Gemini returns
 * (.png, .jpg or .webp). Exit status is 1 when nothing was generated.
 *
 * Examples:
 *   bun scripts/cip/generate.ts --brand "TopGroup" --logo /path/to/logo.png --deliverable "business card"
 *   bun scripts/cip/generate.ts --brand "TopGroup" --logo /path/to/logo.png --industry "consulting" --set
 *   bun scripts/cip/generate.ts --brand "TechFlow" --deliverable "business card" --no-logo-prompt
 *   bun scripts/cip/generate.ts --brand "TechFlow" --logo logo.png --deliverable "business card" --model pro
 *   bun scripts/cip/generate.ts --brand "MyBrand" --logo logo.png --deliverable "vehicle" --output ./mockups --ratio 16:9
 */

import { extname, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { geminiInlineImage, IMAGE_EXTENSIONS, loadEnv, runtime, timestamp } from "../lib/providers.ts";
import { getCipBrief, search } from "./core.ts";

await loadEnv();

/** Model ids (read after loadEnv so .env files can override them). */
export const models = () => ({
  flash: process.env.GEMINI_FLASH_IMAGE_MODEL || "gemini-2.5-flash-image",
  pro: process.env.GEMINI_PRO_IMAGE_MODEL || "gemini-3-pro-image",
});
export const DEFAULT_MODEL = "flash";
export const DEFAULT_DELIVERABLES = ["business card", "letterhead", "office signage", "vehicle", "polo shirt"];

export interface LogoImage {
  path: string;
  mimeType: string;
  data: Uint8Array;
}

function sniffMime(bytes: Uint8Array, path: string): string | null {
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47) return "image/png";
  if (bytes[0] === 0xff && bytes[1] === 0xd8) return "image/jpeg";
  if (bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46) return "image/webp";
  const byExt: Record<string, string> = { ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp", ".gif": "image/gif" };
  return byExt[extname(path).toLowerCase()] ?? null;
}

/** Load the logo file for Gemini image editing (PNG, JPEG, WebP or GIF; transparency is kept). */
export async function loadLogoImage(logoPath: string): Promise<LogoImage | null> {
  const file = Bun.file(logoPath);
  if (!(await file.exists())) {
    console.log(`Error: Logo file not found: ${logoPath}`);
    return null;
  }
  try {
    const data = await file.bytes();
    const mimeType = sniffMime(data, logoPath);
    if (!mimeType) {
      console.log(`Error loading logo: unsupported image type ${extname(logoPath) || "(no extension)"}`);
      return null;
    }
    return { path: logoPath, mimeType, data };
  } catch (error) {
    console.log(`Error loading logo: ${(error as Error).message}`);
    return null;
  }
}

export interface PromptData {
  prompt: string;
  deliverable: string;
  style: string;
  brand: string;
  colors?: string;
  mockup_context: string;
  logo_placement?: string;
}

const first = async (query: string, domain: string) => (await search(query, domain, 1)).results?.[0] ?? {};

/** Build an optimized prompt for CIP mockup generation. */
export async function buildCipPrompt(
  deliverable: string,
  brandName: string,
  style?: string | null,
  industry?: string | null,
  mockup?: string | null,
  useLogoImage = false,
): Promise<PromptData> {
  const deliverableData = await first(deliverable, "deliverable");
  const styleData = style ? await first(style || "corporate minimal", "style") : {};
  const industryData = industry ? await first(industry || "technology", "industry") : {};

  let mockupContext = deliverableData["Mockup Context"] ?? "clean professional";
  if (mockup) {
    const mockupData = (await search(mockup, "mockup", 1)).results?.[0];
    if (mockupData) mockupContext = mockupData["Scene Description"] ?? mockupContext;
  }

  const deliverableName = deliverableData.Deliverable ?? deliverable;
  const description = deliverableData.Description ?? "";
  const logoPlacement = deliverableData["Logo Placement"] ?? "center";
  const styleName = styleData["Style Name"] ?? style ?? "corporate";
  const primaryColors = styleData["Primary Colors"] ?? industryData["Primary Colors"] ?? "#0F172A #FFFFFF";
  const typography = styleData.Typography ?? industryData.Typography ?? "clean sans-serif";
  const materials = styleData.Materials ?? "premium quality";
  const finishes = styleData.Finishes ?? "professional";
  const mood = styleData.Mood ?? industryData.Mood ?? "professional";

  const promptParts = useLogoImage
    ? [
        `Create a professional corporate identity mockup photograph of a ${deliverableName}`,
        "Use the EXACT logo from the provided image - do NOT modify or recreate the logo",
        "The logo MUST appear exactly as shown in the input image",
        `Place the logo on the ${deliverableName} at: ${logoPlacement}`,
        `Brand name: '${brandName}'`,
        description,
        `Design style: ${styleName}`,
        "Color scheme matching the logo colors",
        `Materials: ${materials} with ${finishes} finish`,
        `Setting: ${mockupContext}`,
        `Mood: ${mood}`,
        "Photorealistic product photography",
        "Soft natural lighting, professional studio quality",
        "8K resolution, sharp details",
      ]
    : [
        "Professional corporate identity mockup photograph",
        `showing ${deliverableName} for brand '${brandName}'`,
        description,
        `${styleName} design style`,
        `using colors ${primaryColors}`,
        `${typography} typography`,
        `logo placement: ${logoPlacement}`,
        `${materials} materials with ${finishes} finish`,
        `${mockupContext} setting`,
        `${mood} mood`,
        "photorealistic product photography",
        "soft natural lighting",
        "high quality professional shot",
        "8k resolution detailed",
      ];

  return {
    prompt: promptParts.filter(Boolean).join(", "),
    deliverable: deliverableName,
    style: styleName,
    brand: brandName,
    colors: primaryColors,
    mockup_context: mockupContext,
    logo_placement: logoPlacement,
  };
}

/** Generate one mockup with Gemini Nano Banana; returns the saved path or null. */
export async function generateWithNanoBanana(
  promptData: PromptData,
  outputDir?: string | null,
  modelKey = "flash",
  aspectRatio = "1:1",
  logoImage: LogoImage | null = null,
): Promise<string | null> {
  const apiKey = process.env.GEMINI_API_KEY || process.env.GOOGLE_API_KEY;
  if (!apiKey) {
    console.log("Error: GEMINI_API_KEY or GOOGLE_API_KEY not set");
    return null;
  }
  const available = models();
  const modelName = available[modelKey as keyof typeof available] ?? available[DEFAULT_MODEL];

  console.log(`\n🎨 Generating CIP mockup...`);
  console.log(`   Mode: ${logoImage ? "image-editing" : "text-to-image"}`);
  console.log(`   Deliverable: ${promptData.deliverable}`);
  console.log(`   Brand: ${promptData.brand}`);
  console.log(`   Style: ${promptData.style}`);
  console.log(`   Model: ${modelName}`);
  console.log(`   Context: ${promptData.mockup_context}`);
  if (logoImage) console.log(`   Logo: Using provided image (${logoImage.mimeType}, ${logoImage.data.length} bytes)`);

  try {
    const parts: unknown[] = [{ text: promptData.prompt }];
    if (logoImage) parts.push({ inlineData: { mimeType: logoImage.mimeType, data: Buffer.from(logoImage.data).toString("base64") } });
    const response = await runtime.geminiGenerateContent(modelName, apiKey, {
      contents: [{ role: "user", parts }],
      generationConfig: { responseModalities: ["IMAGE"], imageConfig: { aspectRatio } },
    });
    const image = geminiInlineImage(response);
    if (!image) {
      console.log("No image generated in response");
      return null;
    }
    const mime = image.mimeType.split(";")[0]!.trim().toLowerCase();
    const brandSlug = promptData.brand.toLowerCase().replaceAll(" ", "-");
    const deliverableSlug = promptData.deliverable.toLowerCase().replaceAll(" ", "-");
    const filepath = join(resolve(outputDir ?? process.cwd()), `${brandSlug}-${deliverableSlug}-${timestamp()}${IMAGE_EXTENSIONS[mime] ?? ".png"}`);
    await Bun.write(filepath, image.data);
    console.log(`\n✅ Generated: ${filepath}`);
    return filepath;
  } catch (error) {
    console.log(`Error generating image: ${(error as Error).message}`);
    return null;
  }
}

/** Generate a complete CIP set for a brand. */
export async function generateCipSet(
  brandName: string,
  industry: string,
  style?: string | null,
  deliverables?: string[] | null,
  outputDir?: string | null,
  modelKey = "flash",
  logoPath?: string | null,
  aspectRatio = "1:1",
): Promise<Array<{ deliverable: string; filepath: string; prompt: string }>> {
  let logoImage: LogoImage | null = null;
  if (logoPath) {
    logoImage = await loadLogoImage(logoPath);
    if (!logoImage) console.log("Warning: Could not load logo, falling back to text-to-image mode");
  }
  const brief = await getCipBrief(brandName, industry, style);
  const results: Array<{ deliverable: string; filepath: string; prompt: string }> = [];
  for (const deliverable of deliverables?.length ? deliverables : DEFAULT_DELIVERABLES) {
    const promptData = await buildCipPrompt(deliverable, brandName, brief.style["Style Name"], industry, null, logoImage !== null);
    const filepath = await generateWithNanoBanana(promptData, outputDir, modelKey, aspectRatio, logoImage);
    if (filepath) results.push({ deliverable, filepath, prompt: promptData.prompt });
  }
  return results;
}

/** Ask whether to continue without a logo: 'continue', 'generate' or 'exit'. */
export function checkLogoRequired(brandName: string, skipPrompt = false): "continue" | "generate" | "exit" {
  if (skipPrompt) return "continue";
  console.log(`\n⚠️  No logo image provided for '${brandName}'`);
  console.log("   Without a logo, AI will generate its own interpretation of the brand logo.");
  console.log("");
  console.log("   Options:");
  console.log("   1. Continue without logo (AI-generated logo interpretation)");
  console.log("   2. Generate a logo first using 'logo-design' skill");
  console.log("   3. Exit and provide a logo path with --logo");
  console.log("");
  const choice = (prompt("   Enter choice [1/2/3] (default: 1):") ?? "").trim();
  if (choice === "2") return "generate";
  if (choice === "3") return "exit";
  return "continue";
}

/** CLI; returns the process exit status. */
export async function main(argv = process.argv.slice(2)): Promise<number> {
  const { values } = parseArgs({
    args: argv,
    options: {
      brand: { type: "string", short: "b" },
      logo: { type: "string", short: "l" },
      deliverable: { type: "string", short: "d" },
      deliverables: { type: "string" },
      industry: { type: "string", short: "i", default: "technology" },
      style: { type: "string", short: "s" },
      mockup: { type: "string", short: "m" },
      set: { type: "boolean" },
      output: { type: "string", short: "o" },
      model: { type: "string", default: "flash" },
      ratio: { type: "string", default: "1:1" },
      "prompt-only": { type: "boolean" },
      json: { type: "boolean", short: "j" },
      "no-logo-prompt": { type: "boolean" },
    },
  });
  if (!values.brand) {
    console.error("generate.ts: error: the following arguments are required: --brand/-b");
    return 2;
  }
  if (!["flash", "pro"].includes(String(values.model))) {
    console.error(`generate.ts: error: argument --model: invalid choice: '${values.model}' (choose from flash, pro)`);
    return 2;
  }
  const brand = values.brand;
  const industry = String(values.industry);
  const model = String(values.model);
  const ratio = String(values.ratio);

  let logoImage: LogoImage | null = null;
  if (values.logo) {
    logoImage = await loadLogoImage(values.logo);
    if (!logoImage) {
      console.log("Error: Could not load logo image");
      return 1;
    }
  } else if (!values["prompt-only"]) {
    const action = checkLogoRequired(brand, Boolean(values["no-logo-prompt"]));
    if (action === "generate") {
      console.log("\n💡 To generate a logo, use the logo-design skill:");
      // Resolved from this file so the hint is correct from any cwd and install layout.
      const logoScript = join(import.meta.dir, "..", "logo", "generate.ts");
      console.log(`   bun "${logoScript}" --brand "${brand}" --industry "${industry}"`);
      console.log("\n   Then re-run this command with --logo <generated_logo.png>");
      return 0;
    }
    if (action === "exit") {
      console.log("\n   Provide logo with: --logo /path/to/your/logo.png");
      return 0;
    }
  }
  const useLogo = logoImage !== null;

  if (values.set || values.deliverables) {
    const deliverables = values.deliverables ? values.deliverables.split(",") : null;
    if (values["prompt-only"]) {
      const results: PromptData[] = [];
      for (const d of deliverables ?? DEFAULT_DELIVERABLES) {
        results.push(await buildCipPrompt(d, brand, values.style, industry, values.mockup, useLogo));
      }
      if (values.json) console.log(JSON.stringify(results, null, 2));
      else for (const r of results) console.log(`\n${r.deliverable}:\n${r.prompt}\n`);
      return 0;
    }
    const results = await generateCipSet(brand, industry, values.style, deliverables, values.output, model, values.logo, ratio);
    if (values.json) console.log(JSON.stringify(results, null, 2));
    else console.log(`\n✅ Generated ${results.length} CIP mockups`);
    return results.length ? 0 : 1;
  }

  const promptData = await buildCipPrompt(values.deliverable || "business card", brand, values.style, industry, values.mockup, useLogo);
  if (values["prompt-only"]) {
    console.log(values.json ? JSON.stringify(promptData, null, 2) : `\nPrompt:\n${promptData.prompt}`);
    return 0;
  }
  const filepath = await generateWithNanoBanana(promptData, values.output, model, ratio, logoImage);
  if (values.json) console.log(JSON.stringify({ filepath, ...promptData }, null, 2));
  return filepath === null ? 1 : 0;
}

if (import.meta.main) process.exit(await main());
