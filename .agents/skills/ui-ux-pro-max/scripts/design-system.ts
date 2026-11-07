/**
 * Design System Generator: aggregates search results and applies reasoning
 * rules to produce a complete design-system recommendation.
 *
 *   const result = await generateDesignSystem("SaaS dashboard", { projectName: "My Project" });
 *   console.log(result.text);
 *
 * Persistence (Master + Overrides pattern):
 *   await generateDesignSystem("SaaS dashboard", { projectName: "My Project", persist: true, outputDir: root });
 */

import { link, mkdir, rename, unlink } from "node:fs/promises";
import { dirname, join } from "node:path";
import { DATA_DIR, findBrand, loadBrands, loadCsv, search, type Row, type SearchResult } from "./core.ts";
import { parseCsvRecords } from "./csv.ts";
import { applyDecisionRules, decisionRulesToObject, parseDecisionRules } from "./reasoning-contract.ts";

// ============ CONFIGURATION ============
const REASONING_FILE = "ui-reasoning.csv";

const SEARCH_CONFIG = {
  product: { maxResults: 1 },
  style: { maxResults: 3 },
  color: { maxResults: 5 },
  landing: { maxResults: 2 },
  typography: { maxResults: 2 },
} as const;

export const SEMANTIC_COLOR_ENTRIES: readonly [label: string, key: string, cssVar: string][] = [
  ["Primary", "primary", "--color-primary"],
  ["On Primary", "on_primary", "--color-on-primary"],
  ["Secondary", "secondary", "--color-secondary"],
  ["On Secondary", "on_secondary", "--color-on-secondary"],
  ["Accent/CTA", "accent", "--color-accent"],
  ["On Accent/CTA", "on_accent", "--color-on-accent"],
  ["Background", "background", "--color-background"],
  ["Foreground", "foreground", "--color-foreground"],
  ["Card", "card", "--color-card"],
  ["Card Foreground", "card_foreground", "--color-card-foreground"],
  ["Muted", "muted", "--color-muted"],
  ["Muted Foreground", "muted_foreground", "--color-muted-foreground"],
  ["Border", "border", "--color-border"],
  ["Destructive", "destructive", "--color-destructive"],
  ["On Destructive", "on_destructive", "--color-on-destructive"],
  ["Ring", "ring", "--color-ring"],
];

// ============ DESIGN DIALS (1-10) ============
type Spacing = Record<"xs" | "sm" | "md" | "lg" | "xl" | "2xl" | "3xl", string>;
interface DialInfo {
  label: string;
  style_keywords?: string[];
  tier?: string;
  spacing?: Spacing;
}
type DialName = "variance" | "motion" | "density";

/**
 * Three optional 1-10 sliders that bias the query-based search instead of
 * replacing it. Each dial buckets into a low/mid/high tier.
 */
export const DIAL_TIERS: Record<DialName, [number, number, DialInfo][]> = {
  variance: [
    [1, 3, { label: "Centered / Minimal", style_keywords: ["Minimalism", "Exaggerated Minimalism", "centered", "symmetric", "grid-based"] }],
    [4, 7, { label: "Balanced / Modern", style_keywords: ["modern", "structured", "balanced"] }],
    [8, 10, { label: "Bold / Asymmetric", style_keywords: ["Brutalism", "Bento Grids", "asymmetric", "experimental"] }],
  ],
  motion: [
    [1, 3, { label: "Subtle", tier: "Subtle" }],
    [4, 7, { label: "Standard", tier: "Standard" }],
    [8, 10, { label: "Complex", tier: "Complex" }],
  ],
  density: [
    [1, 3, { label: "Spacious", spacing: { xs: "4px", sm: "8px", md: "24px", lg: "32px", xl: "48px", "2xl": "64px", "3xl": "96px" } }],
    [4, 7, { label: "Standard", spacing: { xs: "4px", sm: "8px", md: "16px", lg: "24px", xl: "32px", "2xl": "48px", "3xl": "64px" } }],
    [8, 10, { label: "Dense / Dashboard", spacing: { xs: "2px", sm: "4px", md: "8px", lg: "12px", xl: "16px", "2xl": "24px", "3xl": "32px" } }],
  ],
};

/** Bucket a 1-10 dial value into its tier config, or null when unset. */
export function resolveDial(dial: DialName, value: number | null | undefined): (DialInfo & { value: number }) | null {
  if (value === null || value === undefined) return null;
  const clamped = Math.max(1, Math.min(10, Math.trunc(value)));
  for (const [lo, hi, info] of DIAL_TIERS[dial]) {
    if (lo <= clamped && clamped <= hi) return { ...info, value: clamped };
  }
  return null;
}

// ============ COLOR MODE RESOLUTION ============
/** Phrases in styles.csv mode columns that mark a style as dark-first. */
const DARK_PRIMARY_MARKERS = [
  "dark mode primary", "dark primary", "dark-only", "dark only",
  "dark preferred", "dark focused", "dark-first", "dark rich",
  "light mode only as exception",
];
/** Query phrases that are an explicit request for a dark theme. */
const DARK_QUERY_MARKERS = ["dark mode", "dark theme", "dark ui", "dark-mode", "darkmode", "night mode", "midnight", "oled"];
/** Anti-pattern clauses that contradict a resolved dark mode. */
const DARK_ANTI_PATTERN_MARKERS = ["dark mode", "dark modes", "dark theme"];
/** Relative luminance below which a Background counts as a dark surface. */
const DARK_BACKGROUND_MAX_LUMINANCE = 0.18;

