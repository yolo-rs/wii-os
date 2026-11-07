#!/usr/bin/env bun
/**
 * Logo generation with Gemini, Atlas Cloud, or MuAPI.
 *
 * Gemini remains the default provider (REST generateContent, GEMINI_API_KEY).
 * Atlas Cloud is opt-in with `--provider atlas` (ATLASCLOUD_API_KEY) and MuAPI
 * with `--provider muapi` (MUAPI_API_KEY); both use asynchronous image APIs.
 *
 * Models:
 * - Nano Banana (default): gemini-2.5-flash-image - fast, high-volume, low-latency
 * - Nano Banana Pro (--pro): gemini-3-pro-image - professional quality, advanced reasoning
 * - MuAPI Nano Banana (--provider muapi): nano-banana - hosted asynchronous image generation
 *
 * Set GEMINI_FLASH_IMAGE_MODEL or GEMINI_PRO_IMAGE_MODEL to use another Gemini model id.
 *
 * Gemini returns PNG or JPEG data. The file gets the extension of the returned
 * type, so `--output logo.png` can be saved as `logo.jpg`; the path actually
 * written is printed.
 *
 * Exit status is 1 when nothing was generated: a failed single request, or a
 * batch where every variant failed. A partly successful batch exits 0.
 *
 * Usage:
 *   bun scripts/logo/generate.ts --prompt "tech startup logo minimalist blue"
 *   bun scripts/logo/generate.ts --prompt "coffee shop vintage badge" --style vintage --output logo.png
 *   bun scripts/logo/generate.ts --brand "TechFlow" --industry tech --style minimalist
 *   bun scripts/logo/generate.ts --brand "TechFlow" --pro
 *   bun scripts/logo/generate.ts --brand "TechFlow" --provider atlas
 *   bun scripts/logo/generate.ts --brand "TechFlow" --provider muapi --muapi-model nano-banana-pro
 *   bun scripts/logo/generate.ts --brand "Unikorn" --batch 9 --output-dir ./logos --pro
 */

import { basename, join } from "node:path";
import { parseArgs } from "node:util";
import { geminiInlineImage, loadEnv, pathForImageType, runtime, timestamp, validatePublicHttpsUrl } from "../lib/providers.ts";

await loadEnv();

// ============ CONFIGURATION ============
export const config = {
  GEMINI_API_KEY: process.env.GEMINI_API_KEY as string | undefined,
  ATLASCLOUD_API_KEY: process.env.ATLASCLOUD_API_KEY as string | undefined,
  MUAPI_API_KEY: process.env.MUAPI_API_KEY as string | undefined,
};

export const geminiModels = () => ({
  flash: process.env.GEMINI_FLASH_IMAGE_MODEL || "gemini-2.5-flash-image",
  pro: process.env.GEMINI_PRO_IMAGE_MODEL || "gemini-3-pro-image",
});

export const ATLAS_MODEL = "google/nano-banana-2-lite/text-to-image";
export const ATLAS_API_BASE = "https://api.atlascloud.ai/api/v1";
export const MUAPI_MODEL = "nano-banana";
export const MUAPI_MODELS = ["nano-banana", "nano-banana-pro"];
export const MUAPI_API_BASE = "https://api.muapi.ai/api/v1";
const ATLAS_POLL_INTERVAL_MS = 2000;
const ATLAS_MAX_POLLS = 90;
const MUAPI_POLL_INTERVAL_MS = 2000;
const MUAPI_MAX_POLLS = 90;

export const ASPECT_RATIOS = ["1:1", "16:9", "9:16", "4:3", "3:4"];
export const DEFAULT_ASPECT_RATIO = "1:1"; // Square is ideal for logos

const LOGO_PROMPT_TEMPLATE = (prompt: string) => `Generate a professional logo image: ${prompt}

Style requirements:
- Clean vector-style illustration suitable for a logo
- Simple, scalable design that works at any size
- Clear silhouette and recognizable shape
- Professional quality suitable for business use
- Centered composition on plain white or transparent background
- No text unless specifically requested
- High contrast and clear edges
- Square format, perfectly centered
- Output as a clean, high-quality logo image
`;

