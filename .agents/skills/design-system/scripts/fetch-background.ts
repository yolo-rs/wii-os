#!/usr/bin/env bun
/**
 * Background Image Fetcher
 * Returns curated Pexels images for slide backgrounds with brand-colored overlays
 * (no API key, no scraping).
 *
 * Usage:
 *   bun scripts/fetch-background.ts hero
 *   bun scripts/fetch-background.ts hero --css
 *   bun scripts/fetch-background.ts --list
 *   bun scripts/fetch-background.ts --colors
 */

import { join } from "node:path";
import { parseArgs } from "node:util";
import { findProjectRoot, pyQuote } from "./lib/project-root.ts";
import { loadCsv, type Row } from "./lib/search-core.ts";

const BACKGROUNDS_CSV = join(import.meta.dir, "..", "data", "slide-backgrounds.csv");

type TokenTree = Record<string, any>;

export interface BrandColors {
  primary: string;
  secondary: string;
  accent: string;
  background: string;
}

const DEFAULT_COLORS: BrandColors = {
  primary: "#3B82F6",
  secondary: "#F59E0B",
  accent: "#10B981",
  background: "#0D0D0D",
};

/** Resolve a reference like {primitive.color.ocean-blue.500} to its value. */
export function resolveTokenReference(ref: unknown, tokens: TokenTree): unknown {
  if (typeof ref !== "string" || !ref.startsWith("{") || !ref.endsWith("}")) return ref;
  let current: any = tokens;
  for (const key of ref.slice(1, -1).split(".")) {
    if (current && typeof current === "object" && !Array.isArray(current)) current = current[key];
    else return null;
  }
  if (current && typeof current === "object" && "$value" in current) return current.$value;
  return current;
}

/** Brand colors from assets/design-tokens.json (semantic references resolved), with defaults. */
export async function loadBrandColors(tokensPath?: string): Promise<BrandColors> {
  const path = tokensPath ?? join(await findProjectRoot(), "assets", "design-tokens.json");
  try {
    const tokens = (await Bun.file(path).json()) as TokenTree;
    const colors: TokenTree = tokens.primitive?.color ?? {};
    const semantic: TokenTree = tokens.semantic?.color ?? {};

    if (Object.keys(semantic).length > 0) {
      const resolveRole = (role: string) => resolveTokenReference(semantic[role]?.$value, tokens) as string | undefined;
      const primary = resolveRole("primary");
      const secondary = resolveRole("secondary");
      if (primary && secondary) {
        return {
          primary,
          secondary,
          accent: resolveRole("accent") || primary,
          background: resolveRole("background") || "#0D0D0D",
        };
      }
    }

    const firstShade = (keys: string[]): string | undefined => {
      for (const key of keys) {
        const scale = colors[key];
        if (scale && typeof scale === "object") {
          const value = scale["500"]?.$value;
          if (value) return value;
        }
      }
      return undefined;
    };

    return {
      primary: firstShade(["ocean-blue", "coral", "blue", "primary"]) || DEFAULT_COLORS.primary,
      secondary: firstShade(["golden-amber", "purple", "amber", "secondary"]) || DEFAULT_COLORS.secondary,
      accent: firstShade(["emerald", "mint", "green", "accent"]) || DEFAULT_COLORS.accent,
      background: colors.dark?.["800"]?.$value ?? "#0D0D0D",
    };
  } catch {
    return { ...DEFAULT_COLORS };
  }
}

export async function loadBackgroundsConfig(): Promise<Record<string, Row>> {
  const config: Record<string, Row> = {};
  if (!(await Bun.file(BACKGROUNDS_CSV).exists())) {
    console.log(`Warning: ${BACKGROUNDS_CSV} not found`);
    return config;
  }
  for (const row of await loadCsv(BACKGROUNDS_CSV)) config[row.slide_type!] = row;
  return config;
}

export function getOverlayCss(style: string, brand: BrandColors): string {
  const overlays: Record<string, string> = {
    "gradient-dark": `linear-gradient(135deg, ${brand.background}E6, ${brand.background}B3)`,
    "gradient-brand": `linear-gradient(135deg, ${brand.primary}CC, ${brand.secondary}99)`,
    "gradient-accent": `linear-gradient(135deg, ${brand.accent}99, transparent)`,
    "blur-dark": "rgba(13,13,13,0.8)",
    "desaturate-dark": "rgba(13,13,13,0.7)",
  };
  return overlays[style] ?? overlays["gradient-dark"]!;
}

const pexels = (id: string) =>
  `https://images.pexels.com/photos/${id}/pexels-photo-${id}.jpeg?auto=compress&cs=tinysrgb&w=1920`;