/** WCAG relative luminance of a #RGB/#RRGGBB string, or null if unparseable. */
export function relativeLuminance(hexColor: string | null | undefined): number | null {
  if (!hexColor) return null;
  let value = hexColor.trim().replace(/^#+/, "");
  if (value.length === 3) value = [...value].map((c) => c + c).join("");
  if (value.length !== 6 || !/^[0-9a-fA-F]{6}$/.test(value)) return null;
  const channels = [0, 2, 4].map((i) => parseInt(value.slice(i, i + 2), 16) / 255);
  const linear = channels.map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!;
}

/** True when a colors.csv row's Background is a dark surface. */
export function paletteIsDark(palette: Row | null | undefined): boolean {
  const luminance = relativeLuminance(palette?.["Background"] ?? "");
  return luminance !== null && luminance < DARK_BACKGROUND_MAX_LUMINANCE;
}

/** WCAG contrast ratio for two hex colors, or null if either is invalid. */
export function contrastRatio(first: string | null | undefined, second: string | null | undefined): number | null {
  const a = relativeLuminance(first);
  const b = relativeLuminance(second);
  if (a === null || b === null) return null;
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/** True when a styles.csv row describes itself as dark-first. */
export function styleIsDarkPrimary(style: Row | null | undefined): boolean {
  if (!style || Object.keys(style).length === 0) return false;
  const preferred = (style["Preferred Mode"] ?? "").trim().toLowerCase();
  if (preferred === "dark" || preferred === "light") return preferred === "dark";
  if (style["Light Mode ✓"] === "not-recommended" && style["Dark Mode ✓"] === "supported") return true;
  const declared = `${style["Light Mode ✓"] ?? ""} ${style["Dark Mode ✓"] ?? ""}`.toLowerCase();
  return DARK_PRIMARY_MARKERS.some((marker) => declared.includes(marker));
}

export function queryWantsDark(query: string): boolean {
  const lowered = (query || "").toLowerCase();
  return DARK_QUERY_MARKERS.some((marker) => lowered.includes(marker));
}

/** Resolve the mode the rest of the output has to agree with. */
export function resolveColorMode(query: string, style: Row): "dark" | "light" {
  return queryWantsDark(query) || styleIsDarkPrimary(style) ? "dark" : "light";
}

/** Keep product brand tokens while deriving accessible dark surfaces. */
function deriveDarkPalette(palette: Row): Row {
  const background = "#0F172A";
  const candidates = [palette["Ring"], palette["Accent"], palette["Primary"], "#60A5FA"];
  const ring = candidates.find((candidate) => (contrastRatio(candidate, background) ?? 0) >= 3) ?? "#60A5FA";
  return {
    ...palette,
    Background: background,
    Foreground: "#F8FAFC",
    Card: "#111827",
    "Card Foreground": "#F8FAFC",
    Muted: "#1E293B",
    "Muted Foreground": "#CBD5E1",
    Border: "#334155",
    Ring: ring,
    _mode_derivation: "derived-dark",
  };
}

/**
 * Pick the highest-ranked palette matching the resolved mode. Only the dark
 * case filters; falls back to the top hit when the data has no matching ramp.
 */
export function selectPaletteForMode(palettes: Row[], mode: string, category: string | null = null): Row {
  if (palettes.length === 0) return {};
  const categoryPalette = palettes.find((palette) => palette["Product Type"] === category);
  if (categoryPalette) {
    if (mode === "dark" && !paletteIsDark(categoryPalette)) return deriveDarkPalette(categoryPalette);
    return categoryPalette;
  }
  if (mode === "dark") {
    const dark = palettes.find((palette) => paletteIsDark(palette));
    if (dark) return dark;
  }
  return palettes[0]!;
}

/** Drop "avoid dark mode" advice once dark mode is the resolved answer. */
export function filterAntiPatternsForMode(antiPatterns: string, mode: string): string {
  if (mode !== "dark" || !antiPatterns) return antiPatterns;
  return antiPatterns
    .split("+")
    .filter((clause) => !DARK_ANTI_PATTERN_MARKERS.some((marker) => clause.toLowerCase().includes(marker)))
    .map((clause) => clause.trim())
    .filter(Boolean)
    .join(" + ");
}

/** Light/dark support labels that agree with the product's anti-patterns. */
export function modeSupportLabels(style: DesignSystem["style"], antiPatterns: string): [string, string] {
  const light = style.light_mode ?? "";
  let dark = style.dark_mode ?? "";
  const avoidsDark = DARK_ANTI_PATTERN_MARKERS.some((marker) => (antiPatterns || "").toLowerCase().includes(marker));
  if (dark && dark !== "not-recommended" && avoidsDark) dark = `${dark}, but avoid for this product`;
  return [light, dark];
}

/** Primary for outline-button text when it is readable, else the foreground. */
function buttonOutlineTextColor(colors: Record<string, string>): string {
  const primary = colors["primary"] || "#2563EB";
  const background = colors["background"] || "#FFFFFF";
  if ((contrastRatio(primary, background) ?? 0) >= 4.5) return primary;
  return colors["foreground"] || primary;
}

// ============ TYPES ============
export interface Reasoning {
  pattern: string;
  style_priority: string[];
  color_mood: string;
  typography_mood: string;
  key_effects: string;
  anti_patterns: string;
  decision_rules: Record<string, string[]>;
  activated_rules: { condition: string; actions: string[] }[];
  constraints: string[];
  preferred_mode: string | null;
  is_default: boolean;
  severity: string;
}

export interface DesignSystem {
  project_name: string;
  category: string;
  pattern: { name: string; sections: string; cta_placement: string; color_strategy: string; conversion: string };
  style: {
    id: string; name: string; type: string; effects: string; keywords: string; best_for: string;
    performance: string; accessibility: string; light_mode: string; dark_mode: string;
  };
  colors: Record<string, string>;
  typography: { heading: string; body: string; mood: string; best_for: string; google_fonts_url: string; css_import: string; mono?: string };
  brand: null | {
    id: string; name: string; theme: string; package: string; source_file: string; selector: string;
    color_scheme: string; radius: string; notes: string;
  };
  color_mode: string;
  key_effects: string;
  anti_patterns: string;
  decision_rules: Record<string, string[]>;
  activated_rules: { condition: string; actions: string[] }[];
  constraints: string[];
  reasoning_default: boolean;
  source_identities: Record<string, string | null>;
  source_derivations: Record<string, string | null>;
  severity: string;
  dials: {
    variance: number | null; variance_label: string | null;
    motion: number | null; motion_label: string | null;
    density: number | null; density_label: string | null;
  };
  motion_snippet: Row;
  spacing_scale: Spacing | null;
}

export interface GenerateOptions {
  projectName?: string | null;
  variance?: number | null;
  motion?: number | null;
  density?: number | null;
  /** Brand ID from brands.csv (e.g. rose-griffon-roy): its real tokens replace the palette and fonts. */
  brand?: string | null;
}

// ============ DESIGN SYSTEM GENERATOR ============
export class DesignSystemGenerator {
  private constructor(
    readonly reasoningData: Row[],
    readonly styleData: Row[],
    readonly styleLookup: Map<string, Row>,
    readonly landingLookup: Map<string, Row>,
  ) {}

  static async create(): Promise<DesignSystemGenerator> {
    const read = async (file: string) => {
      const path = join(DATA_DIR, file);
      return (await Bun.file(path).exists()) ? parseCsvRecords(await Bun.file(path).text()) : [];
    };
    const reasoning = await read(REASONING_FILE);
    const styles = await read("styles.csv");
    const landing = await read("landing.csv");
    const landingLookup = new Map<string, Row>();
    for (const row of landing) {
      for (const identity of [row["Pattern ID"] ?? "", row["Pattern Name"] ?? "", ...(row["Aliases"] ?? "").split("|")]) {
        if (identity.trim()) landingLookup.set(identity.trim().toLowerCase(), row);
      }
    }
    return new DesignSystemGenerator(reasoning, styles, DesignSystemGenerator.buildStyleLookup(styles), landingLookup);
  }

  static buildStyleLookup(styles: Row[]): Map<string, Row> {
    const lookup = new Map<string, Row>();
    for (const style of styles) {
      for (const key of [style["Style ID"] ?? "", style["Style Category"] ?? "", ...(style["Aliases"] ?? "").split("|")]) {
        if (key.trim()) lookup.set(key.trim().toLowerCase(), style);
      }
    }
    return lookup;
  }

  resolveStyle(reference: string | null | undefined): Row {
    let style = this.styleLookup.get(String(reference ?? "").trim().toLowerCase()) ?? {};
    const seen = new Set<string>();
    while (Object.keys(style).length && (style["Status"] ?? "active") === "deprecated") {
      const styleId = style["Style ID"] ?? "";
      const parentId = style["Parent Style ID"] ?? "";
      if (!parentId || seen.has(styleId)) return {};
      seen.add(styleId);
      style = this.styleLookup.get(parentId.toLowerCase()) ?? {};
    }
    return style;
  }

  /** Execute searches across multiple domains. */
  async multiDomainSearch(query: string, category: string, reasoning: Reasoning, stylePriority: string[] | null): Promise<Record<string, SearchResult>> {
    const results: Record<string, SearchResult> = {};
    const constraints = reasoning.constraints.map((item) => item.replaceAll("-", " ")).join(" ");
    const resolvedQuery = [query, category, constraints].filter(Boolean).join(" ");
    for (const [domain, config] of Object.entries(SEARCH_CONFIG)) {
      if (domain === "style" && stylePriority?.length) {
        results[domain] = await search(`${resolvedQuery} ${stylePriority.slice(0, 2).join(" ")}`, domain, config.maxResults);
      } else if (domain === "color") {
        results[domain] = await search(`${reasoning.color_mood} ${resolvedQuery}`, domain, config.maxResults);
      } else if (domain === "landing") {
        // The reasoning pattern is the landing query contract: mixing the
        // product prompt into it can push token coverage below the abstention
        // threshold even when the pattern names an exact row.
        const pattern = reasoning.pattern;
        const landingQuery = this.landingLookup.has(pattern.toLowerCase()) ? pattern : `${pattern} ${resolvedQuery}`;
        results[domain] = await search(landingQuery || query, domain, config.maxResults);
      } else if (domain === "typography") {
        results[domain] = await search(`${reasoning.typography_mood} ${resolvedQuery}`, domain, config.maxResults);
      } else {
        results[domain] = await search(query, domain, config.maxResults);
      }
    }
    return results;
  }

  findReasoningRule(category: string): Row {
    const lowered = category.trim().toLowerCase();
    return this.reasoningData.find((rule) => (rule["UI_Category"] ?? "").trim().toLowerCase() === lowered) ?? {};
  }

  /** Apply reasoning rules for a category. */
  applyReasoning(category: string, query: string): Reasoning {
    const rule = this.findReasoningRule(category);
    if (Object.keys(rule).length === 0) {
      return {
        pattern: "Hero + Features + CTA",
        style_priority: ["Minimalism", "Flat Design"],
        color_mood: "Professional",
        typography_mood: "Clean",
        key_effects: "Subtle hover transitions",
        anti_patterns: "",
        decision_rules: {},
        activated_rules: [],
        constraints: [],
        preferred_mode: null,
        is_default: true,
        severity: "MEDIUM",
      };
    }
    const decisionRules = parseDecisionRules(rule["Decision_Rules"] ?? "{}");
    const applied = applyDecisionRules(decisionRules, query);
    const stylePriority = (rule["Style_Priority"] ?? "").split("+").map((style) => style.trim());
    const appliedStyleNames = applied.style_ids.map((id) => this.resolveStyle(id)["Style Category"] ?? id);
    return {
      pattern: applied.pattern || rule["Recommended_Pattern"] || "",
      style_priority: [...appliedStyleNames, ...stylePriority],
      color_mood: rule["Color_Mood"] ?? "",
      typography_mood: rule["Typography_Mood"] ?? "",
      key_effects: rule["Key_Effects"] ?? "",
      anti_patterns: rule["Anti_Patterns"] ?? "",
      decision_rules: decisionRulesToObject(decisionRules),
      activated_rules: applied.activated,
      constraints: applied.constraints,
      preferred_mode: applied.mode,
      is_default: false,
      severity: rule["Severity"] ?? "MEDIUM",
    };
  }

  /** Select the best matching result based on priority keywords. */
  selectBestMatch(results: Row[], priorityKeywords: string[]): Row {
    if (results.length === 0) return {};
    if (priorityKeywords.length === 0) return results[0]!;
    // Canonical reasoning has authority over lexical candidates.
    for (const priority of priorityKeywords) {
      const resolved = this.resolveStyle(priority);
      if (Object.keys(resolved).length && (resolved["Status"] ?? "active") !== "deprecated") return { ...resolved };
    }
    const tokens = (text: string) => new Set(text.toLowerCase().match(/[a-z0-9]+/g) ?? []);
    const scored = results.map((result) => {
      const resultStr = pythonDictRepr(result).toLowerCase();
      let score = 0;
      for (const keyword of priorityKeywords) {
        const keywordTokens = tokens(keyword);
        const nameTokens = tokens(result["Style Category"] ?? "");
        const keywordList = [...keywordTokens];
        if (keywordTokens.size && keywordList.every((token) => nameTokens.has(token))) score += 10;
        else if (keywordList.some((token) => tokens(result["Keywords"] ?? "").has(token))) score += 3;
        else if (keywordList.some((token) => resultStr.includes(token))) score += 1;
      }
      return [score, result] as const;
    });
    scored.sort((a, b) => b[0] - a[0]);
    return scored[0] && scored[0][0] > 0 ? scored[0][1] : results[0]!;
  }

  /** Generate the complete design-system recommendation. */
  async generate(query: string, options: GenerateOptions = {}): Promise<DesignSystem> {
    const varianceInfo = resolveDial("variance", options.variance);
    const motionInfo = resolveDial("motion", options.motion);
    const densityInfo = resolveDial("density", options.density);

    let brand: Row | null = null;
    if (options.brand) {
      brand = await findBrand(options.brand);
      if (!brand) {
        const known = (await loadBrands()).map((row) => row["Brand ID"]).filter(Boolean);
        throw new Error(`Unknown brand: ${options.brand}. Available: ${known.join(", ") || "(brands.csv missing)"}`);
      }
    }

    // Step 1: product search gives the category.
    const productResult = await search(query, "product", 1);
    const productResults = productResult.results ?? [];
    const category = productResults[0]?.["Product Type"] ?? "General";

    // Step 2: reasoning rules for this category.
    const reasoning = this.applyReasoning(category, query);
    const stylePriority = reasoning.style_priority;
    const effectiveStylePriority = varianceInfo ? [...varianceInfo.style_keywords!, ...stylePriority] : stylePriority;

    // Step 3: multi-domain search with style priority hints.
    const searchResults = await this.multiDomainSearch(query, category, reasoning, effectiveStylePriority);
    searchResults["product"] = productResult;

    // Step 4: best match per domain.
    const styleResults = searchResults["style"]?.results ?? [];
    const colorResults = searchResults["color"]?.results ?? [];
    const typographyResults = searchResults["typography"]?.results ?? [];
    const landingResults = searchResults["landing"]?.results ?? [];

    const bestStyle = this.selectBestMatch(styleResults, effectiveStylePriority);
    const brandScheme = (brand?.["Color Scheme"] ?? "").trim().toLowerCase();
    const colorMode =
      (brandScheme === "dark" || brandScheme === "light" ? brandScheme : null) ??
      reasoning.preferred_mode ??
      resolveColorMode(query, bestStyle);
    const bestColor = brand ? brandPalette(brand) : selectPaletteForMode(colorResults, colorMode, category);
    const bestTypography: Row = brand ? brandTypography(brand) : (typographyResults[0] ?? {});
    const bestLanding =
      landingResults.find((row) => row["Pattern Name"] === reasoning.pattern) ?? landingResults[0] ?? {};

    // MOTION_INTENSITY dial: pull a matching GSAP skeleton from motion.csv.
    let motionSnippet: Row = {};
    if (motionInfo) {
      const motionMatches = (await search(`${query} ${motionInfo.tier}`, "gsap", 5)).results ?? [];
      motionSnippet = motionMatches.find((match) => match["Intensity Tier"] === motionInfo.tier) ?? motionMatches[0] ?? {};
    }

    // Step 5: build the final recommendation.
    const styleEffects = bestStyle["Effects & Animation"] ?? "";
    const combinedEffects = styleEffects || reasoning.key_effects;

    return {
      project_name: options.projectName || brand?.["Brand"] || query.toUpperCase(),
      category,
      pattern: {
        name: bestLanding["Pattern Name"] ?? reasoning.pattern,
        sections: bestLanding["Section Order"] ?? "Hero > Features > CTA",
        cta_placement: bestLanding["Primary CTA Placement"] ?? "Above fold",
        color_strategy: bestLanding["Color Strategy"] ?? "",
        conversion: bestLanding["Conversion Optimization"] ?? "",
      },
      style: {
        id: bestStyle["Style ID"] ?? "minimalism-and-swiss-style",
        name: bestStyle["Style Category"] ?? "Minimalism",
        type: bestStyle["Type"] ?? "General",
        effects: styleEffects,
        keywords: bestStyle["Keywords"] ?? "",
        best_for: bestStyle["Best For"] ?? "",
        performance: bestStyle["Performance"] ?? "",
        accessibility: bestStyle["Accessibility"] ?? "",
        light_mode: bestStyle["Light Mode ✓"] ?? "",
        dark_mode: bestStyle["Dark Mode ✓"] ?? "",
      },
      colors: {
        primary: bestColor["Primary"] ?? "#2563EB",
        on_primary: bestColor["On Primary"] ?? "",
        secondary: bestColor["Secondary"] ?? "#3B82F6",
        on_secondary: bestColor["On Secondary"] ?? "",
        accent: bestColor["Accent"] ?? "#F97316",
        on_accent: bestColor["On Accent"] ?? "",
        background: bestColor["Background"] ?? "#F8FAFC",
        foreground: bestColor["Foreground"] ?? "#1E293B",
        card: bestColor["Card"] ?? "",
        card_foreground: bestColor["Card Foreground"] ?? "",
        muted: bestColor["Muted"] ?? "",
        muted_foreground: bestColor["Muted Foreground"] ?? "",
        border: bestColor["Border"] ?? "",
        destructive: bestColor["Destructive"] ?? "",
        on_destructive: bestColor["On Destructive"] ?? "",
        ring: bestColor["Ring"] ?? "",
        notes: bestColor["Notes"] ?? "",
        // Legacy keys kept for MASTER.md backward compatibility.
        cta: bestColor["Accent"] ?? "#F97316",
        text: bestColor["Foreground"] ?? "#1E293B",
        on_cta: bestColor["On Accent"] ?? "",
      },
      typography: {
        heading: bestTypography["Heading Font"] ?? "Inter",
        body: bestTypography["Body Font"] ?? "Inter",
        mood: bestTypography["Mood/Style Keywords"] ?? reasoning.typography_mood,
        best_for: bestTypography["Best For"] ?? "",
        google_fonts_url: bestTypography["Google Fonts URL"] ?? "",
        css_import: bestTypography["CSS Import"] ?? "",
        ...(brand?.["Mono Font"] ? { mono: brand["Mono Font"] } : {}),
      },
      brand: brand
        ? {
            id: brand["Brand ID"] ?? "",
            name: brand["Brand"] ?? "",
            theme: brand["Theme"] ?? "",
            package: brand["Package"] ?? "",
            source_file: brand["Source File"] ?? "",
            selector: brand["Selector"] ?? "",
            color_scheme: brand["Color Scheme"] ?? "",
            radius: brand["Radius"] ?? "",
            notes: brand["Notes"] ?? "",
          }
        : null,
      color_mode: colorMode,
      key_effects: combinedEffects,
      anti_patterns: filterAntiPatternsForMode(reasoning.anti_patterns, colorMode),
      decision_rules: reasoning.decision_rules,
      activated_rules: reasoning.activated_rules,
      constraints: reasoning.constraints,
      reasoning_default: reasoning.is_default,
      source_identities: {
        product: productResults.length ? category : null,
        reasoning: reasoning.is_default ? null : category,
        style: bestStyle["Style ID"] || bestStyle["Style Category"] || null,
        color: brand ? null : (bestColor["Product Type"] ?? null),
        typography: brand ? null : (bestTypography["Font Pairing Name"] ?? null),
        landing: bestLanding["Pattern Name"] ?? null,
        ...(brand ? { brand: brand["Brand ID"] ?? null } : {}),
      },
      source_derivations: {
        color_mode: bestColor["_mode_derivation"] ?? null,
        ...(brand ? { brand_source: brand["Source File"] ?? null } : {}),
      },
      severity: reasoning.severity,
      dials: {
        variance: varianceInfo?.value ?? null,
        variance_label: varianceInfo?.label ?? null,
        motion: motionInfo?.value ?? null,
        motion_label: motionInfo?.label ?? null,
        density: densityInfo?.value ?? null,
        density_label: densityInfo?.label ?? null,
      },
      motion_snippet: motionSnippet,
      spacing_scale: densityInfo?.spacing ?? null,
    };
  }
}

/** colors.csv-shaped palette from a brands.csv row (real CSS tokens). */
function brandPalette(brand: Row): Row {
  const palette: Row = { "Product Type": brand["Brand ID"] ?? "", Notes: brand["Notes"] ?? "" };
  for (const column of ["Primary", "On Primary", "Secondary", "On Secondary", "Accent", "On Accent", "Background", "Foreground", "Card", "Card Foreground", "Muted", "Muted Foreground", "Border", "Destructive", "On Destructive", "Ring"]) {
    if (brand[column]) palette[column] = brand[column]!;
  }
  return palette;
}

function brandTypography(brand: Row): Row {
  const typography: Row = { "Font Pairing Name": `${brand["Brand"] ?? ""} ${brand["Theme"] ?? ""}`.trim(), "Mood/Style Keywords": brand["Keywords"] ?? "" };
  if (brand["Heading Font"]) typography["Heading Font"] = brand["Heading Font"];
  if (brand["Body Font"]) typography["Body Font"] = brand["Body Font"];
  return typography;
}

/** Approximation of Python's str(dict) used for loose keyword scoring. */
function pythonDictRepr(row: Row): string {
  const quote = (value: string) => (value.includes("'") && !value.includes('"') ? `"${value}"` : `'${value.replaceAll("\\", "\\\\").replaceAll("'", "\\'")}'`);
  return `{${Object.entries(row).map(([key, value]) => `${quote(key)}: ${quote(value)}`).join(", ")}}`;
}

// ============ OUTPUT FORMATTERS ============
export const BOX_WIDTH = 90;

function pyLen(text: string): number {
  let length = 0;
  for (const _ of text) length++;
  return length;
}

function ljust(text: string, width: number): string {
  return text + " ".repeat(Math.max(0, width - pyLen(text)));
}

/** Hex color to an ANSI true-color swatch, only when the terminal supports it. */
function hexToAnsi(hexColor: string): string {
  if (!hexColor || !hexColor.startsWith("#")) return "";
  const colorterm = Bun.env.COLORTERM ?? "";
  if (colorterm !== "truecolor" && colorterm !== "24bit") return "";
  const hex = hexColor.replace(/^#+/, "");
  if (hex.length !== 6) return "";
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16));
  return `\x1b[38;2;${r};${g};${b}m██\x1b[0m `;
}

/** ljust that ignores zero-width ANSI escape sequences. */
function ansiLjust(text: string, width: number): string {
  const visible = pyLen(text.replace(/\x1b\[[0-9;]*m/g, ""));
  return text + " ".repeat(Math.max(0, width - visible));
}

function sectionHeader(name: string, width: number): string {
  const label = `─── ${name} `;
  return `├${label}${"─".repeat(width - pyLen(label) - 1)}┤`;
}

const CHECKLIST_ITEMS = [
  "No emojis as icons (use SVG: Heroicons/Lucide/Material Symbols)",
  "cursor-pointer on all clickable elements",
  "Hover states with smooth transitions (150-300ms)",
  "Light mode: text contrast 4.5:1 minimum",
  "Focus states visible for keyboard nav",
  "Focus visible in forced-colors mode (outline-color: CanvasText)",
  "prefers-reduced-motion respected",
  "color-scheme declared for light/dark",
  "LCP image: fetchPriority=\"high\" + loading=\"eager\"",
  "Responsive: 375px, 768px, 1024px, 1440px",
];

function hasDials(dials: DesignSystem["dials"]): boolean {
  return dials.variance !== null || dials.motion !== null || dials.density !== null;
}

/** Format the design system as a Unicode box with ANSI color swatches. */
export function formatAsciiBox(designSystem: DesignSystem): string {
  const { project_name: project, pattern, style, colors, typography, dials, brand } = designSystem;
  const effects = designSystem.key_effects;
  const antiPatterns = designSystem.anti_patterns;
  const motionSnippet = designSystem.motion_snippet;
  const lines: string[] = [];
  const w = BOX_WIDTH - 1;

  /** Wrap long text, splitting tokens (URLs) wider than a line. */
  const wrapText = (text: string, prefix: string, width: number): string[] => {
    if (!text) return [];
    const room = width - 2 - pyLen(prefix);
    const words: string[] = [];
    for (const word of text.split(/\s+/).filter(Boolean)) {
      const chars = Array.from(word);
      for (let i = 0; i < chars.length; i += room) words.push(chars.slice(i, i + room).join(""));
    }
    const out: string[] = [];
    let current = prefix;
    for (const word of words) {
      if (pyLen(current) + pyLen(word) + 1 <= width - 2) {
        current += (current !== prefix ? " " : "") + word;
      } else {
        if (current !== prefix) out.push(current);
        current = prefix + word;
      }
    }
    if (current !== prefix) out.push(current);
    return out;
  };
  const addWrapped = (text: string, prefix = "│     ") => {
    for (const line of wrapText(text, prefix, BOX_WIDTH)) lines.push(ljust(line, BOX_WIDTH) + "│");
  };
  const row = (text: string) => lines.push(ljust(text, BOX_WIDTH) + "│");

  const sections = pattern.sections.split(" > ").map((section) => section.trim()).filter(Boolean);

  lines.push("╔" + "═".repeat(w) + "╗");
  lines.push(ansiLjust(`║  TARGET: ${project} - RECOMMENDED DESIGN SYSTEM`, BOX_WIDTH) + "║");
  lines.push("╚" + "═".repeat(w) + "╝");
  lines.push("┌" + "─".repeat(w) + "┐");

  if (hasDials(dials)) {
    lines.push(sectionHeader("DESIGN DIALS", BOX_WIDTH));
    if (dials.variance !== null) row(`│  Variance: ${dials.variance}/10 — ${dials.variance_label}`);
    if (dials.motion !== null) row(`│  Motion:   ${dials.motion}/10 — ${dials.motion_label}`);
    if (dials.density !== null) row(`│  Density:  ${dials.density}/10 — ${dials.density_label}`);
  }

  if (brand) {
    lines.push(sectionHeader("BRAND", BOX_WIDTH));
    addWrapped(`${brand.name} — ${brand.theme} (${brand.id})`, "│  ");
    if (brand.package) addWrapped(`Package: ${brand.package}`);
    if (brand.selector) addWrapped(`Selector: ${brand.selector}`);
    if (brand.color_scheme) addWrapped(`Color scheme: ${brand.color_scheme}`);
    if (brand.radius) addWrapped(`Radius: ${brand.radius}`);
    if (brand.source_file) addWrapped(`Source: ${brand.source_file}`);
  }

  lines.push(sectionHeader("PATTERN", BOX_WIDTH));
  addWrapped(`Name: ${pattern.name}`, "│  ");
  if (pattern.conversion) addWrapped(`Conversion: ${pattern.conversion}`);
  if (pattern.cta_placement) addWrapped(`CTA: ${pattern.cta_placement}`);
  row("│     Sections:");
  sections.forEach((section, index) => addWrapped(`${index + 1}. ${section}`, "│       "));

  lines.push(sectionHeader("STYLE", BOX_WIDTH));
  addWrapped(`Name: ${style.name}`, "│  ");
  const [light, dark] = modeSupportLabels(style, antiPatterns);
  if (light || dark) addWrapped(`Mode Support: Light ${light} | Dark ${dark}`);
  if (style.keywords) addWrapped(`Keywords: ${style.keywords}`);
  if (style.best_for) addWrapped(`Best For: ${style.best_for}`);
  if (style.performance || style.accessibility) addWrapped(`Performance: ${style.performance} | Accessibility: ${style.accessibility}`);

  lines.push(sectionHeader("COLORS", BOX_WIDTH));
  for (const [label, key, cssVar] of SEMANTIC_COLOR_ENTRIES) {
    const hex = colors[key] ?? "";
    if (!hex) continue;
    lines.push(ansiLjust(`│     ${hexToAnsi(hex)}${ljust(label + ":", 18)} ${ljust(hex, 10)} (${cssVar})`, BOX_WIDTH) + "│");
  }
  if (colors["notes"]) {
    for (const line of wrapText(`Notes: ${colors["notes"]}`, "│     ", BOX_WIDTH)) row(line);
  }

  lines.push(sectionHeader("TYPOGRAPHY", BOX_WIDTH));
  addWrapped(`${typography.heading} / ${typography.body}`, "│  ");
  if (typography.mono) addWrapped(`Mono: ${typography.mono}`);
  if (typography.mood) addWrapped(`Mood: ${typography.mood}`);
  if (typography.best_for) addWrapped(`Best For: ${typography.best_for}`);
  if (typography.google_fonts_url) addWrapped(`Google Fonts: ${typography.google_fonts_url}`);
  if (typography.css_import) row(`│     CSS Import: ${Array.from(typography.css_import).slice(0, 67).join("")}...`);

  if (effects) {
    lines.push(sectionHeader("KEY EFFECTS", BOX_WIDTH));
    for (const line of wrapText(effects, "│     ", BOX_WIDTH)) row(line);
  }

  if (Object.keys(motionSnippet).length) {
    lines.push(sectionHeader("MOTION", BOX_WIDTH));
    addWrapped(`${motionSnippet["Category"] ?? ""} (${motionSnippet["Intensity Tier"] ?? ""})`, "│  ");
    addWrapped(`Trigger: ${motionSnippet["Trigger"] ?? ""} | Duration: ${motionSnippet["Duration"] ?? ""} | Easing: ${motionSnippet["Easing"] ?? ""}`);
    for (const line of wrapText(`GSAP: ${motionSnippet["GSAP Snippet"] ?? ""}`, "│     ", BOX_WIDTH)) row(line);
    if (motionSnippet["Framework Notes"]) {
      for (const line of wrapText(`Framework: ${motionSnippet["Framework Notes"]}`, "│     ", BOX_WIDTH)) row(line);
    }
  }

  if (antiPatterns) {
    lines.push(sectionHeader("AVOID", BOX_WIDTH));
    for (const line of wrapText(antiPatterns, "│     ", BOX_WIDTH)) row(line);
  }

  lines.push(sectionHeader("PRE-DELIVERY CHECKLIST", BOX_WIDTH));
  for (const item of CHECKLIST_ITEMS) row(`│     [ ] ${item}`);
  lines.push("└" + "─".repeat(w) + "┘");
  return lines.join("\n");
}

/** Format the design system as markdown. */
export function formatMarkdown(designSystem: DesignSystem): string {
  const { project_name: project, pattern, style, colors, typography, dials, brand } = designSystem;
  const effects = designSystem.key_effects;
  const antiPatterns = designSystem.anti_patterns;
  const motionSnippet = designSystem.motion_snippet;
  const lines: string[] = [`## Design System: ${project}`, ""];

  if (hasDials(dials)) {
    lines.push("### Design Dials");
    if (dials.variance !== null) lines.push(`- **Variance:** ${dials.variance}/10 — ${dials.variance_label}`);
    if (dials.motion !== null) lines.push(`- **Motion:** ${dials.motion}/10 — ${dials.motion_label}`);
    if (dials.density !== null) lines.push(`- **Density:** ${dials.density}/10 — ${dials.density_label}`);
    lines.push("");
  }

  if (brand) lines.push(...brandMarkdown(brand), "");

  lines.push("### Pattern", `- **Name:** ${pattern.name}`);
  if (pattern.conversion) lines.push(`- **Conversion Focus:** ${pattern.conversion}`);
  if (pattern.cta_placement) lines.push(`- **CTA Placement:** ${pattern.cta_placement}`);
  if (pattern.color_strategy) lines.push(`- **Color Strategy:** ${pattern.color_strategy}`);
  lines.push(`- **Sections:** ${pattern.sections}`, "");

  lines.push("### Style", `- **Name:** ${style.name}`);
  const [light, dark] = modeSupportLabels(style, antiPatterns);
  if (light || dark) lines.push(`- **Mode Support:** Light ${light} | Dark ${dark}`);
  if (style.keywords) lines.push(`- **Keywords:** ${style.keywords}`);
  if (style.best_for) lines.push(`- **Best For:** ${style.best_for}`);
  if (style.performance || style.accessibility) lines.push(`- **Performance:** ${style.performance} | **Accessibility:** ${style.accessibility}`);
  lines.push("");

  lines.push("### Colors", "| Role | Hex | CSS Variable |", "|------|-----|--------------|");
  for (const [label, key, cssVar] of SEMANTIC_COLOR_ENTRIES) {
    if (colors[key]) lines.push(`| ${label} | \`${colors[key]}\` | \`${cssVar}\` |`);
  }
  if (colors["notes"]) lines.push(`\n*Notes: ${colors["notes"]}*`);
  lines.push("");

  lines.push("### Typography", `- **Heading:** ${typography.heading}`, `- **Body:** ${typography.body}`);
  if (typography.mono) lines.push(`- **Mono:** ${typography.mono}`);
  if (typography.mood) lines.push(`- **Mood:** ${typography.mood}`);
  if (typography.best_for) lines.push(`- **Best For:** ${typography.best_for}`);
  if (typography.google_fonts_url) lines.push(`- **Google Fonts:** ${typography.google_fonts_url}`);
  if (typography.css_import) lines.push("- **CSS Import:**", "```css", typography.css_import, "```");
  lines.push("");

  if (effects) lines.push("### Key Effects", effects, "");

  if (Object.keys(motionSnippet).length) {
    lines.push(
      "### Motion",
      `**${motionSnippet["Category"] ?? ""}** (${motionSnippet["Intensity Tier"] ?? ""}) — Trigger: ${motionSnippet["Trigger"] ?? ""} | Duration: ${motionSnippet["Duration"] ?? ""} | Easing: \`${motionSnippet["Easing"] ?? ""}\``,
      "```js",
      motionSnippet["GSAP Snippet"] ?? "",
      "```",
    );
    if (motionSnippet["Framework Notes"]) lines.push(`*Framework notes: ${motionSnippet["Framework Notes"]}*`);
    if (motionSnippet["Do"]) lines.push(`- ✅ ${motionSnippet["Do"]}`);
    if (motionSnippet["Don't"]) lines.push(`- ❌ ${motionSnippet["Don't"]}`);
    lines.push("");
  }

  if (antiPatterns) lines.push("### Avoid (Anti-patterns)", `- ${antiPatterns.replaceAll(" + ", "\n- ")}`, "");

  lines.push("### Pre-Delivery Checklist", ...CHECKLIST_ITEMS.map((item) => `- [ ] ${item}`), "");
  return lines.join("\n");
}

function brandMarkdown(brand: NonNullable<DesignSystem["brand"]>): string[] {
  const lines = ["### Brand", `- **Brand:** ${brand.name} — ${brand.theme} (\`${brand.id}\`)`];
  if (brand.package) lines.push(`- **Package:** \`${brand.package}\``);
  if (brand.selector) lines.push(`- **Selector:** \`${brand.selector}\``);
  if (brand.color_scheme) lines.push(`- **Color scheme:** ${brand.color_scheme}`);
  if (brand.radius) lines.push(`- **Radius:** \`${brand.radius}\``);
  if (brand.source_file) lines.push(`- **Source:** \`${brand.source_file}\` (tokens extracted verbatim; use the CSS custom properties, not raw hex)`);
  return lines;
}

// ============ MAIN ENTRY POINT ============
export interface DesignSystemRunOptions extends GenerateOptions {
  format?: "ascii" | "markdown";
  persist?: boolean;
  page?: string | null;
  outputDir?: string | null;
  force?: boolean;
}

export interface Persistence {
  status: "success" | "skipped_exists";
  design_system_dir: string;
  master_file: string;
  created_files: string[];
  message?: string;
}

/** Main entry point for design-system generation. */
export async function generateDesignSystem(
  query: string,
  options: DesignSystemRunOptions = {},
): Promise<{ text: string; design_system: DesignSystem; persistence: Persistence | null }> {
  const generator = await DesignSystemGenerator.create();
  const designSystem = await generator.generate(query, options);
  const persistence = options.persist
    ? await persistDesignSystem(designSystem, options.page ?? null, options.outputDir ?? null, query, options.force ?? false)
    : null;
  const text = options.format === "markdown" ? formatMarkdown(designSystem) : formatAsciiBox(designSystem);
  return { text, design_system: designSystem, persistence };
}

// ============ PERSISTENCE FUNCTIONS ============
/**
 * Slugify a name into a single safe path segment: only [a-z0-9_-] survives,
 * so project/page names can never traverse out of their parent directory.
 */
export function safeSlug(name: unknown, fallback = "default"): string {
  const slug = String(name).toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "");
  return slug || fallback;
}

class FileExistsError extends Error {}

/** Write fully to a temp file, then publish atomically. */
async function writePersistedFile(path: string, content: string, force: boolean): Promise<void> {
  const name = path.split(/[\\/]/).pop()!;
  const tempName = join(dirname(path), `.${name}.${process.pid}.${crypto.randomUUID()}.tmp`);
  let pending = true;
  try {
    await Bun.write(tempName, content);
    if (force) {
      await rename(tempName, path);
      pending = false;
    } else {
      // A same-filesystem hard link publishes only if the destination is absent.
      try {
        await link(tempName, path);
      } catch (error) {
        if ((error as { code?: string }).code === "EEXIST") throw new FileExistsError(path);
        throw error;
      }
    }
  } finally {
    if (pending) await unlink(tempName).catch(() => {});
  }
}

/** Persist to design-system/<project>/ using the Master + Overrides pattern. */
export async function persistDesignSystem(
  designSystem: DesignSystem,
  page: string | null = null,
  outputDir: string | null = null,
  pageQuery: string | null = null,
  force = false,
): Promise<Persistence> {
  const baseDir = outputDir || process.cwd();
  const projectSlug = safeSlug(designSystem.project_name || "default");
  const designSystemDir = join(baseDir, "design-system", projectSlug);
  const pagesDir = join(designSystemDir, "pages");
  const masterFile = join(designSystemDir, "MASTER.md");
  const createdFiles: string[] = [];
  await mkdir(pagesDir, { recursive: true });

  try {
    await writePersistedFile(masterFile, formatMasterMd(designSystem), force);
    createdFiles.push(masterFile);
  } catch (error) {
    if (!(error instanceof FileExistsError)) throw error;
    if (!page) {
      return {
        status: "skipped_exists",
        design_system_dir: designSystemDir,
        master_file: masterFile,
        created_files: [],
        message: `${masterFile} already exists and was not modified. Read it first to check for prior design decisions, then re-run with force=True / --force to overwrite.`,
      };
    }
  }

  if (page) {
    const pageFile = join(pagesDir, `${safeSlug(page, "page")}.md`);
    try {
      await writePersistedFile(pageFile, await formatPageOverrideMd(designSystem, page, pageQuery), force);
      createdFiles.push(pageFile);
    } catch (error) {
      if (!(error instanceof FileExistsError)) throw error;
      if (createdFiles.length === 0) {
        return {
          status: "skipped_exists",
          design_system_dir: designSystemDir,
          master_file: masterFile,
          created_files: [],
          message: `${pageFile} already exists and was not modified.`,
        };
      }
    }
  }
  return { status: "success", design_system_dir: designSystemDir, master_file: masterFile, created_files: createdFiles };
}

function timestamp(): string {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`;
}

/** Format the design system as MASTER.md with hierarchical override logic. */
export function formatMasterMd(designSystem: DesignSystem): string {
  const { project_name: project, pattern, style, colors, typography, dials, brand } = designSystem;
  const effects = designSystem.key_effects;
  const antiPatterns = designSystem.anti_patterns;
  const motionSnippet = designSystem.motion_snippet;
  const spacingScale = designSystem.spacing_scale;
  const lines: string[] = [
    "# Design System Master File",
    "",
    "> **LOGIC:** When building a specific page, first check `design-system/pages/[page-name].md`.",
    "> If that file exists, its rules **override** this Master file.",
    "> If not, strictly follow the rules below.",
    "",
    "---",
    "",
    `**Project:** ${project}`,
    `**Generated:** ${timestamp()}`,
    `**Category:** ${designSystem.category || "General"}`,
  ];
  if (hasDials(dials)) {
    const parts: string[] = [];
    if (dials.variance !== null) parts.push(`Variance ${dials.variance}/10 (${dials.variance_label})`);
    if (dials.motion !== null) parts.push(`Motion ${dials.motion}/10 (${dials.motion_label})`);
    if (dials.density !== null) parts.push(`Density ${dials.density}/10 (${dials.density_label})`);
    lines.push(`**Design Dials:** ${parts.join(" | ")}`);
  }
  lines.push("", "---", "", "## Global Rules", "");
  if (brand) lines.push(...brandMarkdown(brand), "");

  lines.push("### Color Palette", "", "| Role | Hex | CSS Variable |", "|------|-----|--------------|");
  for (const [label, key, cssVar] of SEMANTIC_COLOR_ENTRIES) {
    if (colors[key]) lines.push(`| ${label} | \`${colors[key]}\` | \`${cssVar}\` |`);
  }
  lines.push("");
  if (colors["notes"]) lines.push(`**Color Notes:** ${colors["notes"]}`, "");

  lines.push("### Typography", "", `- **Heading Font:** ${typography.heading || "Inter"}`, `- **Body Font:** ${typography.body || "Inter"}`);
  if (typography.mono) lines.push(`- **Mono Font:** ${typography.mono}`);
  if (typography.mood) lines.push(`- **Mood:** ${typography.mood}`);
  if (typography.google_fonts_url) lines.push(`- **Google Fonts:** [${typography.heading} + ${typography.body}](${typography.google_fonts_url})`);
  lines.push("");
  if (typography.css_import) lines.push("**CSS Import:**", "```css", typography.css_import, "```", "");

  // Spacing variables (overridden by the VISUAL_DENSITY dial when set).
  const scale = spacingScale ?? DIAL_TIERS.density[1]![2].spacing!;
  const spacingUsage: Spacing = {
    xs: "Tight gaps", sm: "Icon gaps, inline spacing", md: "Standard padding",
    lg: "Section padding", xl: "Large gaps", "2xl": "Section margins", "3xl": "Hero padding",
  };
  lines.push("### Spacing Variables", "");
  if (spacingScale) lines.push(`*Density: ${dials.density}/10 — ${dials.density_label}*`, "");
  lines.push("| Token | Value | Usage |", "|-------|-------|-------|");
  for (const token of ["xs", "sm", "md", "lg", "xl", "2xl", "3xl"] as const) {
    const px = scale[token];
    lines.push(`| \`--space-${token}\` | \`${px}\` / \`${parseInt(px, 10) / 16}rem\` | ${spacingUsage[token]} |`);
  }
  lines.push("");

  lines.push(
    "### Shadow Depths", "",
    "| Level | Value | Usage |",
    "|-------|-------|-------|",
    "| `--shadow-sm` | `0 1px 2px rgba(0,0,0,0.05)` | Subtle lift |",
    "| `--shadow-md` | `0 4px 6px rgba(0,0,0,0.1)` | Cards, buttons |",
    "| `--shadow-lg` | `0 10px 15px rgba(0,0,0,0.1)` | Modals, dropdowns |",
    "| `--shadow-xl` | `0 20px 25px rgba(0,0,0,0.15)` | Hero images, featured cards |",
    "",
  );

  const primary = colors["primary"] || "#2563EB";
  lines.push(
    "---", "", "## Component Specs", "",
    "### Buttons", "", "```css",
    "/* Primary Button */",
    ".btn-primary {",
    `  background: ${colors["cta"] || "#F97316"};`,
    `  color: ${colors["on_cta"] || "white"};`,
    "  padding: 12px 24px;",
    "  border-radius: 8px;",
    "  font-weight: 600;",
    "  transition: all 200ms ease;",
    "  cursor: pointer;",
    "}",
    "",
    ".btn-primary:hover {",
    "  opacity: 0.9;",
    "  transform: translateY(-1px);",
    "}",
    "",
    "/* Secondary Button */",
    ".btn-secondary {",
    "  background: transparent;",
    `  color: ${buttonOutlineTextColor(colors)};`,
    `  border: 2px solid ${primary};`,
    "  padding: 12px 24px;",
    "  border-radius: 8px;",
    "  font-weight: 600;",
    "  transition: all 200ms ease;",
    "  cursor: pointer;",
    "}",
    "",
    "/* Keyboard focus, including Windows High Contrast (forced colors) */",
    ".btn-primary:focus-visible,",
    ".btn-secondary:focus-visible {",
    `  outline: 2px solid ${colors["ring"] || primary};`,
    "  outline-offset: 2px;",
    "}",
    "",
    "@media (forced-colors: active) {",
    "  :focus-visible { outline-color: CanvasText; }",
    "}",
    "```",
    "",
    "### Cards", "", "```css",
    ".card {",
    `  background: ${colors["background"] || "#FFFFFF"};`,
    "  border-radius: 12px;",
    "  padding: 24px;",
    "  box-shadow: var(--shadow-md);",
    "  transition: all 200ms ease;",
    "  cursor: pointer;",
    "}",
    "",
    ".card:hover {",
    "  box-shadow: var(--shadow-lg);",
    "  transform: translateY(-2px);",
    "}",
    "```",
    "",
    "### Inputs", "", "```css",
    ".input {",
    "  padding: 12px 16px;",
    "  border: 1px solid #E2E8F0;",
    "  border-radius: 8px;",
    "  font-size: 16px;",
    "  transition: border-color 200ms ease;",
    "}",
    "",
    ".input:focus {",
    `  border-color: ${primary};`,
    "  outline: none;",
    `  box-shadow: 0 0 0 3px ${primary}20;`,
    "}",
    "```",
    "",
    "### Modals", "", "```css",
    ".modal-overlay {",
    "  background: rgba(0, 0, 0, 0.5);",
    "  backdrop-filter: blur(4px);",
    "}",
    "",
    ".modal {",
    "  background: white;",
    "  border-radius: 16px;",
    "  padding: 32px;",
    "  box-shadow: var(--shadow-xl);",
    "  max-width: 500px;",
    "  width: 90%;",
    "}",
    "```",
    "",
    "### Motion and Color Scheme", "", "```css",
    ":root {",
    `  color-scheme: ${designSystem.color_mode === "dark" ? "dark" : "light dark"};`,
    "}",
    "",
    "@media (prefers-reduced-motion: reduce) {",
    "  *, *::before, *::after {",
    "    animation-duration: 0.01ms !important;",
    "    transition-duration: 0.01ms !important;",
    "    scroll-behavior: auto !important;",
    "  }",
    "}",
    "```",
    "",
  );

  lines.push("---", "", "## Style Guidelines", "", `**Style:** ${style.name || "Minimalism"}`, "");
  if (style.keywords) lines.push(`**Keywords:** ${style.keywords}`, "");
  if (style.best_for) lines.push(`**Best For:** ${style.best_for}`, "");
  if (effects) lines.push(`**Key Effects:** ${effects}`, "");

  lines.push("### Page Pattern", "", `**Pattern Name:** ${pattern.name}`, "");
  if (pattern.conversion) lines.push(`- **Conversion Strategy:** ${pattern.conversion}`);
  if (pattern.cta_placement) lines.push(`- **CTA Placement:** ${pattern.cta_placement}`);
  lines.push(`- **Section Order:** ${pattern.sections}`, "");

  if (Object.keys(motionSnippet).length) {
    lines.push(
      "---", "", "## Motion", "",
      `**${motionSnippet["Category"] ?? ""}** (${motionSnippet["Intensity Tier"] ?? ""}) — Trigger: ${motionSnippet["Trigger"] ?? ""} | Duration: ${motionSnippet["Duration"] ?? ""} | Easing: \`${motionSnippet["Easing"] ?? ""}\``,
      "", "```js", motionSnippet["GSAP Snippet"] ?? "", "```", "",
    );
    if (motionSnippet["Framework Notes"]) lines.push(`**Framework notes:** ${motionSnippet["Framework Notes"]}`, "");
    if (motionSnippet["Do"]) lines.push(`- ✅ ${motionSnippet["Do"]}`);
    if (motionSnippet["Don't"]) lines.push(`- ❌ ${motionSnippet["Don't"]}`);
    if (motionSnippet["Performance Notes"]) lines.push(`- ⚡ ${motionSnippet["Performance Notes"]}`);
    lines.push("");
  }

  lines.push("---", "", "## Anti-Patterns (Do NOT Use)", "");
  if (antiPatterns) {
    for (const anti of antiPatterns.split("+").map((item) => item.trim())) if (anti) lines.push(`- ❌ ${anti}`);
  }
  lines.push(
    "",
    "### Additional Forbidden Patterns",
    "",
    "- ❌ **Emojis as icons** — Use SVG icons (Heroicons, Lucide, Material Symbols, Simple Icons)",
    "- ❌ **Missing cursor:pointer** — All clickable elements must have cursor:pointer",
    "- ❌ **Layout-shifting hovers** — Avoid scale transforms that shift layout",
    "- ❌ **Low contrast text** — Maintain 4.5:1 minimum contrast ratio",
    "- ❌ **Instant state changes** — Always use transitions (150-300ms)",
    "- ❌ **Invisible focus states** — Focus states must be visible for a11y, including forced-colors mode",
    "- ❌ **Lazy-loaded LCP image** — The LCP image gets `fetchPriority=\"high\"` and `loading=\"eager\"`",
    "",
    "---",
    "",
    "## Pre-Delivery Checklist",
    "",
    "Before delivering any UI code, verify:",
    "",
    "- [ ] No emojis used as icons (use SVG instead)",
    "- [ ] All icons from consistent icon set (Heroicons/Lucide/Material Symbols)",
    "- [ ] `cursor-pointer` on all clickable elements",
    "- [ ] Hover states with smooth transitions (150-300ms)",
    "- [ ] Light mode: text contrast 4.5:1 minimum",
    "- [ ] Focus states visible for keyboard navigation",
    "- [ ] Focus visible in `forced-colors: active` (`outline-color: CanvasText`)",
    "- [ ] `prefers-reduced-motion` respected",
    "- [ ] `color-scheme` declared for light/dark",
    "- [ ] LCP image uses `fetchPriority=\"high\"` and `loading=\"eager\"`",
    "- [ ] Features limited to Baseline widely available (or progressively enhanced)",
    "- [ ] Responsive: 375px, 768px, 1024px, 1440px",
    "- [ ] No content hidden behind fixed navbars",
    "- [ ] No horizontal scroll on mobile",
    "",
  );
  return lines.join("\n");
}

