#!/usr/bin/env bun
/**
 * Data integrity guardrail for ui-ux-pro-max. Bun built-ins only, so it can
 * run as a standalone pre-publish/CI check:
 *
 *     bun scripts/validate-data.ts
 *
 * Checks, per configured domain/stack CSV:
 *   - file exists
 *   - header row contains every column referenced in searchCols/outputCols
 *   - no duplicate primary-key values ("No") within a file
 *   - "Decision_Rules" JSON parses through the closed decision grammar
 * plus the cross-file semantic contracts (styles, reasoning, colors, charts,
 * typography, icons, catalogs, provenance, stack freshness, Aphrody brands).
 *
 * Exits 0 on success; exits 1 and prints every problem found on failure
 * (a data change can break several files at once, so report the full list).
 */
import { join } from "node:path";
import { CSV_CONFIG, DATA_DIR, STACK_CONFIG, STACK_COLS, STACK_CURRENT_APPLICABILITY } from "./core.ts";
import { parseCsv, type Row } from "./csv.ts";
import { parseDecisionRules } from "./reasoning-contract.ts";

export const REASONING_FILE = "ui-reasoning.csv";
export const STYLE_STATUSES = new Set(["active", "supplemental", "deprecated"]);
export const STACK_STATUSES = new Set([...STYLE_STATUSES, "unverified"]);
export const HEX_COLOR = /^#[0-9A-Fa-f]{6}$/;
export const STYLE_ID = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
export const WCAG_CONFORMANCE = /\bWCAG\s+A{2,3}\+?\b|\bWCAG\b.{0,40}\b(?:compliant|compliance)\b/i;
export const WCAG_GRADE = /\bWCAG\s+A{1,3}\b/i;
export const CHART_TEXT_FALLBACK = /table|list|summary|text/;
export const CHART_NON_COLOR_GUIDANCE =
  /color alone|hue alone|alone is insufficient|only distinction|only carriers?|supplementary|pattern|symbol|outline|direct (?:series |group )?label|marker shape|label every/;