// Curated high-quality images from Pexels (free to use, pre-selected for brand aesthetic)
export const CURATED_IMAGES: Record<string, string[]> = {
  hero: ["3861969", "2582937", "1089438"].map(pexels),
  vision: ["3183150", "3182812", "3184291"].map(pexels),
  team: ["3184418", "3184338", "3182773"].map(pexels),
  testimonial: ["3184465", "1181622"].map(pexels),
  cta: ["3184339", "3184298"].map(pexels),
  problem: ["3760529", "897817"].map(pexels),
  solution: ["3184292", "3184644"].map(pexels),
  hook: ["2582937", "1089438"].map(pexels),
  social: ["3184360", "3184287"].map(pexels),
  demo: ["1181675", "3861958"].map(pexels),
};

export function getCuratedImages(slideType: string): string[] {
  return CURATED_IMAGES[slideType] ?? CURATED_IMAGES.hero ?? [];
}

export function getPexelsSearchUrl(keywords: string): string {
  return `https://www.pexels.com/search/${pyQuote(keywords)}/`;
}

export async function getBackgroundImage(slideType: string): Promise<Record<string, any>> {
  const brandColors = await loadBrandColors();
  const config = await loadBackgroundsConfig();
  const slideConfig = config[slideType];
  let overlayStyle = "gradient-dark";
  let keywords = slideType;
  if (slideConfig) {
    keywords = slideConfig.search_keywords ?? slideConfig.image_category ?? slideType;
    overlayStyle = slideConfig.overlay_style ?? "gradient-dark";
  }

  const urls = getCuratedImages(slideType);
  if (urls.length > 0) {
    return {
      url: urls[0],
      all_urls: urls,
      overlay: getOverlayCss(overlayStyle, brandColors),
      attribution: "Photo from Pexels (free to use)",
      source: "pexels-curated",
      search_url: getPexelsSearchUrl(keywords),
    };
  }
  return {
    url: null,
    overlay: getOverlayCss(overlayStyle, brandColors),
    keywords,
    search_url: getPexelsSearchUrl(keywords),
    available_types: Object.keys(CURATED_IMAGES),
  };
}

export function generateCssForBackground(result: Record<string, any>, slideClass = ".slide-with-bg"): string {
  if (!result.url) {
    return `/* No image scraped. Search manually: ${result.search_url ?? ""} */
/* Overlay ready: ${result.overlay ?? "gradient-dark"} */
`;
  }
  return `${slideClass} {
    background-image: url('${result.url}');
    background-size: cover;
    background-position: center;
    position: relative;
}

${slideClass}::before {
    content: '';
    position: absolute;
    inset: 0;
    background: ${result.overlay};
}

${slideClass} .content {
    position: relative;
    z-index: 1;
}

/* ${result.attribution ?? "Pexels"} - ${result.search_url ?? ""} */
`;
}

const HELP = `usage: fetch-background.ts [-h] [--list] [--css] [--json] [--colors] [--all] [slide_type]

Get background images for slides

positional arguments:
  slide_type  Slide type (hero, vision, team, etc.)

options:
  --list      List available slide types
  --css       Output CSS for the background
  --json      Output JSON
  --colors    Show brand colors
  --all       Show all curated URLs`;

async function main(): Promise<void> {
  const { values, positionals } = parseArgs({
    args: process.argv.slice(2),
    allowPositionals: true,
    options: {
      list: { type: "boolean" },
      css: { type: "boolean" },
      json: { type: "boolean" },
      colors: { type: "boolean" },
      all: { type: "boolean" },
      help: { type: "boolean", short: "h" },
    },
  });

  if (values.help) {
    console.log(HELP);
    return;
  }
  if (values.colors) {
    console.log("\nBrand Colors (from design-tokens.json):");
    for (const [name, value] of Object.entries(await loadBrandColors())) console.log(`  ${name}: ${value}`);
    return;
  }
  if (values.list) {
    console.log("\nAvailable slide types (curated images):");
    for (const [slideType, urls] of Object.entries(CURATED_IMAGES)) console.log(`  ${slideType}: ${urls.length} images`);
    return;
  }
  const slideType = positionals[0];
  if (!slideType) {
    console.log(HELP);
    return;
  }

  const result = await getBackgroundImage(slideType);
  if (values.json) {
    console.log(JSON.stringify(result, null, 2));
  } else if (values.css) {
    console.log(generateCssForBackground(result));
  } else if (values.all) {
    console.log(`\nAll images for '${slideType}':`);
    (result.all_urls ?? []).forEach((url: string, i: number) => console.log(`  ${i + 1}. ${url}`));
  } else {
    console.log(`\nImage URL: ${result.url}`);
    console.log(`Alternatives: ${(result.all_urls ?? []).length} available (use --all)`);
    console.log(`Overlay: ${result.overlay}`);
  }
}

if (import.meta.main) await main();