/** Python's str.title(). */
function titleCase(text: string): string {
  let out = "";
  let previousCased = false;
  for (const ch of text) {
    const cased = ch.toLowerCase() !== ch.toUpperCase();
    out += cased ? (previousCased ? ch.toLowerCase() : ch.toUpperCase()) : ch;
    previousCased = cased;
  }
  return out;
}

/** Format a page-specific override file. */
export async function formatPageOverrideMd(designSystem: DesignSystem, pageName: string, pageQuery: string | null = null): Promise<string> {
  const pageTitle = titleCase(pageName.replaceAll("-", " ").replaceAll("_", " "));
  const overrides = await generateIntelligentOverrides(pageName, pageQuery, designSystem);
  const lines: string[] = [
    `# ${pageTitle} Page Overrides`,
    "",
    `> **PROJECT:** ${designSystem.project_name}`,
    `> **Generated:** ${timestamp()}`,
    `> **Page Type:** ${overrides.page_type || "General"}`,
    "",
    "> ⚠️ **IMPORTANT:** Rules in this file **override** the Master file (`design-system/MASTER.md`).",
    "> Only deviations from the Master are documented here. For all other rules, refer to the Master.",
    "",
    "---",
    "",
    "## Page-Specific Rules",
    "",
  ];
  const section = (title: string, values: Record<string, string>, empty: string) => {
    lines.push(`### ${title}`, "");
    const entries = Object.entries(values);
    if (entries.length) for (const [key, value] of entries) lines.push(`- **${key}:** ${value}`);
    else lines.push(empty);
    lines.push("");
  };
  section("Layout Overrides", overrides.layout, "- No overrides — use Master layout");
  section("Spacing Overrides", overrides.spacing, "- No overrides — use Master spacing");
  section("Typography Overrides", overrides.typography, "- No overrides — use Master typography");
  section("Color Overrides", overrides.colors, "- No overrides — use Master colors");
  lines.push("### Component Overrides", "");
  if (overrides.components.length) for (const item of overrides.components) lines.push(`- ${item}`);
  else lines.push("- No overrides — use Master component specs");
  lines.push("", "---", "", "## Page-Specific Components", "");
  if (overrides.unique_components.length) for (const item of overrides.unique_components) lines.push(`- ${item}`);
  else lines.push("- No unique components for this page");
  lines.push("", "---", "", "## Recommendations", "");
  for (const item of overrides.recommendations) lines.push(`- ${item}`);
  lines.push("");
  return lines.join("\n");
}

