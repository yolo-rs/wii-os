#!/usr/bin/env bun
/**
 * validate-asset.ts
 *
 * Validates marketing assets against brand guidelines.
 * Checks: file naming, file size, format, manifest registration.
 *
 * Usage:
 *   bun scripts/validate-asset.ts <asset-path>
 *   bun scripts/validate-asset.ts <asset-path> --json
 *
 * For color validation of images, use with extract-colors.ts
 */

import { basename, extname, isAbsolute, join, relative } from "node:path";

export const RULES = {
  naming: {
    pattern: /^[a-z]+_[a-z0-9-]+_[a-z0-9-]+_\d{8}(_[a-z0-9-]+)?\.[a-z]+$/,
    description: "{type}_{campaign}_{description}_{timestamp}_{variant}.{ext}",
    examples: ["banner_claude-launch_hero-image_20251209.png", "logo_brand-refresh_horizontal_20251209_dark.svg"],
  },
  dimensions: {
    banner: { minWidth: 600, minHeight: 300 },
    logo: { minWidth: 100, minHeight: 100 },
    design: { minWidth: 800, minHeight: 600 },
    video: { minWidth: 640, minHeight: 480 },
    default: { minWidth: 100, minHeight: 100 },
  },
  fileSize: {
    image: { max: 5 * 1024 * 1024, recommended: 1 * 1024 * 1024 },
    video: { max: 100 * 1024 * 1024, recommended: 50 * 1024 * 1024 },
    svg: { max: 500 * 1024, recommended: 100 * 1024 },
  },
  formats: {
    image: ["png", "jpg", "jpeg", "webp", "gif"],
    vector: ["svg"],
    video: ["mp4", "mov", "webm"],
    document: ["pdf", "psd", "ai", "fig"],
  },
};

export interface ParsedFilename {
  type: string;
  campaign: string;
  description: string;
  timestamp: string;
  variant: string | null;
  extension: string;
}

export function parseFilename(filename: string): ParsedFilename | null {
  const parts = filename.replace(/\.[^.]+$/, "").split("_");
  if (parts.length < 4) return null;
  return {
    type: parts[0]!,
    campaign: parts[1]!,
    description: parts[2]!,
    timestamp: parts[3]!,
    variant: parts.length > 4 ? parts[4]! : null,
    extension: extname(filename).slice(1).toLowerCase(),
  };
}

