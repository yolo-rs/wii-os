#!/usr/bin/env bun
/**
 * extract-colors.ts
 *
 * Extract dominant colors from an image and compare against brand palette.
 * Runs on Bun without external image processing dependencies.
 *
 * For full color extraction from images, integrate with an image-analysis
 * skill or use ImageMagick via shell commands.
 *
 * Usage:
 *   bun scripts/extract-colors.ts <image-path>
 *   bun scripts/extract-colors.ts <image-path> --brand-file <path>
 *   bun scripts/extract-colors.ts --palette  # Show brand palette from guidelines
 *
 * Integration:
 *   magick <image> -colors 10 -depth 8 -format "%c" histogram:info:
 */

import { isAbsolute, join } from "node:path";

const DEFAULT_GUIDELINES_PATH = "docs/brand-guidelines.md";

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

export interface BrandPalette {
  primary: string[];
  secondary: string[];
  neutral: string[];
  semantic: string[];
  all: string[];
}

function resolvePath(p: string): string {
  return isAbsolute(p) ? p : join(process.cwd(), p);
}

/** Extract hex colors from markdown content. */
export function extractHexColors(text: string): string[] {
  return [...new Set(text.match(/#[0-9A-Fa-f]{6}\b/g) ?? [])];
}

/** Parse markdown brand guidelines content for its color palette. */
export function parseBrandColorsFromText(content: string): BrandPalette {
  const palette: BrandPalette = { primary: [], secondary: [], neutral: [], semantic: [], all: [] };
  const sections: Array<{ name: Exclude<keyof BrandPalette, "all">; regex: RegExp }> = [
    { name: "primary", regex: /### Primary[\s\S]*?(?=###|##|$)/i },
    { name: "secondary", regex: /### Secondary[\s\S]*?(?=###|##|$)/i },
    { name: "neutral", regex: /### Neutral[\s\S]*?(?=###|##|$)/i },
    { name: "semantic", regex: /### Semantic[\s\S]*?(?=###|##|$)/i },
  ];
  for (const { name, regex } of sections) {
    const match = content.match(regex);
    if (match) {
      const colors = extractHexColors(match[0]);
      palette[name] = colors;
      palette.all.push(...colors);
    }
  }
  palette.all = [...new Set(palette.all)];
  return palette;
}

/** Parse brand guidelines file for color palette; null when the file is missing. */
export async function parseBrandColors(guidelinesPath: string): Promise<BrandPalette | null> {
  const file = Bun.file(resolvePath(guidelinesPath));
  if (!(await file.exists())) return null;
  return parseBrandColorsFromText(await file.text());
}

export function hexToRgb(hex: string): Rgb | null {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  return result
    ? { r: parseInt(result[1]!, 16), g: parseInt(result[2]!, 16), b: parseInt(result[3]!, 16) }
    : null;
}

export function rgbToHex(r: number, g: number, b: number): string {
  return (
    "#" +
    [r, g, b]
      .map((x) => Math.round(x).toString(16).padStart(2, "0"))
      .join("")
      .toUpperCase()
  );
}

/** Euclidean distance in RGB space. */
export function colorDistance(color1: string | Rgb, color2: string | Rgb): number {
  const rgb1 = typeof color1 === "string" ? hexToRgb(color1) : color1;
  const rgb2 = typeof color2 === "string" ? hexToRgb(color2) : color2;
  if (!rgb1 || !rgb2) return Infinity;
  return Math.sqrt((rgb1.r - rgb2.r) ** 2 + (rgb1.g - rgb2.g) ** 2 + (rgb1.b - rgb2.b) ** 2);
}

export function findNearestBrandColor(
  color: string | Rgb,
  brandColors: string[],
): { color: string | null; distance: number } {
  let nearest: string | null = null;
  let minDistance = Infinity;
  for (const brandColor of brandColors) {
    const distance = colorDistance(color, brandColor);
    if (distance < minDistance) {
      minDistance = distance;
      nearest = brandColor;
    }
  }
  return { color: nearest, distance: minDistance };
}

/** Brand compliance percentage. Distance threshold: 50 (out of max ~441 for RGB). */
export function calculateCompliance(
  extractedColors: Array<string | Rgb> | null | undefined,
  brandColors: string[] | null | undefined,
  threshold = 50,
): number {
  if (!extractedColors || extractedColors.length === 0) return 100;
  if (!brandColors || brandColors.length === 0) return 0;
  const matchCount = extractedColors.filter(
    (color) => findNearestBrandColor(color, brandColors).distance <= threshold,
  ).length;
  return Math.round((matchCount / extractedColors.length) * 100);
}

export function generateImageMagickCommand(imagePath: string, numColors = 10): string {
  return `magick "${imagePath}" -colors ${numColors} -depth 8 -format "%c" histogram:info:`;
}

/** Parse ImageMagick histogram output to extract colors, most common first. */
export function parseImageMagickOutput(output: string): Array<{ hex: string; count: number }> {
  const colors: Array<{ hex: string; count: number }> = [];
  for (const line of output.trim().split("\n")) {
    const hexMatch = line.match(/#([0-9A-Fa-f]{6})/);
    const countMatch = line.match(/^\s*(\d+):/);
    if (hexMatch) {
      colors.push({ hex: "#" + hexMatch[1]!.toUpperCase(), count: countMatch ? parseInt(countMatch[1]!) : 0 });
    }
  }
  colors.sort((a, b) => b.count - a.count);
  return colors;
}

function displayPalette(palette: BrandPalette): void {
  console.log("\n" + "=".repeat(50));
  console.log("BRAND COLOR PALETTE");
  console.log("=".repeat(50));
  const groups: Array<[string, string[]]> = [
    ["Primary Colors", palette.primary],
    ["Secondary Colors", palette.secondary],
    ["Neutral Colors", palette.neutral],
    ["Semantic Colors", palette.semantic],
  ];
  for (const [label, colors] of groups) {
    if (colors.length > 0) {
      console.log(`\n${label}:`);
      for (const c of colors) console.log(`  ${c}`);
    }
  }
  console.log("\n" + "=".repeat(50));
  console.log(`Total: ${palette.all.length} colors in brand palette`);
  console.log("=".repeat(50) + "\n");
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const jsonOutput = args.includes("--json");
  const showPalette = args.includes("--palette");
  const brandFileIdx = args.indexOf("--brand-file");
  const brandFileValue = brandFileIdx !== -1 ? (args[brandFileIdx + 1] ?? null) : null;
  const brandFile = brandFileValue ?? DEFAULT_GUIDELINES_PATH;
  const imagePath = args.find((a) => !a.startsWith("--") && a !== brandFileValue);

  const brandPalette = await parseBrandColors(brandFile);
  if (!brandPalette) {
    console.error(`Brand guidelines not found at: ${brandFile}`);
    console.error(`Create brand guidelines or specify path with --brand-file`);
    process.exit(1);
  }

  if (showPalette || !imagePath) {
    if (jsonOutput) {
      console.log(JSON.stringify(brandPalette, null, 2));
    } else {
      displayPalette(brandPalette);
      if (!imagePath) {
        console.log("To extract colors from an image:");
        console.log("  bun scripts/extract-colors.ts <image-path>");
        console.log("\nOr use ImageMagick directly:");
        console.log('  magick image.png -colors 10 -depth 8 -format "%c" histogram:info:');
      }
    }
    return;
  }

  const resolvedPath = resolvePath(imagePath);
  if (!(await Bun.file(resolvedPath).exists())) {
    console.error(`Image not found: ${resolvedPath}`);
    process.exit(1);
  }

  const command = generateImageMagickCommand(resolvedPath);
  const result = {
    image: resolvedPath,
    brandPalette,
    extractionCommand: command,
    instructions: [
      "1. Run the ImageMagick command to extract colors:",
      `   ${command}`,
      "",
      "2. Or use an image-analysis skill (e.g. ai-multimodal, if installed) to extract the 10 most dominant colors as hex values",
      "",
      "3. Then compare extracted colors against brand palette",
    ],
    complianceCheck: {
      threshold: 50,
      description: "Colors within distance 50 (RGB space) are considered brand-compliant",
      brandColors: brandPalette.all,
    },
  };

  if (jsonOutput) {
    console.log(JSON.stringify(result, null, 2));
    return;
  }
  console.log("\n" + "=".repeat(60));
  console.log("COLOR EXTRACTION HELPER");
  console.log("=".repeat(60));
  console.log(`\nImage: ${result.image}`);
  console.log(`\nBrand Colors: ${brandPalette.all.length} colors loaded`);
  console.log("\nTo extract colors from this image:\n");
  for (const line of result.instructions) console.log(line);
  console.log("\n" + "=".repeat(60));
  console.log("\nBrand Palette Reference:");
  console.log(`  Primary: ${brandPalette.primary.join(", ") || "none"}`);
  console.log(`  Secondary: ${brandPalette.secondary.join(", ") || "none"}`);
  console.log(`  Neutral: ${brandPalette.neutral.join(", ") || "none"}`);
  console.log("=".repeat(60) + "\n");
}

if (import.meta.main) await main();