export const STYLE_MODIFIERS: Record<string, string> = {
  minimalist: "minimalist, simple geometric shapes, clean lines, lots of white space, single color or limited palette",
  vintage: "vintage, retro, badge style, distressed texture, heritage feel, warm earth tones",
  modern: "modern, sleek, gradient colors, tech-forward, innovative feel",
  luxury: "luxury, elegant, gold accents, refined, premium feel, serif typography",
  playful: "playful, fun, colorful, friendly, approachable, rounded shapes",
  corporate: "corporate, professional, trustworthy, stable, conservative colors",
  organic: "organic, natural, flowing lines, earth tones, sustainable feel",
  geometric: "geometric, abstract, mathematical precision, symmetrical",
  "hand-drawn": "hand-drawn, artisan, sketch-like, authentic, imperfect lines",
  "3d": "3D, dimensional, depth, shadows, isometric perspective",
  abstract: "abstract mark, conceptual, symbolic, non-literal representation, artistic interpretation",
  lettermark: "lettermark, single letter or initials, typographic, monogram style, distinctive character",
  wordmark: "wordmark, logotype, custom typography, brand name as logo, distinctive lettering",
  emblem: "emblem, badge, crest style, enclosed design, traditional, authoritative feel",
  mascot: "mascot, character, friendly face, personified, memorable figure",
  gradient: "gradient, color transition, vibrant, modern digital feel, smooth color flow",
  lineart: "line art, single stroke, continuous line, elegant simplicity, wire-frame style",
  "negative-space": "negative space, clever use of white space, hidden meaning, dual imagery, optical illusion",
};

export const INDUSTRY_PROMPTS: Record<string, string> = {
  tech: "technology company, digital, innovative, modern, circuit-like elements",
  healthcare: "healthcare, medical, caring, trust, cross or heart symbol",
  finance: "financial services, stable, trustworthy, growth, upward elements",
  food: "food and beverage, appetizing, warm colors, welcoming",
  fashion: "fashion brand, elegant, stylish, refined, artistic",
  fitness: "fitness and sports, dynamic, energetic, powerful, movement",
  eco: "eco-friendly, sustainable, natural, green, leaf or earth elements",
  education: "education, knowledge, growth, learning, book or cap symbol",
  "real-estate": "real estate, property, home, roof or building silhouette",
  creative: "creative agency, artistic, unique, expressive, colorful",
};

export function enhancePrompt(basePrompt: string, style?: string | null, industry?: string | null, brandName?: string | null): string {
  const parts = [basePrompt];
  if (style && STYLE_MODIFIERS[style]) parts.push(STYLE_MODIFIERS[style]);
  if (industry && INDUSTRY_PROMPTS[industry]) parts.push(INDUSTRY_PROMPTS[industry]);
  if (brandName) parts.unshift(`Logo for '${brandName}':`);
  return LOGO_PROMPT_TEMPLATE(parts.join(", "));
}

// ============ ATLAS CLOUD ============
function atlasPredictionData(response: any): Record<string, any> {
  if (!response || typeof response !== "object" || Array.isArray(response)) {
    throw new TypeError("Atlas Cloud returned an invalid response");
  }
  if (![undefined, null, 0, 200].includes(response.code)) {
    throw new Error(response.message || "Atlas Cloud request failed");
  }
  const data = response.data;
  if (!data || typeof data !== "object" || Array.isArray(data)) {
    throw new TypeError("Atlas Cloud response did not include prediction data");
  }
  return data;
}