export function validateFilename(filename: string) {
  const issues: string[] = [];
  const suggestions: string[] = [];

  if (!RULES.naming.pattern.test(filename)) {
    issues.push("Filename does not match naming convention");
    suggestions.push(`Expected format: ${RULES.naming.description}`);
    suggestions.push(`Examples: ${RULES.naming.examples.join(", ")}`);
  }

  const parsed = parseFilename(filename);
  if (parsed) {
    if (!/^\d{8}$/.test(parsed.timestamp)) issues.push("Timestamp should be YYYYMMDD format");
    if (parsed.campaign && !/^[a-z0-9-]+$/.test(parsed.campaign)) issues.push("Campaign name should be kebab-case");
    if (parsed.description && !/^[a-z0-9-]+$/.test(parsed.description)) issues.push("Description should be kebab-case");
    const validTypes = ["banner", "logo", "design", "video", "infographic", "icon", "photo"];
    if (!validTypes.includes(parsed.type)) suggestions.push(`Consider using type: ${validTypes.join(", ")}`);
  }
  return { valid: issues.length === 0, issues, suggestions, parsed };
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Bytes";
  const k = 1024;
  const sizes = ["Bytes", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

export function validateFileSize(size: number, extension: string) {
  const issues: string[] = [];
  const warnings: string[] = [];
  const limits = RULES.formats.video.includes(extension)
    ? RULES.fileSize.video
    : extension === "svg"
      ? RULES.fileSize.svg
      : RULES.fileSize.image;
  if (size > limits.max) {
    issues.push(`File size (${formatBytes(size)}) exceeds maximum (${formatBytes(limits.max)})`);
  } else if (size > limits.recommended) {
    warnings.push(`File size (${formatBytes(size)}) exceeds recommended (${formatBytes(limits.recommended)})`);
  }
  return { valid: issues.length === 0, issues, warnings, size };
}

export function validateFormat(extension: string) {
  const issues: string[] = [];
  const info: { category: string | null } = { category: null };
  const { image, vector, video, document } = RULES.formats;
  if (![...image, ...vector, ...video, ...document].includes(extension)) {
    issues.push(`Unsupported file format: .${extension}`);
    return { valid: false, issues, info };
  }
  if (image.includes(extension)) info.category = "image";
  else if (vector.includes(extension)) info.category = "vector";
  else if (video.includes(extension)) info.category = "video";
  else if (document.includes(extension)) info.category = "document";
  return { valid: true, issues, info };
}

async function checkManifest(filepath: string) {
  const manifestFile = Bun.file(join(process.cwd(), ".assets", "manifest.json"));
  if (!(await manifestFile.exists())) return { registered: false, message: "Manifest not found" };
  try {
    const manifest = (await manifestFile.json()) as { assets?: Array<{ path: string }> };
    const relativePath = relative(process.cwd(), filepath);
    const found = manifest.assets?.find((a) => a.path === relativePath || a.path === filepath);
    return {
      registered: !!found,
      message: found ? "Asset registered in manifest" : "Asset not in manifest",
      asset: found,
    };
  } catch {
    return { registered: false, message: "Error reading manifest" };
  }
}

export function suggestFilename(parsed: ParsedFilename | null): string | null {
  if (!parsed) return null;
  const today = new Date().toISOString().slice(0, 10).replace(/-/g, "");
  return `${parsed.type || "asset"}_${parsed.campaign || "general"}_${parsed.description || "untitled"}_${today}.${parsed.extension || "png"}`;
}

export async function validateAsset(assetPath: string) {
  const results = {
    path: assetPath,
    filename: basename(assetPath),
    valid: true,
    issues: [] as string[],
    warnings: [] as string[],
    suggestions: [] as string[],
    checks: {} as Record<string, any>,
  };

  const file = Bun.file(assetPath);
  if (!(await file.exists())) {
    results.valid = false;
    results.issues.push(`File not found: ${assetPath}`);
    return results;
  }

  const filename = basename(assetPath);
  const extension = extname(filename).slice(1).toLowerCase();

  const filenameResult = validateFilename(filename);
  results.checks.filename = filenameResult;
  if (!filenameResult.valid) {
    results.issues.push(...filenameResult.issues);
    results.suggestions.push(...filenameResult.suggestions);
  }

  const formatResult = validateFormat(extension);
  results.checks.format = formatResult;
  if (!formatResult.valid) results.issues.push(...formatResult.issues);

  const sizeResult = validateFileSize(file.size, extension);
  results.checks.fileSize = sizeResult;
  if (!sizeResult.valid) results.issues.push(...sizeResult.issues);
  results.warnings.push(...sizeResult.warnings);

  const manifestResult = await checkManifest(assetPath);
  results.checks.manifest = manifestResult;
  if (!manifestResult.registered) {
    results.warnings.push("Asset not registered in manifest.json");
    results.suggestions.push("Register asset in .assets/manifest.json for tracking");
  }

  if (!filenameResult.valid && filenameResult.parsed) {
    const suggested = suggestFilename(filenameResult.parsed);
    if (suggested) results.suggestions.push(`Suggested filename: ${suggested}`);
  }

  results.valid = results.issues.length === 0;
  return results;
}

function formatOutput(results: Awaited<ReturnType<typeof validateAsset>>): string {
  const lines: string[] = [];
  lines.push("\n" + "=".repeat(60));
  lines.push(`ASSET VALIDATION: ${results.filename}`);
  lines.push("=".repeat(60));
  lines.push(`\nStatus: ${results.valid ? "PASS" : "FAIL"}`);
  lines.push(`Path: ${results.path}`);
  const groups: Array<[string, string[]]> = [
    ["ISSUES", results.issues],
    ["WARNINGS", results.warnings],
    ["SUGGESTIONS", results.suggestions],
  ];
  for (const [label, items] of groups) {
    if (items.length > 0) {
      lines.push(`\n${label}:`);
      for (const item of items) lines.push(`  - ${item}`);
    }
  }
  if (results.checks.fileSize?.size) lines.push(`\nFile Size: ${formatBytes(results.checks.fileSize.size)}`);
  lines.push("\n" + "=".repeat(60));
  return lines.join("\n");
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const jsonOutput = args.includes("--json");
  const assetPath = args.find((a) => !a.startsWith("--"));

  if (!assetPath) {
    console.error("Usage: bun scripts/validate-asset.ts <asset-path> [--json]");
    console.error("\nExamples:");
    console.error("  bun scripts/validate-asset.ts assets/banners/social-media/banner_launch_hero_20251209.png");
    console.error("  bun scripts/validate-asset.ts assets/logos/icon-only/logo-icon.svg --json");
    process.exit(1);
  }

  const resolvedPath = isAbsolute(assetPath) ? assetPath : join(process.cwd(), assetPath);
  const results = await validateAsset(resolvedPath);
  console.log(jsonOutput ? JSON.stringify(results, null, 2) : formatOutput(results));
  process.exit(results.valid ? 0 : 1);
}

if (import.meta.main) await main();