interface PageOverrides {
  page_type: string;
  layout: Record<string, string>;
  spacing: Record<string, string>;
  typography: Record<string, string>;
  colors: Record<string, string>;
  components: string[];
  unique_components: string[];
  recommendations: string[];
}

/** Page overrides from layered style/UX/landing searches. */
async function generateIntelligentOverrides(pageName: string, pageQuery: string | null, _designSystem: DesignSystem): Promise<PageOverrides> {
  const combined = `${pageName.toLowerCase()} ${(pageQuery ?? "").toLowerCase()}`;
  const styleResults = (await search(combined, "style", 1)).results ?? [];
  const uxResults = (await search(combined, "ux", 3)).results ?? [];
  const landingResults = (await search(combined, "landing", 1)).results ?? [];
  const pageType = detectPageType(combined, styleResults);
  const layout: Record<string, string> = {};
  const spacing: Record<string, string> = {};
  const colors: Record<string, string> = {};
  const components: string[] = [];
  let recommendations: string[] = [];

  const style = styleResults[0];
  if (style) {
    const keywords = (style["Keywords"] ?? "").toLowerCase();
    const effects = style["Effects & Animation"] ?? "";
    if (["data", "dense", "dashboard", "grid"].some((kw) => keywords.includes(kw))) {
      layout["Max Width"] = "1400px or full-width";
      layout["Grid"] = "12-column grid for data flexibility";
      spacing["Content Density"] = "High — optimize for information display";
    } else if (["minimal", "simple", "clean", "single"].some((kw) => keywords.includes(kw))) {
      layout["Max Width"] = "800px (narrow, focused)";
      layout["Layout"] = "Single column, centered";
      spacing["Content Density"] = "Low — focus on clarity";
    } else {
      layout["Max Width"] = "1200px (standard)";
      layout["Layout"] = "Full-width sections, centered content";
    }
    if (effects) recommendations.push(`Effects: ${effects}`);
  }
  for (const ux of uxResults) {
    if (ux["Do"]) recommendations.push(`${ux["Category"] ?? ""}: ${ux["Do"]}`);
    if (ux["Don't"]) components.push(`Avoid: ${ux["Don't"]}`);
  }
  const landing = landingResults[0];
  if (landing) {
    if (landing["Section Order"]) layout["Sections"] = landing["Section Order"];
    if (landing["Primary CTA Placement"]) recommendations.push(`CTA Placement: ${landing["Primary CTA Placement"]}`);
    if (landing["Color Strategy"]) colors["Strategy"] = landing["Color Strategy"];
  }
  if (Object.keys(layout).length === 0) {
    layout["Max Width"] = "1200px";
    layout["Layout"] = "Responsive grid";
  }
  if (recommendations.length === 0) {
    recommendations = ["Refer to MASTER.md for all design rules", "Add specific overrides as needed for this page"];
  }
  return { page_type: pageType, layout, spacing, typography: {}, colors, components, unique_components: [], recommendations };
}