export async function generateWithAtlas(prompt: string, outputPath: string, aspectRatio: string, apiKey: string | undefined | null, model: string): Promise<void> {
  if (!apiKey) throw new Error("ATLASCLOUD_API_KEY not set");
  let data = atlasPredictionData(
    await runtime.jsonRequest(`${ATLAS_API_BASE}/model/generateImage`, apiKey, {
      method: "POST",
      payload: { model, prompt, aspect_ratio: aspectRatio },
    }),
  );
  const predictionId = data.id;
  if (!predictionId) throw new Error("Atlas Cloud did not return a prediction ID");

  for (let poll = 0; poll <= ATLAS_MAX_POLLS; poll++) {
    const status = String(data.status ?? "").toLowerCase();
    if (status === "completed") {
      const outputs = data.outputs;
      if (!Array.isArray(outputs) || outputs.length === 0 || typeof outputs[0] !== "string") {
        throw new Error("Atlas Cloud completed without an image URL");
      }
      await runtime.downloadImage(outputs[0], outputPath, "image provider");
      return;
    }
    if (["failed", "timeout", "canceled", "cancelled"].includes(status)) {
      throw new Error(data.error || `Atlas Cloud prediction ${status}`);
    }
    if (poll === ATLAS_MAX_POLLS) break;
    await runtime.sleep(ATLAS_POLL_INTERVAL_MS);
    data = atlasPredictionData(await runtime.jsonRequest(`${ATLAS_API_BASE}/model/prediction/${predictionId}`, apiKey));
  }
  throw new Error("Atlas Cloud prediction timed out while polling");
}

// ============ MUAPI ============
const isPlainObject = (v: unknown): v is Record<string, any> => !!v && typeof v === "object" && !Array.isArray(v);

/** The response and the common MuAPI envelopes (data/output/result). */
function muapiResponseObjects(response: unknown): Array<Record<string, any>> {
  if (!isPlainObject(response)) throw new TypeError("MuAPI returned an invalid response");
  const objects = [response];
  for (const key of ["data", "output", "result"]) {
    const value = response[key];
    if (isPlainObject(value) && !objects.includes(value)) objects.push(value);
  }
  return objects;
}

function muapiResponseValue(response: unknown, keys: string[]): any {
  for (const item of muapiResponseObjects(response)) {
    for (const key of keys) {
      const value = item[key];
      if (value !== undefined && value !== null && value !== "") return value;
    }
  }
  return null;
}

function muapiError(response: unknown): string {
  const value = muapiResponseValue(response, ["error", "message", "detail"]);
  return typeof value === "string" ? value.slice(0, 300) : "MuAPI request failed";
}

function muapiResultUrl(response: unknown): string {
  const invalid = "MuAPI creation response did not include a valid HTTPS result URL";
  for (const item of muapiResponseObjects(response)) {
    const urls = item.urls;
    if (!isPlainObject(urls) || !("get" in urls)) continue;
    const resultUrl = urls.get;
    if (typeof resultUrl !== "string" || !resultUrl) throw new Error(invalid);
    try {
      validatePublicHttpsUrl(resultUrl);
    } catch {
      throw new Error(invalid);
    }
    return resultUrl;
  }
  throw new Error(invalid);
}

function muapiOutputUrl(response: unknown): string {
  for (const item of muapiResponseObjects(response)) {
    const outputs = item.outputs;
    if (!Array.isArray(outputs)) continue;
    for (const output of outputs) {
      if (typeof output === "string" && output.startsWith("https://")) return output;
      if (isPlainObject(output)) {
        for (const key of ["url", "image_url"]) {
          const value = output[key];
          if (typeof value === "string" && value.startsWith("https://")) return value;
        }
      }
    }
  }
  throw new Error("MuAPI completed without an HTTPS image URL");
}