export const CSS_IMPORT = /^\s*@import\s+url\((['"])(https:\/\/fonts\.googleapis\.com\/css2\?[^'"]+)\1\);\s*$/i;
export const FONT_WEIGHT = /(?<!\d)(?:[1-9]00)(?!\d)/g;
export const ICON_IMPORT =
  /import\s*\{\s*[A-Z][A-Za-z0-9]*(?:\s*,\s*[A-Z][A-Za-z0-9]*)*\s*\}\s*from\s*['"](?:@phosphor-icons\/react|phosphor-react-native|@heroicons\/react\/24\/(?:outline|solid))['"]/;
export const ICON_USAGE_REQUIREMENTS = [/aria-hidden/i, /text alternative/i, /accessible name/i, /aria-(?:pressed|expanded)/i];
export const LANDING_QUANTIFIED_CLAIM = /\b\d+(?:\.\d+)?x\b|\b(?:increases?|reduces?)\s+(?:engagement|conversion|returns?)\b/i;
export const CHART_RISKS = new Set(["risk:low", "risk:conditional", "risk:high"]);
export const ICON_ROLES = new Set(["decorative", "meaningful", "interactive", "guideline"]);
export const ICON_CONTEXTS = new Set(["decorative", "meaningful", "interactive"]);
export const COLOR_CONTRAST_PAIRS: Record<string, [string, string, number]> = {
  "On Primary": ["Primary", "normal-text", 4.5],
  "On Secondary": ["Secondary", "normal-text", 4.5],
  "On Accent": ["Accent", "normal-text", 4.5],
  Foreground: ["Background", "normal-text", 4.5],
  "Card Foreground": ["Card", "normal-text", 4.5],
  "Muted Foreground": ["Muted", "normal-text", 4.5],
  "On Destructive": ["Destructive", "normal-text", 4.5],
  Ring: ["Background", "non-text-focus-indicator", 3.0],
};
export const PROVENANCE_KINDS = new Set(["reasoning", "style", "dataset-contract", "catalog-snapshot"]);
export const PROVENANCE_STATUSES = new Set(["active", "supplemental", "deprecated"]);
export const PROVENANCE_SLAS = new Set(["manual-verified", "needs-review"]);
export const PROVENANCE_SOURCE_TYPES = new Set(["official", "derived"]);
export const PROVENANCE_APPLIES_TO = new Set(["search", "design-guidance", "design-system", "gallery", "style-search"]);
export const CORE_PROVENANCE_FILES = new Set([
  "colors.csv", "charts.csv", "ux-guidelines.csv", "landing.csv",
  "typography.csv", "icons.csv", "motion.csv", "app-interface.csv",
  "react-performance.csv", "stacks/html-tailwind.csv",
]);
export const CATALOG_PROVENANCE_FILES = new Set(["google-fonts.csv", "phosphor-icons-upstream.json"]);
export const CATALOG_PROVENANCE_IDS = new Set(["google-fonts-catalog-2026-08-13", "phosphor-icons-catalog-2.1.1"]);
export const FONT_LICENSES = new Set(["OFL", "APACHE2", "UFL"]);
export const GOOGLE_FONTS_REVISION = /^[0-9a-f]{40}$/;
export const PHOSPHOR_WEIGHTS = new Set(["thin", "light", "regular", "bold", "fill", "duotone"]);
export const OFFICIAL_SOURCE_HOSTS = new Set([
  "carbondesignsystem.com", "developer.android.com", "developer.apple.com",
  "developers.google.com", "fluent2.microsoft.design", "github.com",
  "greensock.com", "gsap.com", "m3.material.io", "opensource.adobe.com",
  "react.dev", "s2.spectrum.adobe.com", "shopify.dev",
  "design-system.service.gov.uk", "tailwindcss.com", "www.w3.org",
]);
/** Hosts accepted as the official Docs URL of a Critical/High stack row. */
export const STACK_OFFICIAL_HOSTS: Record<string, Set<string>> = {
  react: new Set(["react.dev"]),
  nextjs: new Set(["nextjs.org"]),
  vue: new Set(["vuejs.org", "pinia.vuejs.org"]),
  svelte: new Set(["svelte.dev", "kit.svelte.dev"]),
  astro: new Set(["docs.astro.build"]),
  angular: new Set(["angular.dev"]),
  "html-tailwind": new Set(["tailwindcss.com"]),
  shadcn: new Set(["ui.shadcn.com"]),
  m3: new Set(["m3.material.io", "material-web.dev", "github.com", "base-ui.com"]),
  "rg-ui": new Set(["github.com", "ui.shadcn.com", "base-ui.com", "tailwindcss.com", "nextjs.org"]),
  nuxtjs: new Set(["nuxt.com"]),
  "nuxt-ui": new Set(["ui.nuxt.com"]),
  "react-native": new Set(["reactnative.dev", "react.dev"]),
  flutter: new Set(["api.flutter.dev", "docs.flutter.dev"]),
  swiftui: new Set(["developer.apple.com"]),
  "jetpack-compose": new Set(["developer.android.com"]),
  avalonia: new Set(["docs.avaloniaui.net"]),
  uwp: new Set(["learn.microsoft.com"]),
  winui: new Set(["learn.microsoft.com"]),
  wpf: new Set(["learn.microsoft.com"]),
  uno: new Set(["platform.uno"]),
  javafx: new Set(["openjfx.io", "mkpaz.github.io", "www.w3.org"]),
  threejs: new Set(["threejs.org", "github.com", "www.npmjs.com", "www.w3.org"]),
  laravel: new Set(["laravel.com"]),
};
export const REQUIRED_UX_GUIDANCE: Record<string, string> = {
  "Focus Not Obscured (Minimum)": "Web",
  "Focus Not Obscured (Enhanced)": "Web",
  "Focus Appearance": "Web",
  "Dragging Movements": "All",
  "Target Size (Minimum)": "Web",
  "Consistent Help": "All",
  "Redundant Entry": "All",
  "Accessible Authentication (Minimum)": "All",
  "Auto-Rotating Content Controls": "All",
};
/** brands.csv contract (Aphrody brand themes extracted from the Bun OS CSS). */
export const BRAND_HEADER = [
  "No", "Brand ID", "Brand", "Theme", "Package", "Source File", "Selector", "Color Scheme",
  "Primary", "On Primary", "Secondary", "On Secondary", "Accent", "On Accent", "Background", "Foreground",
  "Card", "Card Foreground", "Muted", "Muted Foreground", "Border", "Destructive", "On Destructive", "Ring",
  "Radius", "Heading Font", "Body Font", "Mono Font", "Keywords", "Notes",
];
export const BRAND_COLOR_COLUMNS = BRAND_HEADER.slice(BRAND_HEADER.indexOf("Primary"), BRAND_HEADER.indexOf("Radius"));

type Json = any;

// ---------- helpers ----------
function relativeLuminance(value: string): number {
  if (!HEX_COLOR.test(value ?? "")) throw new Error(`invalid hex color '${value}'`);
  const channels = [1, 3, 5].map((index) => Number.parseInt(value.slice(index, index + 2), 16) / 255);
  const linear = channels.map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!;
}

/** WCAG contrast for two opaque six-digit sRGB colors. */
export function contrastRatio(foreground: string, background: string): number {
  const a = relativeLuminance(foreground);
  const b = relativeLuminance(background);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

const fileCache = new Map<string, Promise<{ headers: string[]; rows: Row[] }>>();
/** csv.DictReader-equivalent read: header + records (blank lines skipped). */
function readRows(path: string): Promise<{ headers: string[]; rows: Row[] }> {
  let cached = fileCache.get(path);
  if (!cached) {
    cached = Bun.file(path)
      .text()
      .then((text) => {
        const [headers = [], ...values] = parseCsv(text.startsWith("﻿") ? text.slice(1) : text);
        const rows = values
          .filter((fields) => fields.length > 0)
          .map((fields) => Object.fromEntries(headers.map((name, index) => [name, fields[index] ?? ""])) as Row);
        return { headers, rows };
      });
    fileCache.set(path, cached);
  }
  return cached;
}

const split = (value: string, delimiter: string) =>
  (value ?? "").split(delimiter).map((part) => part.trim()).filter(Boolean);
const casefold = (value: string) => (value ?? "").toLowerCase();
const sorted = (values: Iterable<string>) => [...values].sort();
const isSubset = <T>(a: Set<T>, b: Set<T>) => [...a].every((value) => b.has(value));
const setEquals = <T>(a: Set<T>, b: Set<T>) => a.size === b.size && isSubset(a, b);
const today = () => {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
};

function isIsoDate(value: unknown): value is string {
  if (typeof value !== "string") return false;
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return false;
  const date = new Date(Date.UTC(+match[1]!, +match[2]! - 1, +match[3]!));
  return date.getUTCFullYear() === +match[1]! && date.getUTCMonth() === +match[2]! - 1 && date.getUTCDate() === +match[3]!;
}

const validDate = (value: unknown) => isIsoDate(value) && value <= today();
const catalogDate = (value: unknown) => isIsoDate(value) && Number(value.slice(0, 4)) > 1970 && value <= today();
const validConfidence = (value: unknown) => value === null || value === undefined || (typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= 1);

function urlParts(value: string): { scheme: string; host: string | null; path: string; query: string; hasUserOrPort: boolean } {
  const match = /^([A-Za-z][A-Za-z0-9+.-]*):(?:\/\/([^/?#]*))?([^?#]*)(?:\?([^#]*))?/.exec(value ?? "");
  if (!match) return { scheme: "", host: null, path: value ?? "", query: "", hasUserOrPort: false };
  const authority = match[2];
  const hostPort = authority?.includes("@") ? authority.slice(authority.lastIndexOf("@") + 1) : authority;
  const host = hostPort === undefined ? null : hostPort.replace(/:\d*$/, "").toLowerCase() || null;
  return {
    scheme: match[1]!.toLowerCase(),
    host,
    path: match[3] ?? "",
    query: match[4] ?? "",
    hasUserOrPort: !!authority && (authority.includes("@") || /:\d+$/.test(authority)),
  };
}

/** urllib.parse.quote_plus with the default safe set. */
function quotePlus(value: string): string {
  return encodeURIComponent(value)
    .replace(/[!'()*]/g, (ch) => `%${ch.charCodeAt(0).toString(16).toUpperCase()}`)
    .replace(/%20/g, "+");
}

// ---------- per-file checks ----------
export async function checkFile(label: string, file: string, searchCols: readonly string[], outputCols: readonly string[], problems: string[]) {
  const path = join(DATA_DIR, file);
  const name = file.split("/").pop()!;
  if (!(await Bun.file(path).exists())) {
    problems.push(`[${label}] missing file: ${path}`);
    return null;
  }
  let parsed: { headers: string[]; rows: Row[] };
  try {
    parsed = await readRows(path);
  } catch (error) {
    problems.push(`[${label}] failed to parse ${name}: ${(error as Error).message}`);
    return null;
  }
  const headerSet = new Set(parsed.headers);
  for (const col of new Set([...searchCols, ...outputCols])) {
    if (!headerSet.has(col)) problems.push(`[${label}] ${name}: expected column '${col}' not found in header`);
  }
  // Only check duplicates against the sequential "No" identifier column.
  if (headerSet.has("No")) {
    const seen = new Map<string, number>();
    parsed.rows.forEach((row, index) => {
      const key = row.No ?? "";
      if (seen.has(key)) problems.push(`[${label}] ${name}: duplicate 'No' value '${key}' on rows ${seen.get(key)} and ${index + 2}`);
      else seen.set(key, index + 2);
    });
  } else if (label.startsWith("stack:")) {
    problems.push(
      `[${label}] ${name}: missing 'No' index column present in other stack files ` +
        "(schema drift -- harmless for search, but inconsistent with the rest of data/stacks/)",
    );
  }
  return parsed;
}

export function checkStyleContract(styles: Row[], products: Row[], reasoning: Row[], problems: string[], replacementTargets: Record<string, Set<string>> = {}) {
  const ids = new Set<string>();
  const lookup = new Map<string, string>();
  const parents = new Map<string, string>();
  const byId = new Map<string, Row>();
  for (const row of styles) {
    const styleId = row["Style ID"] ?? "";
    if (!STYLE_ID.test(styleId)) problems.push(`[style] invalid Style ID '${styleId}'`);
    if (ids.has(styleId)) problems.push(`[style] duplicate Style ID '${styleId}'`);
    ids.add(styleId);
    byId.set(styleId, row);
    const status = row.Status ?? "";
    const parent = row["Parent Style ID"] ?? "";
    if (!STYLE_STATUSES.has(status)) problems.push(`[style:${styleId}] invalid Status '${status}'`);
    const replacementDomain = row["Replacement Domain"] ?? "";
    const replacementId = row["Replacement ID"] ?? "";
    if (status === "supplemental" && !parent) problems.push(`[style:${styleId}] supplemental rows require Parent Style ID`);
    if (status === "deprecated") {
      const hasParent = !!parent;
      const hasRedirect = !!(replacementDomain && replacementId);
      if (hasParent === hasRedirect) problems.push(`[style:${styleId}] deprecated rows require exactly one parent or redirect`);
      if (hasRedirect && !replacementTargets[replacementDomain]?.has(replacementId)) {
        problems.push(`[style:${styleId}] invalid ${replacementDomain} redirect '${replacementId}'`);
      }
    } else if (replacementDomain || replacementId) {
      problems.push(`[style:${styleId}] only deprecated rows may redirect`);
    }
    parents.set(styleId, parent);
    for (const key of [styleId, row["Style Category"] ?? "", ...split(row.Aliases ?? "", "|")]) {
      const folded = casefold(key);
      if (lookup.has(folded) && lookup.get(folded) !== styleId) problems.push(`[style] ambiguous identity '${key}' -> ${lookup.get(folded)}, ${styleId}`);
      lookup.set(folded, styleId);
    }
  }
  for (const [styleId, parent] of parents) {
    if (parent && (!ids.has(parent) || parent === styleId)) problems.push(`[style:${styleId}] invalid parent '${parent}'`);
    const seen = new Set([styleId]);
    let current = parent;
    while (current) {
      if (seen.has(current)) {
        problems.push(`[style:${styleId}] parent cycle through '${current}'`);
        break;
      }
      seen.add(current);
      current = parents.get(current) ?? "";
    }
    if (parent && byId.get(parent)?.Status !== "active") problems.push(`[style:${styleId}] parent must target an active style`);
  }
  const references: string[] = [];
  for (const row of products) {
    references.push(...split(row["Primary Style Recommendation"] ?? "", "+"), ...split(row["Secondary Styles"] ?? "", ","));
  }
  for (const row of reasoning) references.push(...split(row.Style_Priority ?? "", "+"));
  for (const reference of sorted(new Set(references))) {
    const resolved = lookup.get(casefold(reference));
    if (!resolved) problems.push(`[style] unresolved reference '${reference}'`);
    else if (styles.find((row) => row["Style ID"] === resolved)?.Status === "deprecated") problems.push(`[style] reference targets deprecated style '${reference}'`);
  }

  const performanceLevels = new Set(["cost:low", "cost:moderate", "cost:high"]);
  const accessibilityLevels = new Set(["risk:low", "risk:conditional", "risk:high"]);
  const modeLevels = new Set(["supported", "conditional", "not-recommended"]);
  const promptLengths = new Map<string, number[]>();
  for (const row of styles) {
    const styleId = row["Style ID"] ?? "";
    if (!performanceLevels.has((row.Performance ?? "").split("|")[0]!)) problems.push(`[style:${styleId}] invalid Performance vocabulary`);
    if (!accessibilityLevels.has((row.Accessibility ?? "").split("|")[0]!)) problems.push(`[style:${styleId}] invalid Accessibility vocabulary`);
    if (WCAG_CONFORMANCE.test(Object.values(row).join(" "))) problems.push(`[style:${styleId}] accessibility conformance guarantee`);
    const compatibility = row["Framework Compatibility"] ?? "";
    if (/\d+\/10/.test(compatibility)) problems.push(`[style:${styleId}] framework score is unsupported`);
    if (["all frameworks", "excellent", "performant", "lightweight"].some((term) => casefold(compatibility).includes(term))) {
      problems.push(`[style:${styleId}] unsupported framework guarantee`);
    }
    for (const field of ["Light Mode ✓", "Dark Mode ✓"]) {
      if (!modeLevels.has(row[field] ?? "")) problems.push(`[style:${styleId}] invalid ${field} vocabulary`);
    }
    if (!["auto", "light", "dark"].includes(row["Preferred Mode"] ?? "")) problems.push(`[style:${styleId}] invalid Preferred Mode`);
    const length = (row["AI Prompt Keywords"] ?? "").split(/\s+/).filter(Boolean).length;
    const type = row.Type ?? "";
    promptLengths.set(type, [...(promptLengths.get(type) ?? []), length]);
    if (length > 40) problems.push(`[style:${styleId}] AI prompt exceeds 40 words`);
  }
  const median = (values: number[]) => {
    const s = [...values].sort((a, b) => a - b);
    const mid = Math.floor(s.length / 2);
    return s.length % 2 ? s[mid]! : (s[mid - 1]! + s[mid]!) / 2;
  };
  const general = promptLengths.get("General");
  const mobile = promptLengths.get("Mobile");
  if (general?.length && mobile?.length && median(mobile) > median(general) * 1.25) problems.push("[style] mobile prompt median exceeds 1.25x general median");
  return ids;
}

export function checkReasoningContract(products: Row[], colors: Row[], reasoning: Row[], styleIds: Set<string>, patterns: Set<string>, problems: string[]) {
  const semanticSets: [string, Row[], string][] = [
    ["products", products, "Product Type"],
    ["colors", colors, "Product Type"],
    ["reasoning", reasoning, "UI_Category"],
  ];
  for (const [label, rows, key] of semanticSets) {
    const counts = new Map<string, number>();
    for (const row of rows) counts.set(row[key] ?? "", (counts.get(row[key] ?? "") ?? 0) + 1);
    const duplicates = sorted([...counts].filter(([, count]) => count > 1).map(([value]) => value));
    if (rows.length !== 192) problems.push(`[reasoning] ${label} must contain exactly 192 rows; got ${rows.length}`);
    if (duplicates.length) problems.push(`[reasoning] duplicate ${label} labels: ${duplicates.join(", ")}`);
  }
  const productNames = new Set(products.map((row) => row["Product Type"] ?? ""));
  const colorNames = new Set(colors.map((row) => row["Product Type"] ?? ""));
  const reasoningNames = new Set(reasoning.map((row) => row.UI_Category ?? ""));
  if (!setEquals(productNames, colorNames)) problems.push("[reasoning] products/colors product labels differ");
  const missing = sorted([...productNames].filter((name) => !reasoningNames.has(name)));
  const extra = sorted([...reasoningNames].filter((name) => !productNames.has(name)));
  if (missing.length) problems.push(`[reasoning] missing exact product rows: ${missing.join(", ")}`);
  if (extra.length) problems.push(`[reasoning] unknown product rows: ${extra.join(", ")}`);
  for (const row of reasoning) {
    const category = row.UI_Category ?? "";
    let rules: Map<string, string[]>;
    try {
      rules = parseDecisionRules(row.Decision_Rules ?? "");
    } catch (error) {
      problems.push(`[reasoning:${category}] ${(error as Error).message}`);
      continue;
    }
    for (const actions of rules.values()) {
      for (const action of actions) {
        const index = action.indexOf(":");
        const prefix = action.slice(0, index);
        const value = action.slice(index + 1);
        if (prefix === "style" && !styleIds.has(value)) problems.push(`[reasoning:${category}] unknown style action '${value}'`);
        if (prefix === "pattern" && !patterns.has(value)) problems.push(`[reasoning:${category}] unknown pattern action '${value}'`);
      }
    }
    const pattern = row.Recommended_Pattern ?? "";
    if (!patterns.has(pattern)) problems.push(`[reasoning:${category}] unknown Recommended_Pattern '${pattern}'`);
    const confidence = row.Confidence ?? "";
    if (confidence) {
      const value = Number(confidence.trim());
      if (!confidence.trim() || Number.isNaN(value) || !(value >= 0 && value <= 1)) problems.push(`[reasoning:${category}] invalid Confidence '${confidence}'`);
    }
  }
}

export function checkColorContract(rows: Row[], problems: string[]) {
  for (const row of rows) {
    const product = row["Product Type"] ?? "";
    const ratios: Record<string, number> = {};
    for (const [foreground, [background, role, minimum]] of Object.entries(COLOR_CONTRAST_PAIRS)) {
      let ratio: number;
      try {
        ratio = contrastRatio(row[foreground] ?? "", row[background] ?? "");
      } catch (error) {
        problems.push(`[color:${product}] ${(error as Error).message}`);
        continue;
      }
      if (ratio + 1e-9 < minimum) problems.push(`[color:${product}] ${role} ${foreground}/${background} contrast ${ratio.toFixed(2)}:1 is below ${minimum.toFixed(1)}:1`);
      ratios[foreground] = ratio;
    }
    if ("Muted Foreground" in ratios && "Foreground" in ratios && ratios["Muted Foreground"]! > ratios.Foreground! + 1e-9) {
      problems.push(`[color:${product}] muted text contrast exceeds primary text contrast`);
    }
    const value = (row.Destructive ?? "").replace(/^#+/, "");
    const channels = [0, 2, 4].map((index) => value.slice(index, index + 2));
    if (channels.every((channel) => /^[0-9A-Fa-f]+$/.test(channel))) {
      const [red, green, blue] = channels.map((channel) => Number.parseInt(channel, 16)) as [number, number, number];
      if (green > red * 1.1 && green > blue * 1.1) problems.push(`[color:${product}] Destructive token is success green`);
    }
  }
}

export function checkChartContract(rows: Row[], problems: string[]) {
  for (const row of rows) {
    const dataType = row["Data Type"] ?? "";
    if (row["Accessibility Grade"] !== "deprecated: use Accessibility Risk") problems.push(`[chart:${dataType}] invalid deprecated Accessibility Grade`);
    if (!CHART_RISKS.has(row["Accessibility Risk"] ?? "")) problems.push(`[chart:${dataType}] invalid Accessibility Risk`);
    const fallback = casefold(`${row["Accessibility Notes"] ?? ""} ${row["A11y Fallback"] ?? ""}`);
    if (!CHART_TEXT_FALLBACK.test(fallback)) problems.push(`[chart:${dataType}] missing text/table/list fallback`);
    if (!CHART_NON_COLOR_GUIDANCE.test(fallback)) problems.push(`[chart:${dataType}] missing non-color distinction guidance`);
    if ((row["Interactive Level"] ?? "").trim() && !fallback.includes("keyboard")) problems.push(`[chart:${dataType}] missing keyboard interaction equivalent`);
    if (WCAG_GRADE.test(fallback)) problems.push(`[chart:${dataType}] fallback claims WCAG conformance`);
  }
}

/** parse_qs(urlsplit(url).query)["family"] (blank values dropped, '+' decoded as space). */
export function fontFamilies(url: string): string[] {
  const query = urlParts(url).query;
  if (!query) return [];
  const values: string[] = [];
  for (const pair of query.split("&")) {
    if (!pair) continue;
    const eq = pair.indexOf("=");
    if (eq === -1) continue;
    const key = decodeURIComponent(pair.slice(0, eq).replace(/\+/g, " "));
    const value = decodeURIComponent(pair.slice(eq + 1).replace(/\+/g, " "));
    if (key === "family" && value) values.push(value);
  }
  return values;
}

export const fontNames = (declarations: string[]) => new Set(declarations.map((declaration) => declaration.split(":")[0]!.replaceAll("+", " ")));
export const configuredFontNames = (config: string) => new Set([...config.matchAll(/'([^']+)'/g)].map((match) => match[1]!));

export function importedWeights(declarations: string[]): Set<string> {
  const weights = new Set<string>();
  for (const declaration of declarations) {
    if (!declaration.includes(":") || !declaration.includes("@")) continue;
    const axisValues = declaration.slice(declaration.indexOf(":") + 1);
    const at = axisValues.indexOf("@");
    if (at === -1) continue;
    const axes = axisValues.slice(0, at);
    const values = axisValues.slice(at + 1);
    if (axes.includes("wght")) for (const weight of values.match(FONT_WEIGHT) ?? []) weights.add(weight);
  }
  return weights;
}

export function declaredWeights(notes: string): Set<string> {
  const weights = new Set<string>();
  for (const match of notes.matchAll(/(?:weights?|strictly)[^.;]{0,100}/gi)) {
    for (const weight of match[0].match(FONT_WEIGHT) ?? []) weights.add(weight);
  }
  return weights;
}

export function checkTypographyContract(rows: Row[], problems: string[]) {
  for (const row of rows) {
    const pairing = row["Font Pairing Name"] ?? "";
    const urlFamilies = fontFamilies(row["Google Fonts URL"] ?? "");
    const families = fontNames(urlFamilies);
    const configured = configuredFontNames(row["Tailwind Config"] ?? "");
    const named = new Set([row["Heading Font"] ?? "", row["Body Font"] ?? ""]);
    if (!isSubset(named, families) || !isSubset(named, configured)) problems.push(`[typography:${pairing}] named/imported/configured fonts differ`);
    const importMatch = CSS_IMPORT.exec(row["CSS Import"] ?? "");
    if (!importMatch) {
      problems.push(`[typography:${pairing}] invalid CSS Import`);
      continue;
    }
    const importFamilies = fontFamilies(importMatch[2]!);
    if (JSON.stringify([...urlFamilies].sort()) !== JSON.stringify([...importFamilies].sort())) problems.push(`[typography:${pairing}] Google URL and CSS Import differ`);
    const imported = importedWeights(urlFamilies);
    const declared = declaredWeights(row.Notes ?? "");
    if (declared.size && !isSubset(declared, imported)) {
      problems.push(`[typography:${pairing}] recommended weights not imported: ${sorted([...declared].filter((w) => !imported.has(w))).join(", ")}`);
    }
  }
}

async function loadCatalogJson(name: string, problems: string[]): Promise<Json> {
  let payload: Json;
  try {
    payload = JSON.parse(await Bun.file(join(DATA_DIR, name)).text());
  } catch (error) {
    problems.push(`[catalog:${name}] invalid JSON: ${(error as Error).message}`);
    return {};
  }
  if (typeof payload !== "object" || payload === null || Array.isArray(payload) || payload.schemaVersion !== 1) {
    problems.push(`[catalog:${name}] schemaVersion 1 object is required`);
    return {};
  }
  return payload;
}

export function validGoogleFontsExclusionSource(value: unknown): boolean {
  if (typeof value !== "string") return false;
  const parts = urlParts(value);
  if (parts.scheme !== "https" || parts.hasUserOrPort) return false;
  if (parts.host === "fonts.google.com") return parts.path !== "";
  return parts.host === "github.com" && (parts.path === "/google/fonts" || parts.path.startsWith("/google/fonts/"));
}

const isObject = (value: unknown): value is Record<string, Json> => typeof value === "object" && value !== null && !Array.isArray(value);

export function checkFontCatalog(rows: Row[], licenses: Json, typography: Row[], problems: string[]) {
  const families = rows.map((row) => row.Family ?? "");
  const familySet = new Set(families);
  if (!families.length || familySet.size !== families.length || familySet.has("")) problems.push("[catalog:google-fonts] Family values must be non-empty and unique");
  const available = new Map<string, Set<string>>();
  for (const row of rows) {
    const family = row.Family ?? "";
    if (!catalogDate(row["Date Added"]) || !catalogDate(row["Last Modified"])) problems.push(`[catalog:google-fonts:${family}] invalid or suspicious date`);
    if (row["Google Fonts URL"] !== `https://fonts.google.com/specimen/${quotePlus(family)}`) problems.push(`[catalog:google-fonts:${family}] invalid specimen URL`);
    const styles = new Set(split(row.Styles ?? "", "|"));
    if (!styles.size) problems.push(`[catalog:google-fonts:${family}] Styles cannot be empty`);
    available.set(family, new Set([...styles].map((style) => (style.endsWith("i") ? style.slice(0, -1) : style))));
  }
  const source = licenses.source;
  if (!isObject(source) || source.repository !== "https://github.com/google/fonts" || source.metadataFile !== "METADATA.pb" || !GOOGLE_FONTS_REVISION.test(String(source.revision ?? ""))) {
    problems.push("[catalog:google-font-licenses] invalid source revision");
  }
  const entries = licenses.families;
  const excluded = licenses.excludedFamilies;
  if (licenses.familyCount !== rows.length || !Array.isArray(entries) || !Array.isArray(excluded)) {
    problems.push("[catalog:google-font-licenses] invalid counts or arrays");
    return;
  }
  const licensedNames = new Set<unknown>();
  for (const item of entries) {
    if (!isObject(item)) {
      problems.push("[catalog:google-font-licenses] family entry must be an object");
      continue;
    }
    const name = item.name;
    if (
      typeof name !== "string" || !name || licensedNames.has(name) || !FONT_LICENSES.has(item.license) || item.status !== "active" ||
      !catalogDate(item.date_added) || !catalogDate(item.verifiedAt)
    ) {
      problems.push(`[catalog:google-font-licenses:${name}] invalid active family`);
    }
    licensedNames.add(name);
  }
  if (!setEquals(licensedNames, familySet as Set<unknown>)) problems.push("[catalog:google-font-licenses] active families must match google-fonts.csv");
  const excludedNames = new Set<unknown>();
  for (const item of excluded) {
    const name = isObject(item) ? item.name : undefined;
    const sourceUrl = isObject(item) ? item.source : undefined;
    if (
      typeof name !== "string" || !name || excludedNames.has(name) || familySet.has(name) || item.status !== "needs-review" ||
      typeof item.reason !== "string" || !item.reason || !validGoogleFontsExclusionSource(sourceUrl) || !catalogDate(item.verifiedAt)
    ) {
      problems.push(`[catalog:google-font-licenses:${name}] invalid exclusion`);
    }
    excludedNames.add(name);
  }
  for (const row of typography) {
    const pairing = row["Font Pairing Name"] ?? "";
    for (const declaration of fontFamilies(row["Google Fonts URL"] ?? "")) {
      const family = declaration.split(":")[0]!.replaceAll("+", " ");
      const styles = available.get(family);
      if (!styles) {
        problems.push(`[typography:${pairing}] font absent from approved catalog: ${family}`);
        continue;
      }
      let weights = importedWeights([declaration]);
      if (!weights.size) weights = new Set(["400"]);
      if (!isSubset(weights, styles)) problems.push(`[typography:${pairing}] catalog lacks ${family} weights: ${sorted([...weights].filter((w) => !styles.has(w))).join(", ")}`);
    }
  }
}

export function checkPhosphorCatalog(curated: Row[], manifest: Json, problems: string[]) {
  const source = isObject(manifest.source) ? manifest.source : {};
  const imports = isObject(manifest.reactImports) ? manifest.reactImports : {};
  const icons = manifest.icons;
  if (source.package !== "@phosphor-icons/core" || source.version !== "2.1.1") problems.push("[catalog:phosphor] unpinned core package");
  if (source.reactPackage !== "@phosphor-icons/react" || source.reactVersion !== "2.1.10") problems.push("[catalog:phosphor] unpinned React package");
  const importsOk =
    Object.keys(imports).length === 2 && imports.clientModule === "@phosphor-icons/react" && imports.ssrModule === "@phosphor-icons/react/ssr";
  if (
    manifest.status !== "active" || !catalogDate(manifest.verifiedAt) || !setEquals(new Set(manifest.weights ?? []), PHOSPHOR_WEIGHTS) ||
    !importsOk || !Array.isArray(icons) || manifest.iconCount !== icons.length
  ) {
    problems.push("[catalog:phosphor] invalid snapshot metadata");
    return;
  }
  const names = new Map<string, string>();
  const components = new Set<string>();
  for (const item of icons) {
    const name = isObject(item) ? item.name : undefined;
    const component = isObject(item) ? item.component : undefined;
    if (
      !name || names.has(name) || !component || components.has(component) ||
      item.clientImport !== `import { ${component} } from "@phosphor-icons/react"` || item.ssrImport !== `import { ${component} } from "@phosphor-icons/react/ssr"`
    ) {
      problems.push(`[catalog:phosphor:${name}] invalid identity or imports`);
    }
    names.set(name, component);
    components.add(component);
  }
  const phosphorRows = curated.filter((row) => row.Library === "Phosphor");
  if (manifest.curatedValidatedCount !== phosphorRows.length) problems.push("[catalog:phosphor] curated validation count is stale");
  for (const row of phosphorRows) {
    const name = row["Icon Name"] ?? "";
    const component = /import\s*\{\s*([A-Za-z0-9]+)/.exec(row["Import Code"] ?? "")?.[1] ?? "";
    if (names.get(name) !== component) problems.push(`[catalog:phosphor:${name}] curated icon is absent or mismatched`);
  }
}

async function digest(path: string): Promise<string> {
  const bytes = new Uint8Array(await Bun.file(path).arrayBuffer());
  const out: number[] = [];
  for (let i = 0; i < bytes.length; i++) if (!(bytes[i] === 0x0d && bytes[i + 1] === 0x0a)) out.push(bytes[i]!);
  return new Bun.CryptoHasher("sha256").update(new Uint8Array(out)).digest("hex");
}

export async function checkCatalogSummary(summary: Json, licenses: Json, phosphor: Json, problems: string[]) {
  if (!catalogDate(summary.verifiedAt)) problems.push("[catalog:summary] invalid verifiedAt");
  const counts = isObject(summary.counts) ? summary.counts : {};
  const count = async (name: string) => (await readRows(join(DATA_DIR, name))).rows.length;
  const styles = (await readRows(join(DATA_DIR, "styles.csv"))).rows;
  let stackGuidelines = 0;
  for (const config of Object.values(STACK_CONFIG)) stackGuidelines += await count(config.file);
  const expected: Record<string, unknown> = {
    styles: {
      total: styles.length,
      searchable: styles.filter((row) => row.Status !== "deprecated").length,
      active: styles.filter((row) => row.Status === "active").length,
      supplemental: styles.filter((row) => row.Status === "supplemental").length,
      deprecated: styles.filter((row) => row.Status === "deprecated").length,
    },
    products: await count("products.csv"),
    palettes: await count("colors.csv"),
    reasoningProfiles: await count(REASONING_FILE),
    fontPairings: await count("typography.csv"),
    googleFonts: await count("google-fonts.csv"),
    curatedIcons: await count("icons.csv"),
    upstreamPhosphorIcons: phosphor.iconCount,
    uxGuidelines: await count("ux-guidelines.csv"),
    motionPresets: await count("motion.csv"),
    chartTypes: await count("charts.csv"),
    stacks: Object.keys(STACK_CONFIG).length,
    stackGuidelines,
  };
  if (await Bun.file(join(DATA_DIR, "brands.csv")).exists()) expected.brands = await count("brands.csv");
  for (const [key, value] of Object.entries(expected)) {
    if (JSON.stringify(counts[key]) !== JSON.stringify(value)) problems.push(`[catalog:summary] stale count for ${key}`);
  }
  const snapshots = isObject(summary.snapshots) ? summary.snapshots : {};
  for (const name of ["google-fonts.csv", "google-font-licenses.json", "icons.csv", "phosphor-icons-upstream.json"]) {
    // Line endings are normalized so the check matches generate-catalog-summary.ts on CRLF checkouts too.
    const snapshot = snapshots[name];
    if (!isObject(snapshot) || Object.keys(snapshot).length !== 1 || snapshot.sha256 !== (await digest(join(DATA_DIR, name)))) {
      problems.push(`[catalog:summary] stale snapshot for ${name}`);
    }
  }
  const policy = summary.promotionPolicy;
  if (
    !isObject(policy) || Object.keys(policy).length !== 3 || policy.changedFamilySetRequiresExplicitApproval !== true ||
    policy.relevanceGateRequired !== true || policy.unlicensedFamiliesExcluded !== true
  ) {
    problems.push("[catalog:summary] invalid promotion policy");
  }
  const pending = (licenses.excludedFamilies ?? [])
    .filter(isObject)
    .map((item: Json) => ({ family: item.name ?? null, reason: item.reason ?? null }))
    .sort((a: Json, b: Json) => {
      const x = casefold(a.family ?? "");
      const y = casefold(b.family ?? "");
      return x < y ? -1 : x > y ? 1 : 0;
    });
  if (JSON.stringify(summary.pendingCandidates) !== JSON.stringify(pending)) problems.push("[catalog:summary] pending candidates do not match exclusions");
}

export async function checkCatalogContract(domainRows: Record<string, Row[]>, problems: string[]) {
  if (await Bun.file(join(DATA_DIR, ".google-font-refresh.incomplete.json")).exists()) problems.push("[catalog:google-fonts] incomplete refresh marker requires review");
  const licenses = await loadCatalogJson("google-font-licenses.json", problems);
  const phosphor = await loadCatalogJson("phosphor-icons-upstream.json", problems);
  const summary = await loadCatalogJson("catalog-summary.json", problems);
  if (Object.keys(licenses).length) checkFontCatalog(domainRows["google-fonts"] ?? [], licenses, domainRows.typography ?? [], problems);
  if (Object.keys(phosphor).length) checkPhosphorCatalog(domainRows.icons ?? [], phosphor, problems);
  if (Object.keys(summary).length) await checkCatalogSummary(summary, licenses, phosphor, problems);
}

export function checkIconContract(rows: Row[], problems: string[]) {
  for (const row of rows) {
    const icon = row["Icon Name"] ?? "";
    const role = row["Semantic Role"] ?? "";
    const contexts = new Set(split(row["Allowed Contexts"] ?? "", "|"));
    const usage = row.Usage ?? "";
    if (!ICON_ROLES.has(role)) problems.push(`[icons:${icon}] invalid Semantic Role '${role}'`);
    if (!setEquals(contexts, ICON_CONTEXTS)) problems.push(`[icons:${icon}] incomplete contextual semantics`);
    if (/[㐀-鿿]/.test(usage)) problems.push(`[icons:${icon}] Usage must use canonical English`);
    const imports = row["Import Code"] ?? "";
    if (imports.includes("IconName") || !ICON_IMPORT.test(imports)) problems.push(`[icons:${icon}] invalid or placeholder icon import`);
    if (ICON_USAGE_REQUIREMENTS.some((pattern) => !pattern.test(usage))) problems.push(`[icons:${icon}] incomplete contextual accessibility guidance`);
  }
}

export function checkUxContract(rows: Row[], problems: string[]) {
  const byIssue = new Map(rows.map((row) => [row.Issue ?? "", row]));
  for (const issue of sorted(Object.keys(REQUIRED_UX_GUIDANCE).filter((issue) => !byIssue.has(issue)))) problems.push(`[ux] missing WCAG 2.2 guidance '${issue}'`);
  for (const [issue, platform] of Object.entries(REQUIRED_UX_GUIDANCE)) {
    const row = byIssue.get(issue);
    if (row && (row.Platform !== platform || !["Medium", "High", "Critical"].includes(row.Severity ?? ""))) problems.push(`[ux:${issue}] shifted or invalid semantic fields`);
  }
}

export function checkMotionContract(rows: Row[], problems: string[]) {
  for (const row of rows) {
    const text = casefold(Object.values(row).join(" "));
    if (!text.includes("reduced-motion") && !text.includes("user-controlled")) problems.push(`[motion:${row.No}] missing explicit motion opt-out`);
  }
}

export function checkAppInterfaceContract(rows: Row[], problems: string[]) {
  const target = rows.find((row) => row.Issue === "Touch Target Size") ?? {};
  const found = new Set(Object.values(target).join(" ").match(/44pt|48dp/g) ?? []);
  if (!found.has("44pt") || !found.has("48dp")) problems.push("[web:Touch Target Size] must distinguish iOS 44pt and Android 48dp");
}

export function checkReactContract(rows: Row[], problems: string[]) {
  const reactText = rows.map((row) => Object.values(row).join(" ")).join("\n");
  if (reactText.includes("useLatest")) problems.push("[react] unqualified community useLatest guidance is not allowed");
  const effectEvent = rows.find((row) => row.Issue === "Effect Events") ?? {};
  const effectText = casefold(Object.values(effectEvent).join(" "));
  if (!["inside effects", "dependencies"].every((phrase) => effectText.includes(phrase))) problems.push("[react:Effect Events] current scope/dependency guidance is required");
}

export function checkLandingClaims(rows: Row[], problems: string[]) {
  for (const row of rows) {
    const optimization = row["Conversion Optimization"] ?? "";
    if (optimization.includes("%") || LANDING_QUANTIFIED_CLAIM.test(optimization) || casefold(optimization).includes("best conversion")) {
      problems.push(`[landing:${row["Pattern Name"]}] unsupported quantitative claim`);
    }
  }
}

/** Aphrody brand themes: exact header, unique ids, #RRGGBB colors, light/dark scheme, real source file. */
export function checkBrandContract(headers: string[], rows: Row[], problems: string[]) {
  if (JSON.stringify(headers) !== JSON.stringify(BRAND_HEADER)) problems.push(`[brand] brands.csv header must be: ${BRAND_HEADER.join(",")}`);
  const ids = new Set<string>();
  for (const row of rows) {
    const id = row["Brand ID"] ?? "";
    if (!STYLE_ID.test(id)) problems.push(`[brand] invalid Brand ID '${id}'`);
    if (ids.has(id)) problems.push(`[brand] duplicate Brand ID '${id}'`);
    ids.add(id);
    for (const column of ["Brand", "Theme", "Package", "Source File", "Selector"]) {
      if (!(row[column] ?? "").trim()) problems.push(`[brand:${id}] ${column} is required`);
    }
    if (!["light", "dark"].includes(row["Color Scheme"] ?? "")) problems.push(`[brand:${id}] Color Scheme must be light or dark`);
    for (const column of BRAND_COLOR_COLUMNS) {
      const value = row[column] ?? "";
      if (value && !HEX_COLOR.test(value)) problems.push(`[brand:${id}] ${column} must be #RRGGBB, got '${value}'`);
    }
    for (const column of ["Primary", "Background", "Foreground"]) {
      if (!row[column]) problems.push(`[brand:${id}] ${column} is required`);
    }
    if (/^(?:[A-Za-z]:|\/|https?:)/.test(row["Source File"] ?? "")) problems.push(`[brand:${id}] Source File must be a repository-relative path`);
  }
}

export function checkCoreDataContract(domainRows: Record<string, Row[]>, problems: string[]) {
  checkColorContract(domainRows.color ?? [], problems);
  checkChartContract(domainRows.chart ?? [], problems);
  checkTypographyContract(domainRows.typography ?? [], problems);
  checkIconContract(domainRows.icons ?? [], problems);
  checkUxContract(domainRows.ux ?? [], problems);
  checkMotionContract(domainRows.gsap ?? [], problems);
  checkAppInterfaceContract(domainRows.web ?? [], problems);
  checkReactContract(domainRows.react ?? [], problems);
  checkLandingClaims(domainRows.landing ?? [], problems);
}

export function validProvenanceSource(source: unknown, sourceIndex: number, identity: string, problems: string[]): boolean {
  if (!isObject(source)) {
    problems.push(`[provenance] source ${sourceIndex} for ${identity} must be an object`);
    return false;
  }
  const { type, ref } = source;
  if (!PROVENANCE_SOURCE_TYPES.has(type) || typeof ref !== "string") {
    problems.push(`[provenance] invalid source for ${identity}`);
    return false;
  }
  if (type === "official") {
    const parts = urlParts(ref);
    if (parts.scheme !== "https" || !parts.host || !OFFICIAL_SOURCE_HOSTS.has(parts.host)) {
      problems.push(`[provenance] unapproved official source for ${identity}`);
      return false;
    }
  } else if (!ref || urlParts(ref).scheme) {
    problems.push(`[provenance] derived source must use a local dataset reference for ${identity}`);
    return false;
  }
  return true;
}

/** Validate curated-stack applicability and official high-impact sources. */
export function checkStackFreshnessContract(stack: string, rows: Row[], problems: string[]) {
  const hosts = STACK_OFFICIAL_HOSTS[stack];
  if (!hosts) return;
  let activeCount = 0;
  for (const row of rows) {
    const identity = `[stack:${stack}:${row.No ?? "?"}]`;
    const status = row.Status ?? "";
    const appliesTo = casefold((row["Applies To"] ?? "").trim());
    const expected = STACK_CURRENT_APPLICABILITY[stack] ?? stack;
    if (!appliesTo || !appliesTo.startsWith(stack)) problems.push(`${identity} Applies To must start with '${stack}'`);
    else if (status === "active" && !appliesTo.startsWith(expected)) problems.push(`${identity} Applies To must target '${expected}'`);
    if (status === "active") {
      activeCount++;
      if (appliesTo.includes("legacy")) problems.push(`${identity} active row cannot target legacy versions`);
    } else if (status === "deprecated" && !appliesTo.includes("legacy")) {
      problems.push(`${identity} deprecated row must be visibly legacy`);
    } else if (status === "unverified") {
      problems.push(`${identity} curated stack row cannot remain unverified`);
    }
    if (!["Critical", "High"].includes(row.Severity ?? "")) continue;
    const parts = urlParts(row["Docs URL"] ?? "");
    if (parts.scheme !== "https" || !parts.host || !hosts.has(parts.host)) problems.push(`${identity} Critical/High row requires an official Docs URL`);
    if (!validDate(row["Verified At"] ?? "")) problems.push(`${identity} Critical/High row requires ISO Verified At`);
  }
  if (!activeCount && stack !== "uwp") problems.push(`[stack:${stack}] requires at least one active current row`);
}

export async function validDatasetSourceKey(sourceFile: string, sourceKey: Json, identity: string, problems: string[]): Promise<boolean> {
  if (!CORE_PROVENANCE_FILES.has(sourceFile)) {
    problems.push(`[provenance] unknown dataset-contract sourceFile for ${identity}`);
    return false;
  }
  let parsed: { headers: string[]; rows: Row[] };
  try {
    parsed = await readRows(join(DATA_DIR, sourceFile));
  } catch {
    problems.push(`[provenance] unreadable sourceFile for ${identity}`);
    return false;
  }
  const scope = isObject(sourceKey) ? sourceKey.Scope : undefined;
  if (typeof scope !== "string" || !scope.startsWith("No ")) {
    problems.push(`[provenance] dataset scope must bind rows for ${identity}`);
    return false;
  }
  const rowPart = scope.split(";")[0]!;
  const referenced = new Set((rowPart.match(/\b\d+\b/g) ?? []).map(Number));
  for (const [, start, end] of rowPart.matchAll(/(\d+)\s*-\s*(\d+)/g)) {
    for (let value = Number(start); value <= Number(end); value++) referenced.add(value);
  }
  const rowIds = new Set(parsed.rows.filter((row) => /^\d+$/.test(row.No ?? "")).map((row) => Number(row.No)));
  if (!referenced.size || !isSubset(referenced, rowIds)) {
    problems.push(`[provenance] dataset scope references unknown rows for ${identity}`);
    return false;
  }
  const semanticHeaders = parsed.headers.filter((header) => header !== "No");
  const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  if (!semanticHeaders.some((header) => new RegExp(`\\b${escape(header)}\\b`, "i").test(scope))) {
    problems.push(`[provenance] dataset scope must bind source fields for ${identity}`);
    return false;
  }
  return true;
}

export async function validCatalogSourceKey(sourceFile: string, sourceKey: Json, identity: string, problems: string[]): Promise<boolean> {
  if (!CATALOG_PROVENANCE_FILES.has(sourceFile)) {
    problems.push(`[provenance] unknown catalog sourceFile for ${identity}`);
    return false;
  }
  if (!(await Bun.file(join(DATA_DIR, sourceFile)).exists())) {
    problems.push(`[provenance] missing catalog sourceFile for ${identity}`);
    return false;
  }
  const snapshot = isObject(sourceKey) ? sourceKey.Snapshot : undefined;
  const count = isObject(sourceKey) ? sourceKey.Count : undefined;
  if (snapshot !== "catalog-summary.json" || typeof count !== "number" || !Number.isInteger(count) || count <= 0) {
    problems.push(`[provenance] catalog sourceKey must bind snapshot and count for ${identity}`);
    return false;
  }
  let expectedCount: unknown;
  try {
    expectedCount = sourceFile.endsWith(".csv")
      ? (await readRows(join(DATA_DIR, sourceFile))).rows.length
      : JSON.parse(await Bun.file(join(DATA_DIR, sourceFile)).text()).iconCount;
  } catch {
    problems.push(`[provenance] unreadable catalog sourceFile for ${identity}`);
    return false;
  }
  if (count !== expectedCount) {
    problems.push(`[provenance] catalog count is stale for ${identity}`);
    return false;
  }
  return true;
}

const pyTuple = (kind: unknown, id: unknown) => `(${kind === undefined || kind === null ? "None" : `'${kind}'`}, ${id === undefined || id === null ? "None" : `'${id}'`})`;

export async function checkProvenance(reasoning: Row[], styles: Row[], problems: string[], provenancePath = join(DATA_DIR, "data-provenance.json")) {
  let payload: Json;
  try {
    payload = JSON.parse(await Bun.file(provenancePath).text());
  } catch (error) {
    problems.push(`[provenance] invalid data-provenance.json: ${(error as Error).message}`);
    return;
  }
  if (!isObject(payload) || payload.schemaVersion !== 1 || !Array.isArray(payload.records)) {
    problems.push("[provenance] schemaVersion 1 and records array are required");
    return;
  }
  const covered = new Set<string>();
  const coveredStyles = new Set<string>();
  const identities = new Set<string>();
  const validRecords: Json[] = [];
  for (const [index, record] of payload.records.entries()) {
    if (!isObject(record)) {
      problems.push(`[provenance] record ${index} must be an object`);
      continue;
    }
    const kind = record.entityKind;
    const entityId = record.entityId;
    if (!PROVENANCE_KINDS.has(kind) || typeof entityId !== "string" || !entityId) {
      problems.push(`[provenance] invalid identity at record ${index}`);
      continue;
    }
    const identity = pyTuple(kind, entityId);
    if (identities.has(identity)) problems.push(`[provenance] duplicate identity ${identity}`);
    identities.add(identity);
    if (!PROVENANCE_STATUSES.has(record.status)) problems.push(`[provenance] invalid status for ${identity}`);
    if (!PROVENANCE_SLAS.has(record.sla)) problems.push(`[provenance] invalid sla for ${identity}`);
    if (!validDate(record.verifiedAt)) problems.push(`[provenance] invalid verifiedAt for ${identity}`);
    if (typeof record.sourceFile !== "string" || !record.sourceFile) problems.push(`[provenance] sourceFile required for ${identity}`);
    if (!isObject(record.sourceKey) || !Object.keys(record.sourceKey).length) problems.push(`[provenance] sourceKey object required for ${identity}`);
    const appliesTo = record.appliesTo;
    if (!Array.isArray(appliesTo) || !appliesTo.length || appliesTo.some((value) => !PROVENANCE_APPLIES_TO.has(value))) problems.push(`[provenance] invalid appliesTo for ${identity}`);
    if (!validConfidence(record.confidence)) problems.push(`[provenance] invalid confidence for ${identity}`);
    let sources = record.sources;
    if (!Array.isArray(sources) || !sources.length) {
      problems.push(`[provenance] sources required for ${identity}`);
      sources = [];
    }
    const validSources = (sources as unknown[]).filter((source, sourceIndex) => validProvenanceSource(source, sourceIndex, identity, problems)) as Json[];
    const sourceTypes = new Set(validSources.map((source) => source.type));
    if (record.sla === "manual-verified" && [...sourceTypes].every((type) => type === "derived")) {
      problems.push(`[provenance] ${identity} cannot be manual-verified from derived sources only`);
    }
    const sourceKey = isObject(record.sourceKey) ? record.sourceKey : {};
    if (kind === "reasoning" && sourceKey.UI_Category) covered.add(sourceKey.UI_Category);
    if (kind === "style" && sourceKey["Style ID"]) coveredStyles.add(sourceKey["Style ID"]);
    let sourceKeyValid = true;
    if (kind === "dataset-contract") sourceKeyValid = await validDatasetSourceKey(record.sourceFile, sourceKey, identity, problems);
    else if (kind === "catalog-snapshot") sourceKeyValid = await validCatalogSourceKey(record.sourceFile, sourceKey, identity, problems);
    if (validSources.length && sourceKeyValid) validRecords.push(record);
  }
  const newRows = new Set(reasoning.filter((row) => Number.parseInt(row.No ?? "0", 10) >= 162).map((row) => row.UI_Category ?? ""));
  const missingRows = sorted([...newRows].filter((row) => !covered.has(row)));
  if (missingRows.length) problems.push(`[provenance] missing new reasoning rows: ${missingRows.join(", ")}`);
  const newStyles = new Set(styles.filter((row) => Number.parseInt(row.No ?? "0", 10) > 85).map((row) => row["Style ID"] ?? ""));
  const missingStyles = sorted([...newStyles].filter((style) => !coveredStyles.has(style)));
  if (missingStyles.length) problems.push(`[provenance] missing new style rows: ${missingStyles.join(", ")}`);
  const hasOfficial = (record: Json) => (record.sources ?? []).some((source: Json) => isObject(source) && source.type === "official");
  const coreFiles = new Set(validRecords.filter((record) => record.entityKind === "dataset-contract" && hasOfficial(record)).map((record) => record.sourceFile));
  const missingCore = sorted([...CORE_PROVENANCE_FILES].filter((file) => !coreFiles.has(file)));
  if (missingCore.length) problems.push(`[provenance] missing official core dataset records: ${missingCore.join(", ")}`);
  const catalogIds = new Set(validRecords.filter((record) => record.entityKind === "catalog-snapshot" && hasOfficial(record)).map((record) => record.entityId));
  const missingCatalogs = sorted([...CATALOG_PROVENANCE_IDS].filter((id) => !catalogIds.has(id)));
  if (missingCatalogs.length) problems.push(`[provenance] missing official catalog snapshots: ${missingCatalogs.join(", ")}`);
}

/** Return every semantic data problem without terminating the process. */
export async function validate(): Promise<string[]> {
  const problems: string[] = [];
  const domainRows: Record<string, Row[]> = {};
  const domainHeaders: Record<string, string[]> = {};
  for (const [domain, config] of Object.entries(CSV_CONFIG) as [string, { file: string; searchCols: readonly string[]; outputCols: readonly string[] }][]) {
    const parsed = await checkFile(`domain:${domain}`, config.file, config.searchCols, config.outputCols, problems);
    domainRows[domain] = parsed?.rows ?? [];
    domainHeaders[domain] = parsed?.headers ?? [];
  }
  for (const [stack, config] of Object.entries(STACK_CONFIG)) {
    const parsed = await checkFile(`stack:${stack}`, config.file, STACK_COLS.searchCols, STACK_COLS.outputCols, problems);
    const headers = parsed?.headers ?? [];
    const rows = parsed?.rows ?? [];
    const required = ["Applies To", "Status", "Verified At"];
    const missing = required.filter((field) => !headers.includes(field)).sort();
    if (headers.length && missing.length) problems.push(`[stack:${stack}] missing contract fields [${missing.map((f) => `'${f}'`).join(", ")}]`);
    for (const row of rows) {
      const status = row.Status ?? "";
      if (!STACK_STATUSES.has(status)) problems.push(`[stack:${stack}] invalid Status '${status}'`);
      if (status !== "unverified" && !validDate(row["Verified At"] ?? "")) problems.push(`[stack:${stack}] active rows require ISO Verified At`);
    }
    checkStackFreshnessContract(stack, rows, problems);
  }

  let reasoning: Row[] = [];
  const reasoningPath = join(DATA_DIR, REASONING_FILE);
  if (await Bun.file(reasoningPath).exists()) {
    reasoning = (await checkFile("reasoning", REASONING_FILE, ["UI_Category"], ["UI_Category", "Decision_Rules", "Reasoning", "Confidence"], problems))?.rows ?? [];
  } else {
    problems.push(`[reasoning] missing file: ${reasoningPath}`);
  }

  const styles = domainRows.style ?? [];
  const products = domainRows.product ?? [];
  const colors = domainRows.color ?? [];
  const landing = domainRows.landing ?? [];
  const patterns = new Set(landing.map((row) => row["Pattern Name"] ?? ""));
  for (const row of landing) for (const alias of split(row.Aliases ?? "", "|")) patterns.add(alias);
  const patternIds = new Set(landing.map((row) => row["Pattern ID"] ?? ""));
  if (patternIds.size !== landing.length || patternIds.has("")) problems.push("[landing] Pattern ID values must be non-empty and unique");
  const landingIdentities = new Map<string, string>();
  for (const row of landing) {
    for (const identity of [row["Pattern ID"] ?? "", row["Pattern Name"] ?? "", ...split(row.Aliases ?? "", "|")]) {
      const folded = casefold(identity.trim());
      const owner = landingIdentities.get(folded);
      if (!folded) problems.push("[landing] Pattern Name values must be non-empty");
      else if (owner && owner !== row["Pattern ID"]) problems.push(`[landing] ambiguous identity '${identity}' -> ${owner}, ${row["Pattern ID"]}`);
      else landingIdentities.set(folded, row["Pattern ID"] ?? "");
    }
  }
  const styleIds = checkStyleContract(styles, products, reasoning, problems, {
    landing: patternIds,
    style: new Set(styles.map((row) => row["Style ID"] ?? "")),
  });
  checkReasoningContract(products, colors, reasoning, styleIds, patterns, problems);
  checkCoreDataContract(domainRows, problems);
  await checkCatalogContract(domainRows, problems);
  for (const row of landing) {
    const sections = (row["Section Order"] ?? "").split(" > ");
    if (sections.length < 2 || sections.some((part) => /^\d+\.\s/.test(part))) problems.push(`[landing:${row["Pattern Name"]}] invalid Section Order delimiter`);
  }
  if ("brand" in CSV_CONFIG) checkBrandContract(domainHeaders.brand ?? [], domainRows.brand ?? [], problems);
  await checkProvenance(reasoning, styles, problems);
  return problems;
}

if (import.meta.main) {
  const problems = await validate();
  if (problems.length) {
    console.log(`FAILED: ${problems.length} data integrity issue(s) found:\n`);
    for (const problem of problems) console.log(`  - ${problem}`);
    process.exit(1);
  }
  console.log(`OK: validated ${Object.keys(CSV_CONFIG).length} domain files, ${Object.keys(STACK_CONFIG).length} stack files, and ui-reasoning.csv`);
}