/** Detect the page type from context and search results. */
export function detectPageType(context: string, styleResults: Row[]): string {
  const lowered = context.toLowerCase();
  const patterns: [string[], string][] = [
    [["dashboard", "admin", "analytics", "data", "metrics", "stats", "monitor", "overview"], "Dashboard / Data View"],
    [["checkout", "payment", "cart", "purchase", "order", "billing"], "Checkout / Payment"],
    [["settings", "profile", "account", "preferences", "config"], "Settings / Profile"],
    [["landing", "marketing", "homepage", "hero", "home", "promo"], "Landing / Marketing"],
    [["login", "signin", "signup", "register", "auth", "password"], "Authentication"],
    [["pricing", "plans", "subscription", "tiers", "packages"], "Pricing / Plans"],
    [["blog", "article", "post", "news", "content", "story"], "Blog / Article"],
    [["product", "item", "detail", "pdp", "shop", "store"], "Product Detail"],
    [["search", "results", "browse", "filter", "catalog", "list"], "Search Results"],
    [["empty", "404", "error", "not found", "zero"], "Empty State"],
  ];
  for (const [keywords, pageType] of patterns) {
    if (keywords.some((kw) => lowered.includes(kw))) return pageType;
  }
  const first = styleResults[0];
  if (first) {
    const bestFor = (first["Best For"] ?? "").toLowerCase();
    if (bestFor.includes("dashboard") || bestFor.includes("data")) return "Dashboard / Data View";
    if (bestFor.includes("landing") || bestFor.includes("marketing")) return "Landing / Marketing";
  }
  return "General";
}

/** Re-export for callers that load CSVs alongside the generator. */
export { loadCsv };