export async function generateWithMuapi(prompt: string, outputPath: string, aspectRatio: string, apiKey: string | undefined | null, model: string): Promise<void> {
  if (!apiKey) throw new Error("MUAPI_API_KEY not set");
  if (!MUAPI_MODELS.includes(model)) {
    throw new Error(`Unsupported MuAPI logo model: ${model}. Choose one of: ${MUAPI_MODELS.join(", ")}`);
  }
  const response = await runtime.jsonRequest(`${MUAPI_API_BASE}/${model}`, apiKey, {
    method: "POST",
    payload: { prompt, aspect_ratio: aspectRatio },
    apiKeyHeader: "x-api-key",
  });
  const requestId = muapiResponseValue(response, ["request_id", "id"]);
  if (typeof requestId !== "string" || !requestId) throw new Error("MuAPI did not return a request ID");
  const resultUrl = muapiResultUrl(response);

  let data = response;
  for (let poll = 0; poll <= MUAPI_MAX_POLLS; poll++) {
    const status = String(muapiResponseValue(data, ["status"]) ?? "").toLowerCase();
    if (["completed", "succeeded", "success"].includes(status)) {
      await runtime.downloadImage(muapiOutputUrl(data), outputPath, "MuAPI");
      return;
    }
    if (["failed", "error", "timeout", "canceled", "cancelled"].includes(status)) {
      throw new Error(`MuAPI generation ${status}: ${muapiError(data)}`);
    }
    if (poll === MUAPI_MAX_POLLS) break;
    await runtime.sleep(MUAPI_POLL_INTERVAL_MS);
    data = await runtime.jsonRequest(resultUrl, apiKey, { apiKeyHeader: "x-api-key" });
  }
  throw new Error("MuAPI prediction timed out while polling");
}

// ============ GEMINI ============
const SAFETY_SETTINGS = ["HARM_CATEGORY_HATE_SPEECH", "HARM_CATEGORY_DANGEROUS_CONTENT", "HARM_CATEGORY_SEXUALLY_EXPLICIT", "HARM_CATEGORY_HARASSMENT"].map(
  (category) => ({ category, threshold: "BLOCK_LOW_AND_ABOVE" }),
);

/** Generate with Gemini and return the path the image was saved to. */
export async function generateWithGemini(prompt: string, outputPath: string, aspectRatio: string, usePro: boolean): Promise<string> {
  if (!config.GEMINI_API_KEY) throw new Error("GEMINI_API_KEY not set");
  const models = geminiModels();
  const response = await runtime.geminiGenerateContent(usePro ? models.pro : models.flash, config.GEMINI_API_KEY, {
    contents: [{ role: "user", parts: [{ text: prompt }] }],
    generationConfig: { responseModalities: ["IMAGE", "TEXT"], imageConfig: { aspectRatio } },
    safetySettings: SAFETY_SETTINGS,
  });
  const image = geminiInlineImage(response);
  if (!image) throw new Error("Gemini did not return an image");
  const savedPath = pathForImageType(outputPath, image.mimeType);
  if (savedPath !== outputPath) {
    console.log(`Gemini returned ${image.mimeType}, so saving as ${savedPath} instead of ${outputPath}`);
  }
  await Bun.write(savedPath, image.data);
  return savedPath;
}

export interface LogoOptions {
  prompt: string;
  style?: string | null;
  industry?: string | null;
  brandName?: string | null;
  outputPath?: string | null;
  usePro?: boolean;
  aspectRatio?: string | null;
  provider?: string;
  atlasModel?: string;
  muapiModel?: string;
}

/** Generate a logo; returns the saved path or null on failure. */
async function generateLogo(options: LogoOptions): Promise<string | null> {
  const { prompt, style, industry, brandName, usePro = false, provider = "gemini", atlasModel = ATLAS_MODEL, muapiModel = MUAPI_MODEL } = options;
  const fullPrompt = enhancePrompt(prompt, style, industry, brandName);
  const ratio = options.aspectRatio && ASPECT_RATIOS.includes(options.aspectRatio) ? options.aspectRatio : DEFAULT_ASPECT_RATIO;
  let outputPath = options.outputPath ?? `${brandName ? brandName.toLowerCase().replaceAll(" ", "_") : "logo"}_${timestamp()}.png`;

  const models = geminiModels();
  const modelLabel =
    provider === "atlas"
      ? `Atlas Cloud (${atlasModel})`
      : provider === "muapi"
        ? `MuAPI (${muapiModel})`
        : usePro
          ? `Nano Banana Pro (${models.pro})`
          : `Nano Banana (${models.flash})`;

  console.log(`Generating logo with ${modelLabel}...`);
  console.log(`Aspect ratio: ${ratio}`);
  console.log(`Prompt: ${fullPrompt.slice(0, 150)}...`);
  console.log();

  try {
    if (provider === "atlas") await generateWithAtlas(fullPrompt, outputPath, ratio, config.ATLASCLOUD_API_KEY, atlasModel);
    else if (provider === "muapi") await generateWithMuapi(fullPrompt, outputPath, ratio, config.MUAPI_API_KEY, muapiModel);
    else outputPath = await generateWithGemini(fullPrompt, outputPath, ratio, usePro);
    console.log(`Logo saved to: ${outputPath}`);
    return outputPath;
  } catch (error) {
    console.log(`Error generating logo: ${(error as Error).message}`);
    return null;
  }
}

const BATCH_STYLES: Array<[string, string]> = [
  ["minimalist", "Clean, simple geometric shape with minimal details"],
  ["modern", "Sleek gradient with tech-forward aesthetic"],
  ["geometric", "Abstract geometric patterns, mathematical precision"],
  ["gradient", "Vibrant color transitions, modern digital feel"],
  ["abstract", "Conceptual symbolic representation"],
  ["lettermark", "Stylized letter 'U' as monogram"],
  ["negative-space", "Clever use of negative space, hidden meaning"],
  ["lineart", "Single stroke continuous line design"],
  ["3d", "Dimensional design with depth and shadows"],
];

/** Generate multiple logo variants with different styles. */
async function generateBatch(options: {
  prompt: string;
  brandName: string;
  count: number;
  outputDir: string;
  usePro?: boolean;
  brandContext?: string | null;
  aspectRatio?: string | null;
  provider?: string;
  atlasModel?: string;
  muapiModel?: string;
}): Promise<string[]> {
  const { prompt, brandName, count, outputDir, usePro = false, brandContext, provider = "gemini", atlasModel = ATLAS_MODEL, muapiModel = MUAPI_MODEL } = options;
  const results: string[] = [];
  const modelLabel = provider === "atlas" ? `Atlas Cloud (${atlasModel})` : provider === "muapi" ? `MuAPI (${muapiModel})` : `Nano Banana ${usePro ? "Pro" : "Flash"}`;
  const ratio = options.aspectRatio && ASPECT_RATIOS.includes(options.aspectRatio) ? options.aspectRatio : DEFAULT_ASPECT_RATIO;

  console.log(`\n${"=".repeat(60)}`);
  console.log(`  BATCH LOGO GENERATION: ${brandName}`);
  console.log(`  Model: ${modelLabel}`);
  console.log(`  Aspect Ratio: ${ratio}`);
  console.log(`  Variants: ${count}`);
  console.log(`  Output: ${outputDir}`);
  console.log(`${"=".repeat(60)}\n`);

  const total = Math.min(count, BATCH_STYLES.length);
  for (let i = 0; i < total; i++) {
    const [styleKey, styleDesc] = BATCH_STYLES[i]!;
    let enhanced = `${prompt}, ${styleDesc}`;
    if (brandContext) enhanced = `${brandContext}, ${enhanced}`;
    const filename = `${brandName.toLowerCase().replaceAll(" ", "_")}_${styleKey}_${String(i + 1).padStart(2, "0")}.png`;

    console.log(`[${i + 1}/${count}] Generating ${styleKey} variant...`);
    const result = await api.generateLogo({
      prompt: enhanced,
      style: styleKey,
      industry: "tech",
      brandName,
      outputPath: join(outputDir, filename),
      usePro,
      aspectRatio: options.aspectRatio,
      provider,
      atlasModel,
      muapiModel,
    });
    if (result) {
      results.push(result);
      console.log(`  ✓ Saved: ${basename(result)}\n`);
    } else {
      console.log(`  ✗ Failed: ${styleKey}\n`);
    }
    if (i < count - 1) await runtime.sleep(2000); // Rate limiting between requests
  }

  console.log(`\n${"=".repeat(60)}`);
  console.log(`  BATCH COMPLETE: ${results.length}/${count} logos generated`);
  console.log(`${"=".repeat(60)}\n`);
  return results;
}

/** Patchable entry points (tests replace generateLogo like mock.patch.object). */
export const api = { generateLogo, generateBatch };

function usageError(message: string): number {
  console.error(`generate.ts: error: ${message}`);
  return 2;
}

/** CLI; returns the process exit status. */
export async function main(argv = process.argv.slice(2)): Promise<number> {
  const { values } = parseArgs({
    args: argv,
    options: {
      prompt: { type: "string", short: "p" },
      brand: { type: "string", short: "b" },
      style: { type: "string", short: "s" },
      industry: { type: "string", short: "i" },
      output: { type: "string", short: "o" },
      "output-dir": { type: "string" },
      batch: { type: "string" },
      "brand-context": { type: "string" },
      pro: { type: "boolean" },
      provider: { type: "string", default: "gemini" },
      "atlas-model": { type: "string", default: ATLAS_MODEL },
      "muapi-model": { type: "string", default: MUAPI_MODEL },
      "aspect-ratio": { type: "string", short: "r", default: DEFAULT_ASPECT_RATIO },
      "list-styles": { type: "boolean" },
      "list-industries": { type: "boolean" },
    },
  });

  const provider = String(values.provider);
  if (!["gemini", "atlas", "muapi"].includes(provider)) return usageError(`argument --provider: invalid choice: '${provider}'`);
  if (values.style && !STYLE_MODIFIERS[values.style]) return usageError(`argument --style/-s: invalid choice: '${values.style}'`);
  if (values.industry && !INDUSTRY_PROMPTS[values.industry]) return usageError(`argument --industry/-i: invalid choice: '${values.industry}'`);
  if (!MUAPI_MODELS.includes(String(values["muapi-model"]))) return usageError(`argument --muapi-model: invalid choice: '${values["muapi-model"]}'`);
  if (!ASPECT_RATIOS.includes(String(values["aspect-ratio"]))) return usageError(`argument --aspect-ratio/-r: invalid choice: '${values["aspect-ratio"]}'`);
  if (provider !== "gemini" && values.pro) {
    return usageError("--pro is only available with --provider gemini; use --muapi-model nano-banana-pro for MuAPI");
  }

  if (values["list-styles"]) {
    console.log("Available styles:");
    for (const [style, desc] of Object.entries(STYLE_MODIFIERS)) console.log(`  ${style}: ${desc.slice(0, 60)}...`);
    return 0;
  }
  if (values["list-industries"]) {
    console.log("Available industries:");
    for (const [industry, desc] of Object.entries(INDUSTRY_PROMPTS)) console.log(`  ${industry}: ${desc.slice(0, 60)}...`);
    return 0;
  }
  if (!values.prompt && !values.brand) return usageError("Either --prompt or --brand is required");

  const prompt = values.prompt || "professional logo";
  const batch = values.batch !== undefined ? Number.parseInt(values.batch, 10) : 0;
  if (batch) {
    const brand = values.brand ?? "Logo";
    const results = await api.generateBatch({
      prompt,
      brandName: brand,
      count: batch,
      outputDir: values["output-dir"] || `./${brand.toLowerCase().replaceAll(" ", "_")}_logos`,
      usePro: Boolean(values.pro),
      brandContext: values["brand-context"],
      aspectRatio: values["aspect-ratio"],
      provider,
      atlasModel: values["atlas-model"],
      muapiModel: values["muapi-model"],
    });
    return results.length ? 0 : 1;
  }

  const saved = await api.generateLogo({
    prompt,
    style: values.style,
    industry: values.industry,
    brandName: values.brand,
    outputPath: values.output,
    usePro: Boolean(values.pro),
    aspectRatio: values["aspect-ratio"],
    provider,
    atlasModel: values["atlas-model"],
    muapiModel: values["muapi-model"],
  });
  return saved === null ? 1 : 0;
}

if (import.meta.main) process.exit(await main());
