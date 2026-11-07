/**
 * UI/UX Pro Max core: calibrated BM25 + identity search over the local CSV
 * catalogs. Runs on Bun only (Bun.file, no dependencies).
 */

import { join } from "node:path";
import { parseCsvRecords, type Row } from "./csv.ts";
import { sequenceRatio } from "./difflib.ts";
import { escapeRegExp, phrasePattern } from "./reasoning-contract.ts";

export type { Row } from "./csv.ts";

// ============ CONFIGURATION ============
/** Catalog directory; `UIPRO_DATA_DIR` points the engine at another catalog (fixtures). */
export const DATA_DIR = Bun.env.UIPRO_DATA_DIR ?? join(import.meta.dir, "..", "data");
export const MAX_RESULTS = 3;

export interface DomainConfig {
  file: string;
  searchCols: readonly string[];
  outputCols: readonly string[];
}

export const CSV_CONFIG = {
  style: {
    file: "styles.csv",
    searchCols: ["Style ID", "Style Category", "Aliases", "Keywords", "Best For", "Type", "AI Prompt Keywords"],
    outputCols: ["Style ID", "Style Category", "Aliases", "Status", "Parent Style ID", "Preferred Mode", "Type", "Keywords", "Primary Colors", "Effects & Animation", "Best For", "Light Mode ✓", "Dark Mode ✓", "Performance", "Accessibility", "Framework Compatibility", "Complexity", "AI Prompt Keywords", "CSS/Technical Keywords", "Implementation Checklist", "Design System Variables"],
  },
  color: {
    file: "colors.csv",
    searchCols: ["Product Type", "Notes"],
    outputCols: ["Product Type", "Primary", "On Primary", "Secondary", "On Secondary", "Accent", "On Accent", "Background", "Foreground", "Card", "Card Foreground", "Muted", "Muted Foreground", "Border", "Destructive", "On Destructive", "Ring", "Notes"],
  },
  chart: {
    file: "charts.csv",
    searchCols: ["Data Type", "Keywords", "Best Chart Type", "When to Use", "When NOT to Use", "Accessibility Notes"],
    outputCols: ["Data Type", "Keywords", "Best Chart Type", "Secondary Options", "When to Use", "When NOT to Use", "Data Volume Threshold", "Color Guidance", "Accessibility Grade", "Accessibility Risk", "Accessibility Notes", "A11y Fallback", "Library Recommendation", "Interactive Level"],
  },
  landing: {
    file: "landing.csv",
    searchCols: ["Pattern ID", "Pattern Name", "Aliases", "Keywords", "Conversion Optimization", "Section Order"],
    outputCols: ["Pattern ID", "Pattern Name", "Aliases", "Keywords", "Section Order", "Primary CTA Placement", "Color Strategy", "Conversion Optimization"],
  },
  product: {
    file: "products.csv",
    searchCols: ["Product Type", "Keywords", "Primary Style Recommendation", "Key Considerations"],
    outputCols: ["Product Type", "Keywords", "Primary Style Recommendation", "Secondary Styles", "Landing Page Pattern", "Dashboard Style (if applicable)", "Color Palette Focus"],
  },
  ux: {
    file: "ux-guidelines.csv",
    searchCols: ["Category", "Issue", "Description", "Platform"],
    outputCols: ["Category", "Issue", "Platform", "Description", "Do", "Don't", "Code Example Good", "Code Example Bad", "Severity"],
  },
  typography: {
    file: "typography.csv",
    searchCols: ["Font Pairing Name", "Category", "Mood/Style Keywords", "Best For", "Heading Font", "Body Font"],
    outputCols: ["Font Pairing Name", "Category", "Heading Font", "Body Font", "Mood/Style Keywords", "Best For", "Google Fonts URL", "CSS Import", "Tailwind Config", "Notes"],
  },
  icons: {
    file: "icons.csv",
    searchCols: ["Category", "Icon Name", "Keywords", "Best For", "Library"],
    outputCols: ["Category", "Icon Name", "Keywords", "Library", "Import Code", "Usage", "Best For", "Style", "Semantic Role", "Allowed Contexts"],
  },
  gsap: {
    file: "motion.csv",
    searchCols: ["Category", "Intensity Tier", "Keywords", "Trigger"],
    outputCols: ["Category", "Intensity Tier", "Trigger", "Duration", "Easing", "GSAP Snippet", "Framework Notes", "Do", "Don't", "Performance Notes"],
  },
  react: {
    file: "react-performance.csv",
    searchCols: ["Category", "Issue", "Keywords", "Description"],
    outputCols: ["Category", "Issue", "Platform", "Description", "Do", "Don't", "Code Example Good", "Code Example Bad", "Severity"],
  },
  web: {
    file: "app-interface.csv",
    searchCols: ["Category", "Issue", "Keywords", "Description"],
    outputCols: ["Category", "Issue", "Platform", "Description", "Do", "Don't", "Code Example Good", "Code Example Bad", "Severity"],
  },
  "google-fonts": {
    file: "google-fonts.csv",
    searchCols: ["Family", "Category", "Stroke", "Classifications", "Keywords", "Subsets", "Designers"],
    outputCols: ["Family", "Category", "Stroke", "Classifications", "Styles", "Variable Axes", "Subsets", "Designers", "Popularity Rank", "Google Fonts URL"],
  },
  brand: {
    file: "brands.csv",
    searchCols: ["Brand ID", "Brand", "Theme", "Package", "Selector", "Color Scheme", "Heading Font", "Body Font", "Keywords"],
    outputCols: ["Brand ID", "Brand", "Theme", "Package", "Source File", "Selector", "Color Scheme", "Primary", "On Primary", "Secondary", "On Secondary", "Accent", "On Accent", "Background", "Foreground", "Card", "Card Foreground", "Muted", "Muted Foreground", "Border", "Destructive", "On Destructive", "Ring", "Radius", "Heading Font", "Body Font", "Mono Font", "Keywords", "Notes"],
  },
} as const satisfies Record<string, DomainConfig>;

export type Domain = keyof typeof CSV_CONFIG;
export const DOMAINS = Object.keys(CSV_CONFIG) as Domain[];

/**
 * Output columns whose content (code samples, checklists) must never be
 * hard-truncated for display: truncating mid-snippet destroys the value.
 */
export const UNTRUNCATED_COLS: ReadonlySet<string> = new Set([
  "Code Example Good", "Code Example Bad", "Code Good", "Code Bad",
  "Implementation Checklist", "Design System Variables", "CSS Import",
  "Tailwind Config", "GSAP Snippet",
]);

export const STACK_CONFIG: Readonly<Record<string, { file: string }>> = {
  react: { file: "stacks/react.csv" },
  nextjs: { file: "stacks/nextjs.csv" },
  vue: { file: "stacks/vue.csv" },
  svelte: { file: "stacks/svelte.csv" },
  astro: { file: "stacks/astro.csv" },
  swiftui: { file: "stacks/swiftui.csv" },
  "react-native": { file: "stacks/react-native.csv" },
  flutter: { file: "stacks/flutter.csv" },
  nuxtjs: { file: "stacks/nuxtjs.csv" },
  "nuxt-ui": { file: "stacks/nuxt-ui.csv" },
  "html-tailwind": { file: "stacks/html-tailwind.csv" },
  shadcn: { file: "stacks/shadcn.csv" },
  m3: { file: "stacks/m3.csv" },
  "rg-ui": { file: "stacks/rg-ui.csv" },
  "jetpack-compose": { file: "stacks/jetpack-compose.csv" },
  threejs: { file: "stacks/threejs.csv" },
  angular: { file: "stacks/angular.csv" },
  laravel: { file: "stacks/laravel.csv" },
  javafx: { file: "stacks/javafx.csv" },
  wpf: { file: "stacks/wpf.csv" },
  winui: { file: "stacks/winui.csv" },
  avalonia: { file: "stacks/avalonia.csv" },
  uno: { file: "stacks/uno.csv" },
  uwp: { file: "stacks/uwp.csv" },
};

/** Common columns for all stacks. */
export const STACK_COLS = {
  searchCols: ["Category", "Guideline", "Description", "Do", "Don't", "Code Good", "Code Bad"],
  outputCols: ["Category", "Guideline", "Description", "Do", "Don't", "Code Good", "Code Bad", "Severity", "Docs URL", "Applies To", "Status", "Verified At"],
} as const;

export const WEB_STACK_CURRENT_MAJORS: Readonly<Record<string, number>> = {
  react: 19,
  nextjs: 16,
  vue: 3,
  svelte: 5,
  astro: 7,
  angular: 22,
  "html-tailwind": 4,
  nuxtjs: 4,
  "nuxt-ui": 4,
};
export const WEB_STACKS: ReadonlySet<string> = new Set([...Object.keys(WEB_STACK_CURRENT_MAJORS), "shadcn", "m3", "rg-ui"]);

export const STACK_CURRENT_VERSIONS: Readonly<Record<string, readonly number[]>> = {
  ...Object.fromEntries(Object.entries(WEB_STACK_CURRENT_MAJORS).map(([stack, major]) => [stack, [major]])),
  "react-native": [0, 86],
  flutter: [3, 44],
  swiftui: [16],
  "jetpack-compose": [1, 11],
  avalonia: [12],
  winui: [3],
  javafx: [26],
  threejs: [0, 185],
  laravel: [13],
  m3: [1, 4],
  "rg-ui": [1, 4],
};
export const LEGACY_ONLY_STACKS: ReadonlySet<string> = new Set(["uwp"]);
export const STACK_CURRENT_APPLICABILITY: Readonly<Record<string, string>> = {
  react: "react 19.2.x",
  nextjs: "nextjs 16.2",
  vue: "vue 3.5.x",
  svelte: "svelte 5",
  astro: "astro 7.1.6",
  angular: "angular 22.x",
  "html-tailwind": "html-tailwind 4.3",
  shadcn: "shadcn cli 4",
  m3: "m3 1.4.4",
  "rg-ui": "rg-ui 1.4.4",
  nuxtjs: "nuxtjs 4.5",
  "nuxt-ui": "nuxt-ui 4.11.1",
  "react-native": "react-native 0.86.x",
  flutter: "flutter 3.44.x",
  swiftui: "swiftui current",
  "jetpack-compose": "jetpack-compose 1.11.4",
  avalonia: "avalonia 12",
  uwp: "uwp legacy",
  winui: "winui current",
  wpf: "wpf current",
  uno: "uno current",
  javafx: "javafx 26",
  threejs: "threejs 0.185.1",
  laravel: "laravel 13.x",
};

const STACK_QUERY_NAMES: Readonly<Record<string, string>> = {
  react: "react",
  nextjs: "next(?:\\.js|js)?",
  vue: "vue",
  svelte: "svelte",
  astro: "astro",
  angular: "angular",
  "html-tailwind": "tailwind(?:\\s*css)?",
  nuxtjs: "nuxt(?:\\.js|js)?",
  "nuxt-ui": "nuxt\\s*ui",
  "react-native": "react[\\s-]*native",
  flutter: "flutter",
  swiftui: "(?:ios|swiftui\\s+ios)",
  "jetpack-compose": "(?:jetpack\\s*)?compose",
  avalonia: "avalonia",
  winui: "winui",
  javafx: "javafx",
  threejs: "three(?:\\.js|js)?",
  laravel: "laravel",
  m3: "(?:@aphrody/)?m3",
  "rg-ui": "rg[\\s-]*ui|rose\\s*griffon\\s*ui|@rosegriffon/ui",
};

export const AVAILABLE_STACKS = Object.keys(STACK_CONFIG);

const INDEX_VERSION = 2;
export const SEARCH_CALIBRATION_VERSION = "2026-08-12-v1";

export interface Threshold {
  min_score: number;
  min_margin: number;
  min_coverage: number;
}

/**
 * Search calibration uses evidence coverage first; raw BM25 floors are kept
 * domain-specific because corpora vary greatly in size and document length.
 */
const DOMAIN_SCORE_FLOORS: Partial<Record<Domain, number>> = {
  style: 4.3, landing: 4.0, product: 6.0, icons: 5.8, react: 3.3,
};
export const SEARCH_THRESHOLDS = Object.fromEntries(
  DOMAINS.map((domain) => [
    domain,
    { min_score: DOMAIN_SCORE_FLOORS[domain] ?? 0, min_margin: 0, min_coverage: domain === "landing" ? 0.5 : 0 },
  ]),
) as Record<Domain, Threshold>;
export const STACK_THRESHOLD: Threshold = { min_score: 3.6, min_margin: 0, min_coverage: 1 / 3 };
export const NO_THRESHOLD: Threshold = { min_score: 0, min_margin: 0, min_coverage: 0 };
const STYLE_IDENTITY_FIELDS = ["Style ID", "Style Category", "Aliases"];
const LANDING_IDENTITY_FIELDS = ["Pattern ID", "Pattern Name", "Aliases"];
const BRAND_IDENTITY_FIELDS = ["Brand ID", "Brand", "Theme", "Aliases"];
export const DOMAIN_QUERY_REWRITES: Partial<Record<Domain, Record<string, string | null>>> = {
  color: Object.fromEntries(
    ["color", "palette", "hex", "rgb", "token", "semantic", "destructive", "muted", "foreground"].map((term) => [term, null]),
  ),
  landing: { testimonial: "testimonials" },
  style: { css: null, implementation: null, variable: null, checklist: null, tailwind: null },
  ux: { ux: "accessibility", usability: "accessibility", wcag: "accessibility" },
  "google-fonts": { typography: "font" },
  icons: { lucide: null, symbol: null, glyph: null, pictogram: null },
  gsap: { gsap: "animation", quickto: null, scrolltrigger: "scroll", "flip plugin": null, splittext: null },
  react: { nextjs: "react", usecallback: "memoization", useeffect: "effects" },
  web: { aria: "accessibility", outline: "focus", semantic: null, autocomplete: "input", preconnect: null },
  brand: { "brand palette": "theme", "brand tokens": "theme" },
};

// ============ TOKENIZATION ============
const WORD = "[\\p{L}\\p{N}_]";
/** Python's Unicode-aware `\b` around a word token. */
const B_START = `(?<!${WORD})`;
const B_END = `(?!${WORD})`;

/**
 * Common short words that add noise without adding search signal. Domain
 * tokens such as ui, ux, ai, css, 3d, js, os, md and gsap stay searchable.
 */
const STOPWORDS = new Set([
  "to", "in", "on", "at", "is", "of", "by", "or", "an", "if", "no", "so",
  "do", "be", "we", "it", "as", "the", "and", "for", "are", "was",
]);

/** Query/corpus normalization so common spelling variants match each other. */
const SYNONYMS: Readonly<Record<string, string>> = {
  "q&a": "question answer",
  "e-commerce": "ecommerce",
  "dark-mode": "dark",
  darkmode: "dark",
  "light-mode": "light",
  lightmode: "light",
  a11y: "accessibility",
  nav: "navigation",
  "sign-up": "signup",
  "log-in": "login",
  colour: "color",
  colours: "colors",
  customisation: "customization",
  organisation: "organization",
  behaviour: "behavior",
  "ux/ui": "ux ui",
};

const SYNONYM_PATTERNS: [RegExp, string][] = Object.entries(SYNONYMS)
  .sort((a, b) => b[0].length - a[0].length)
  .map(([variant, canonical]) => [phrasePattern(variant, "giu"), canonical]);

/** Apply longest-first synonym substitution at token boundaries. */
export function normalize(text: unknown): string {
  let normalized = String(text);
  for (const [pattern, canonical] of SYNONYM_PATTERNS) {
    normalized = normalized.replace(pattern, () => canonical);
  }
  return normalized;
}

const NON_WORD = /[^\p{L}\p{N}_\s]/gu;

function pyLength(text: string): number {
  let length = 0;
  for (const _ of text) length++;
  return length;
}

// ============ BM25 IMPLEMENTATION ============
export class BM25 {
  corpus: string[][] = [];
  docLengths: number[] = [];
  avgdl = 0;
  idf = new Map<string, number>();
  docFreqs = new Map<string, number>();
  N = 0;
  private termFreqs: Map<string, number>[] = [];

  constructor(
    readonly k1 = 1.5,
    readonly b = 0.75,
  ) {}

  /** Lowercase, normalize synonyms, split, remove punctuation, filter stopwords. */
  tokenize(text: unknown): string[] {
    const cleaned = normalize(String(text).toLowerCase()).replace(NON_WORD, " ");
    return cleaned.split(/\s+/).filter((word) => word && pyLength(word) >= 2 && !STOPWORDS.has(word));
  }

  fit(documents: readonly string[]): void {
    this.corpus = documents.map((doc) => this.tokenize(doc));
    this.N = this.corpus.length;
    if (this.N === 0) return;
    this.docLengths = this.corpus.map((doc) => doc.length);
    this.avgdl = this.docLengths.reduce((sum, length) => sum + length, 0) / this.N || 1;
    this.termFreqs = [];
    for (const doc of this.corpus) {
      const tf = new Map<string, number>();
      for (const word of doc) tf.set(word, (tf.get(word) ?? 0) + 1);
      this.termFreqs.push(tf);
      for (const word of tf.keys()) this.docFreqs.set(word, (this.docFreqs.get(word) ?? 0) + 1);
    }
    for (const [word, freq] of this.docFreqs) {
      this.idf.set(word, Math.log((this.N - freq + 0.5) / (freq + 0.5) + 1));
    }
  }

  /** Score all documents against the query, best first (stable). */
  score(query: string): [number, number][] {
    const queryTokens = this.tokenize(query);
    const scores: [number, number][] = [];
    for (let index = 0; index < this.N; index++) {
      let score = 0;
      const docLength = this.docLengths[index]!;
      const termFreqs = this.termFreqs[index]!;
      for (const token of queryTokens) {
        const idf = this.idf.get(token);
        if (idf === undefined) continue;
        const tf = termFreqs.get(token) ?? 0;
        const numerator = tf * (this.k1 + 1);
        const denominator = tf + this.k1 * (1 - this.b + (this.b * docLength) / this.avgdl);
        score += (idf * numerator) / denominator;
      }
      scores.push([index, score]);
    }
    return scores.sort((a, b) => b[1] - a[1]);
  }

  /** All indexed terms, for suggestion/typo-recovery purposes. */
  vocabulary(): string[] {
    return [...this.idf.keys()];
  }
}

// ============ CSV / INDEX CACHE ============
type Signature = `${number}:${number}`;
const csvCache = new Map<string, { signature: Signature; rows: Row[] }>();
const bm25Cache = new Map<string, { signature: Signature; index: BM25 }>();

async function fileSignature(path: string): Promise<Signature> {
  const file = Bun.file(path);
  const stat = await file.stat();
  return `${stat.mtimeMs}:${stat.size}`;
}

async function loadCsvSnapshot(path: string, attempts = 3): Promise<{ rows: Row[]; signature: Signature }> {
  const signature = await fileSignature(path);
  const cached = csvCache.get(path);
  if (cached && cached.signature === signature) return cached;
  for (let attempt = 0; attempt < attempts; attempt++) {
    const before = await fileSignature(path);
    const rows = parseCsvRecords(await Bun.file(path).text());
    const after = await fileSignature(path);
    if (before === after) {
      const snapshot = { rows, signature: after };
      csvCache.set(path, snapshot);
      return snapshot;
    }
  }
  throw new Error(`File changed while reading: ${path}`);
}

/** Load CSV rows from a stable, signature-verified snapshot. */
export async function loadCsv(path: string): Promise<Row[]> {
  return (await loadCsvSnapshot(path)).rows;
}

async function getBm25(
  path: string,
  searchCols: readonly string[],
  data: readonly Row[],
  signature: Signature,
  cacheVariant = "",
): Promise<BM25> {
  const key = JSON.stringify([path, searchCols, INDEX_VERSION, cacheVariant]);
  const cached = bm25Cache.get(key);
  if (cached && cached.signature === signature) return cached.index;
  const documents = data.map((row) => searchCols.map((column) => String(row[column] ?? "")).join(" "));
  const index = new BM25();
  index.fit(documents);
  bm25Cache.set(key, { signature, index });
  return index;
}

// ============ SEARCH FUNCTIONS ============
export interface Diagnostic {
  normalized_query?: string;
  search_query?: string;
  query_rewrites?: string[];
  top_score?: number;
  runner_up_score?: number;
  margin?: number;
  token_coverage?: number;
  abstained?: boolean;
  calibration_version?: string;
  reason: string;
  error?: string;
}

export interface SearchResult {
  domain?: string | null;
  stack?: string;
  query?: string;
  file?: string;
  count?: number;
  results?: Row[];
  auto_detected?: boolean;
  runner_up_domain?: string;
  redirect?: { domain: string; id: string };
  suggestions?: string[];
  diagnostics?: Diagnostic;
  error?: string;
}

function queryCoverage(index: BM25, query: string): number {
  const tokens = new Set(index.tokenize(query));
  if (tokens.size === 0) return 0;
  let hits = 0;
  for (const token of tokens) if (index.idf.has(token)) hits++;
  return hits / tokens.size;
}

function project(row: Row, columns: readonly string[]): Row {
  const out: Row = {};
  for (const column of columns) if (column in row) out[column] = row[column] ?? "";
  return out;
}

export type RowFilter = (row: Row) => boolean;

/** Calibrated search returning results, index, and internal diagnostics. */
export async function searchCsvDetailed(
  path: string,
  searchCols: readonly string[],
  outputCols: readonly string[],
  query: string,
  maxResults: number,
  threshold: Threshold | null = null,
  routingDomain: Domain | null = null,
  rowFilter: RowFilter | null = null,
  cacheVariant = "",
): Promise<{ results: Row[]; index: BM25 | null; diagnostic: Diagnostic }> {
  if (!(await Bun.file(path).exists())) return { results: [], index: null, diagnostic: { reason: "missing-file" } };

  let data: Row[];
  let signature: Signature;
  try {
    ({ rows: data, signature } = await loadCsvSnapshot(path));
  } catch {
    const name = path.split(/[\\/]/).pop();
    return { results: [], index: null, diagnostic: { reason: "read-error", error: `Unable to read search data: ${name}` } };
  }
  if (data.length === 0) return { results: [], index: null, diagnostic: { reason: "empty-data" } };
  if (rowFilter) {
    data = data.filter(rowFilter);
    if (data.length === 0) return { results: [], index: null, diagnostic: { reason: "empty-data" } };
  }

  const bm25 = await getBm25(path, searchCols, data, signature, cacheVariant);
  const [searchQuery, rewrites] = await rewriteQueryForDomain(query, routingDomain, bm25);
  const ranked = bm25.score(searchQuery);
  const limits = threshold ?? NO_THRESHOLD;
  const topScore = ranked[0]?.[1] ?? 0;
  const runnerUpScore = ranked[1]?.[1] ?? 0;
  const coverage = queryCoverage(bm25, searchQuery);
  const abstain =
    topScore <= limits.min_score ||
    coverage < limits.min_coverage ||
    (limits.min_margin > 0 && topScore - runnerUpScore < limits.min_margin);

  const results: Row[] = [];
  if (!abstain) {
    for (const [index, score] of ranked.slice(0, maxResults)) {
      if (score <= 0) continue;
      results.push(project(data[index]!, outputCols));
    }
  }
  return {
    results,
    index: bm25,
    diagnostic: {
      normalized_query: normalize(query),
      search_query: searchQuery,
      query_rewrites: rewrites,
      top_score: topScore,
      runner_up_score: runnerUpScore,
      margin: topScore - runnerUpScore,
      token_coverage: coverage,
      abstained: abstain,
      calibration_version: SEARCH_CALIBRATION_VERSION,
      reason: abstain ? "low-confidence" : "matched",
    },
  };
}

function passesThreshold(index: BM25, query: string, threshold: Threshold): boolean {
  const ranked = index.score(query);
  const topScore = ranked[0]?.[1] ?? 0;
  const runnerUpScore = ranked[1]?.[1] ?? 0;
  return (
    topScore > threshold.min_score &&
    queryCoverage(index, query) >= threshold.min_coverage &&
    (threshold.min_margin <= 0 || topScore - runnerUpScore >= threshold.min_margin)
  );
}

function compareTuples(a: readonly (number | string)[], b: readonly (number | string)[]): number {
  const length = Math.min(a.length, b.length);
  for (let i = 0; i < length; i++) {
    if (a[i]! < b[i]!) return -1;
    if (a[i]! > b[i]!) return 1;
  }
  return a.length - b.length;
}

/**
 * Nearest known vocabulary terms for a query that returned 0 hits, so the
 * caller can retry instead of silently reporting nothing.
 */
export function suggestTerms(bm25: BM25 | null, query: string, limit = 6, threshold: Threshold | null = null): string[] {
  if (!bm25) return [];
  const queryTokens = new Set(bm25.tokenize(query));
  if (queryTokens.size === 0) return [];
  const candidates: [number, number, string][] = [];
  for (const term of bm25.vocabulary()) {
    if (queryTokens.has(term)) continue;
    let similarity = 0;
    for (const token of queryTokens) similarity = Math.max(similarity, sequenceRatio(token, term));
    if (similarity >= 0.72 && (threshold === null || passesThreshold(bm25, term, threshold))) {
      candidates.push([-similarity, -(bm25.docFreqs.get(term) ?? 0), term]);
    }
  }
  return candidates.sort(compareTuples).slice(0, limit).map(([, , term]) => term);
}

/** Suggest complete public identities so a retry can bypass score thresholds. */
function suggestIdentities(rows: readonly Row[], query: string, fields: readonly string[], limit = 6): string[] {
  const tokenizer = new BM25();
  const queryTokens = new Set(tokenizer.tokenize(query));
  if (queryTokens.size === 0) return [];
  const folded = String(query).trim().toLowerCase();
  const candidates = new Map<string, [number, number, string]>();
  for (const row of rows) {
    for (const identity of rowIdentities(row, fields)) {
      const identityTokens = new Set(tokenizer.tokenize(identity));
      if (identityTokens.size === 0) continue;
      let similarity = 0;
      for (const source of queryTokens) {
        for (const target of identityTokens) similarity = Math.max(similarity, sequenceRatio(source, target));
      }
      if (similarity >= 0.72 && identity.toLowerCase() !== folded) {
        const candidate: [number, number, string] = [-similarity, identityTokens.size, identity];
        candidates.set(JSON.stringify(candidate), candidate);
      }
    }
  }
  return [...candidates.values()].sort(compareTuples).slice(0, limit).map(([, , identity]) => identity);
}

/** Return non-empty public identities from ordinary and alias fields. */
function rowIdentities(row: Row, fields: readonly string[]): string[] {
  const identities: string[] = [];
  for (const field of fields) {
    const values = field === "Aliases" ? (row[field] ?? "").split("|") : [row[field] ?? ""];
    for (const value of values) if (value.trim()) identities.push(value.trim());
  }
  return identities;
}

/**
 * High-signal product labels/aliases from products.csv (never every corpus
 * keyword), falling back to a small seed when the file is missing.
 */
async function loadProductKeywords(): Promise<string[]> {
  const seed = [
    "saas", "ecommerce", "fintech", "healthcare", "gaming", "portfolio",
    "crypto", "fitness", "marketplace", "banking", "cybersecurity",
    "education", "travel", "restaurant", "real estate", "social media",
    "beauty", "spa", "salon", "wellness", "booking",
  ];
  const path = join(DATA_DIR, CSV_CONFIG.product.file);
  if (!(await Bun.file(path).exists())) return seed;
  let rows: Row[];
  try {
    rows = await loadCsv(path);
  } catch {
    return seed;
  }
  const keywords = new Set(seed);
  for (const row of rows) {
    const label = (row["Product Type"] ?? "").replace(/\([^)]*\)/g, "").trim().toLowerCase();
    if (pyLength(label) >= 4) keywords.add(label);
  }
  return [...keywords].sort((a, b) => pyLength(b) - pyLength(a));
}

let domainKeywordsCache: { signature: Signature | null; keywords: Record<Domain, string[]> } | null = null;

export async function domainKeywords(): Promise<Record<Domain, string[]>> {
  const productPath = join(DATA_DIR, CSV_CONFIG.product.file);
  const signature = (await Bun.file(productPath).exists()) ? await fileSignature(productPath) : null;
  if (domainKeywordsCache && domainKeywordsCache.signature === signature) return domainKeywordsCache.keywords;
  const keywords: Record<Domain, string[]> = {
    color: ["color", "palette", "hex", "rgb", "token", "semantic", "accent", "destructive", "muted", "foreground"],
    chart: ["time series", "chart", "graph", "visualization", "trend", "bar chart", "pie", "scatter", "heatmap", "funnel", "forecast"],
    landing: ["landing", "page", "cta", "conversion", "hero", "testimonial", "pricing", "section"],
    product: await loadProductKeywords(),
    style: ["style", "design", "ui", "minimalism", "glassmorphism", "neumorphism", "brutalism", "dark mode", "flat", "aurora", "css", "implementation", "variable", "checklist", "tailwind"],
    ux: ["ux", "usability", "accessibility", "wcag", "touch", "scroll", "animation", "keyboard", "navigation", "mobile"],
    typography: ["font pairing", "typography pairing", "heading font", "body font"],
    "google-fonts": ["google font", "font family", "font weight", "font style", "variable font", "noto", "font for", "find font", "font subset", "font language", "monospace font", "serif font", "sans serif font", "display font", "handwriting font", "font", "typography", "serif", "sans"],
    icons: ["icon", "icons", "lucide", "phosphor", "heroicons", "symbol", "glyph", "pictogram", "svg icon"],
    gsap: ["gsap", "quickto", "scrolltrigger", "stagger", "magnetic cursor", "parallax", "page transition", "scroll reveal", "scroll-triggered", "scrollytelling", "flip plugin", "splittext", "shimmer", "skeleton loader"],
    react: ["react", "next.js", "nextjs", "suspense", "memo", "usecallback", "useeffect", "rerender", "bundle", "waterfall", "barrel", "dynamic import", "rsc", "server component"],
    web: ["aria", "focus", "outline", "semantic", "virtualize", "autocomplete", "form", "input type", "preconnect", "drag reorder", "single pointer", "touch target", "native accessibility"],
    brand: ["brand theme", "brand palette", "brand tokens", "rose griffon", "rosegriffon", "azalee", "azalée", "achillea", "achilléa", "gaelle", "gaëlle", "rg-ui theme", "bun os brand", "aphrody brand"],
  };
  domainKeywordsCache = { signature, keywords };
  return keywords;
}

function containsPhrase(text: string, phrase: string): boolean {
  if (/[\p{L}\p{N}_]/u.test(phrase)) return phrasePattern(phrase).test(text);
  return text.includes(phrase);
}

/** Apply only explicit, semantic rewrites for routing-only vocabulary. */
async function rewriteQueryForDomain(query: string, domain: Domain | null, index: BM25): Promise<[string, string[]]> {
  const keywordsByDomain = await domainKeywords();
  if (!domain || !(domain in keywordsByDomain)) return [query, []];
  const normalized = normalize(query.toLowerCase());
  const rewrites: string[] = [];
  const replacementTerms: string[] = [];
  for (const keyword of keywordsByDomain[domain]) {
    if (!containsPhrase(normalized, keyword)) continue;
    if (index.tokenize(keyword).some((token) => index.idf.has(token))) continue;
    const replacement = DOMAIN_QUERY_REWRITES[domain]?.[keyword];
    if (replacement) {
      rewrites.push(`${keyword}->${replacement}`);
      replacementTerms.push(replacement);
    }
  }
  if (replacementTerms.length === 0) return [query, []];
  const unique = (values: string[]) => [...new Set(values)].sort();
  return [`${query} ${unique(replacementTerms).join(" ")}`, unique(rewrites)];
}

/**
 * Domains checked in this fixed order when scores tie, so results are
 * deterministic instead of depending on object ordering.
 */
const DOMAIN_TIEBREAK_ORDER: Domain[] = [
  "ux", "product", "style", "color", "typography", "google-fonts",
  "chart", "landing", "icons", "gsap", "react", "web", "brand",
];

/**
 * Auto-detect the most relevant domain and its runner-up. Matches are
 * weighted by keyword length (multi-word phrases are more specific); ties are
 * broken by a fixed domain priority order.
 */
export async function detectDomainWithRunnerUp(query: string): Promise<[Domain, Domain | null]> {
  const queryLower = normalize(query.toLowerCase());
  const keywordsByDomain = await domainKeywords();
  const scores = new Map<Domain, number>();
  for (const [domain, keywords] of Object.entries(keywordsByDomain) as [Domain, string[]][]) {
    let total = 0;
    for (const keyword of keywords) {
      if (containsPhrase(queryLower, keyword)) {
        const specificity = Math.max(1, keyword.split(/\s+/).filter(Boolean).length);
        total += domain !== "product" ? 2 * specificity : specificity;
      }
    }
    scores.set(domain, total);
  }
  if (new RegExp(`(?<!${WORD})#[0-9a-f]{3,8}(?!${WORD})`, "iu").test(queryLower)) {
    scores.set("color", (scores.get("color") ?? 0) + 2);
  }
  const rank = (domain: Domain) => {
    const position = DOMAIN_TIEBREAK_ORDER.indexOf(domain);
    return position === -1 ? 999 : position;
  };
  const ranked = [...scores.entries()].sort((a, b) => b[1] - a[1] || rank(a[0]) - rank(b[0]));
  const [bestDomain, bestScore] = ranked[0]!;
  const result: Domain = bestScore > 0 ? bestDomain : "style";
  const runnerUp = ranked[1] && ranked[1][1] > 0 ? ranked[1][0] : null;
  return [result, runnerUp];
}

export async function detectDomain(query: string): Promise<Domain> {
  return (await detectDomainWithRunnerUp(query))[0];
}

function wordSet(text: string): Set<string> {
  return new Set(text.match(/[\p{L}\p{N}_]+/gu) ?? []);
}

function isSubset<T>(subset: Set<T>, superset: Set<T>): boolean {
  for (const value of subset) if (!superset.has(value)) return false;
  return true;
}

/** Resolve an explicit style identity without opening generic variant ranking. */
function styleIdentity(rows: readonly Row[], query: string, allowContained = true): Row | null {
  const folded = String(query ?? "").trim().toLowerCase();
  const queryTokens = wordSet(normalize(folded));
  const genericTokens = new Set(["app", "design", "interface", "style", "system", "ui"]);
  const candidates: [number, number, number, Row][] = [];
  for (const row of rows) {
    const identities = rowIdentities(row, STYLE_IDENTITY_FIELDS);
    if (identities.some((identity) => identity.toLowerCase() === folded)) return row;
    if (!allowContained) continue;
    for (const identity of identities) {
      const identityTokens = wordSet(normalize(identity.toLowerCase()));
      if (identityTokens.size && isSubset(identityTokens, queryTokens) && [...identityTokens].some((token) => pyLength(token) >= 4)) {
        const distinctive = [...identityTokens].filter((token) => !genericTokens.has(token)).length;
        candidates.push([distinctive, identityTokens.size, pyLength(identity), row]);
      }
    }
  }
  if (candidates.length === 0) return null;
  candidates.sort((a, b) => b[0] - a[0] || b[1] - a[1]);
  const best = candidates[0]!;
  const bestRows = new Map<string, Row>();
  for (const candidate of candidates) {
    if (candidate[0] === best[0] && candidate[1] === best[1] && candidate[2] === best[2]) {
      bestRows.set(candidate[3]["Style ID"] ?? "", candidate[3]);
    }
  }
  return bestRows.size === 1 ? [...bestRows.values()][0]! : null;
}

/** Return one row whose stable public identity exactly matches the query. */
function exactRowIdentity(rows: readonly Row[], query: string, fields: readonly string[]): Row | null {
  const folded = String(query ?? "").trim().toLowerCase();
  const matches = rows.filter((row) => rowIdentities(row, fields).some((identity) => identity.toLowerCase() === folded));
  return matches.length === 1 ? matches[0]! : null;
}

/** Load rows for optional identity routing, leaving search to report I/O errors. */
async function loadRowsOrEmpty(path: string): Promise<Row[]> {
  try {
    if (!(await Bun.file(path).exists())) return [];
    return await loadCsv(path);
  } catch {
    return [];
  }
}

function validMaxResults(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 1 && value <= 20;
}

function exactMatchDiagnostic(query: string, reason: string): Diagnostic {
  return {
    normalized_query: normalize(query),
    search_query: query,
    query_rewrites: [],
    top_score: 0,
    runner_up_score: 0,
    margin: 0,
    token_coverage: 1,
    abstained: false,
    calibration_version: SEARCH_CALIBRATION_VERSION,
    reason,
  };
}

/** Resolve a deprecated in-domain alias, or expose a cross-domain redirect. */
function styleSearchDestination(rows: readonly Row[], matched: Row | null): [Row | null, { domain: string; id: string } | null] {
  if (!matched || (matched["Status"] ?? "active") !== "deprecated") return [matched, null];
  const parentId = (matched["Parent Style ID"] ?? "").trim();
  if (parentId) return [rows.find((row) => row["Style ID"] === parentId) ?? null, null];
  const domain = (matched["Replacement Domain"] ?? "").trim();
  const replacementId = (matched["Replacement ID"] ?? "").trim();
  if (domain === "style" && replacementId) return [rows.find((row) => row["Style ID"] === replacementId) ?? null, null];
  if (domain && replacementId) return [null, { domain, id: replacementId }];
  return [null, null];
}

function isDomain(value: unknown): value is Domain {
  return typeof value === "string" && value in CSV_CONFIG;
}

/** Main search function with auto-domain detection. */
export async function search(
  query: string,
  domain: string | null = null,
  maxResults: number = MAX_RESULTS,
  diagnostics = false,
): Promise<SearchResult> {
  if (!validMaxResults(maxResults)) return { error: "max_results must be an integer from 1 to 20", domain };
  const autoDetected = domain === null;
  let runnerUp: Domain | null = null;
  let styleRows: Row[] | null = null;
  let exact: Row | null = null;
  let redirect: { domain: string; id: string } | null = null;
  let landingRows: Row[] = [];
  let brandRows: Row[] = [];
  if (domain === null) {
    styleRows = await loadRowsOrEmpty(join(DATA_DIR, CSV_CONFIG.style.file));
    const matchedStyle = styleIdentity(styleRows, query, false);
    if (matchedStyle) {
      domain = "style";
      [exact, redirect] = styleSearchDestination(styleRows, matchedStyle);
    } else {
      [domain, runnerUp] = await detectDomainWithRunnerUp(query);
    }
  }

  const searchDomain: Domain = isDomain(domain) ? domain : "style";
  const config: DomainConfig = CSV_CONFIG[searchDomain];
  const path = join(DATA_DIR, config.file);
  if (!(await Bun.file(path).exists())) return { error: `File not found: ${path}`, domain };

  if (searchDomain === "style" && exact === null && redirect === null) {
    styleRows ??= await loadRowsOrEmpty(path);
    [exact, redirect] = styleSearchDestination(styleRows, styleIdentity(styleRows, query));
  } else if (searchDomain === "landing") {
    landingRows = await loadRowsOrEmpty(path);
    exact = exactRowIdentity(landingRows, query, LANDING_IDENTITY_FIELDS);
  } else if (searchDomain === "brand") {
    brandRows = await loadRowsOrEmpty(path);
    exact = exactRowIdentity(brandRows, query, BRAND_IDENTITY_FIELDS);
  }

  let results: Row[];
  let bm25: BM25 | null = null;
  let diagnostic: Diagnostic;
  if (exact) {
    results = [project(exact, config.outputCols)];
    diagnostic = exactMatchDiagnostic(query, "exact-identity");
  } else if (redirect) {
    results = [];
    diagnostic = {
      normalized_query: normalize(query),
      search_query: query,
      query_rewrites: [],
      abstained: true,
      calibration_version: SEARCH_CALIBRATION_VERSION,
      reason: "cross-domain-redirect",
    };
  } else {
    ({ results, index: bm25, diagnostic } = await searchCsvDetailed(
      path,
      config.searchCols,
      config.outputCols,
      query,
      maxResults,
      SEARCH_THRESHOLDS[searchDomain],
      searchDomain,
      searchDomain === "style" ? (row) => (row["Status"] ?? "active") === "active" : null,
      searchDomain === "style" ? "active-only" : "",
    ));
  }

  if (searchDomain === "icons" && containsPhrase(normalize(query.toLowerCase()), "lucide")) {
    results = [];
    diagnostic = { ...diagnostic, abstained: true, reason: "unsupported-library" };
  }

  const out: SearchResult = { domain, query, file: config.file, count: results.length, results };
  if (autoDetected) {
    out.auto_detected = true;
    if (runnerUp) out.runner_up_domain = runnerUp;
  }
  if (redirect) out.redirect = redirect;
  if (diagnostic.error) out.error = diagnostic.error;
  if (results.length === 0) {
    if (searchDomain === "landing") out.suggestions = suggestIdentities(landingRows, query, LANDING_IDENTITY_FIELDS);
    else if (searchDomain === "brand" && !bm25) out.suggestions = suggestIdentities(brandRows, query, BRAND_IDENTITY_FIELDS);
    else out.suggestions = suggestTerms(bm25, query, 6, SEARCH_THRESHOLDS[searchDomain]);
  }
  if (diagnostics) out.diagnostics = diagnostic;
  return out;
}

const MIGRATION_INTENT = new RegExp(`${B_START}(?:migrat${WORD}*|upgrad${WORD}*|replac${WORD}*|instead|modern|current)${B_END}`, "u");

/** Whether a stack query explicitly targets an older framework generation. */
function stackQueryRequestsLegacy(query: string, stack: string): boolean {
  const normalized = normalize(String(query ?? "").toLowerCase());
  if (LEGACY_ONLY_STACKS.has(stack)) return true;
  const currentVersion = STACK_CURRENT_VERSIONS[stack];
  const stackName = STACK_QUERY_NAMES[stack];
  if (currentVersion && stackName) {
    const pattern = new RegExp(
      `${B_START}(?:${stackName})\\s*(?:sdk|ui)?\\s*(?:[@(]\\s*)?(?:v(?:ersion)?\\s*)?(\\d+)(?:\\.(\\d+))?\\s*\\)?`,
      "gu",
    );
    const requested: number[][] = [];
    for (const match of normalized.matchAll(pattern)) {
      requested.push(match.slice(1).filter((value) => value !== undefined).map(Number));
    }
    if (stack === "threejs") {
      for (const match of normalized.matchAll(new RegExp(`${B_START}r(\\d+)${B_END}`, "gu"))) requested.push([0, Number(match[1])]);
    }
    const migrationIntent = MIGRATION_INTENT.test(normalized);
    if (requested.length) {
      if (migrationIntent && requested.some((version) => compareTuples(version, currentVersion.slice(0, version.length)) >= 0)) {
        return false;
      }
      return requested.every((version) => compareTuples(version, currentVersion.slice(0, version.length)) < 0);
    }
  }
  if (MIGRATION_INTENT.test(normalized)) return false;
  return new RegExp(`${B_START}(?:legacy|deprecated)${B_END}`, "u").test(normalized);
}

/** Choose one coherent applicability generation for stack retrieval. */
function stackRowFilter(rows: readonly Row[], query: string, stack: string): [RowFilter, string] {
  const statuses = new Set(rows.map((row) => row["Status"] ?? "unverified"));
  const hasLegacy = statuses.has("deprecated");
  const requestsLegacy = stackQueryRequestsLegacy(query, stack);
  let statusFilter: RowFilter;
  let variant: string;
  if (hasLegacy && requestsLegacy) {
    statusFilter = (row) => row["Status"] === "deprecated";
    variant = "legacy-only";
  } else if (requestsLegacy && stack in STACK_CURRENT_VERSIONS) {
    return [() => false, "legacy-unavailable"];
  } else if (statuses.has("active")) {
    statusFilter = (row) => row["Status"] === "active";
    variant = "current-only";
  } else {
    statusFilter = (row) => (row["Status"] ?? "unverified") !== "deprecated";
    variant = "non-legacy";
  }

  if (stack !== "shadcn" && stack !== "rg-ui") return [statusFilter, variant];
  const normalized = normalize(String(query ?? "").toLowerCase());
  let requestedBase: string;
  if (normalized.includes("base ui")) requestedBase = "base";
  else if (normalized.includes("react aria")) requestedBase = "aria";
  else if (normalized.includes("radix") || normalized.includes("aschild")) requestedBase = "radix";
  else return [statusFilter, variant];

  const matchesBase: RowFilter = (row) => {
    const match = /\bbase=([^;]+)/.exec((row["Applies To"] ?? "").toLowerCase());
    const bases = match ? match[1]!.split("|") : [];
    return statusFilter(row) && bases.includes(requestedBase);
  };
  // rg-ui is Base UI only: never filter its rows away for a base it does not tag.
  if (stack === "rg-ui" && !rows.some((row) => /\bbase=/.test((row["Applies To"] ?? "").toLowerCase()))) {
    return [statusFilter, variant];
  }
  return [matchesBase, `${variant};base=${requestedBase}`];
}

/** Resolve a standalone API identifier even when its BM25 IDF is low. */
function exactStackIdentifier(rows: readonly Row[], query: string, rowFilter: RowFilter): Row | null {
  const identifier = String(query ?? "").trim();
  if (pyLength(identifier) < 6 || /\s/.test(identifier)) return null;
  const pattern = new RegExp(`(?<![A-Za-z0-9_])${escapeRegExp(identifier)}(?![A-Za-z0-9_])`, "iu");
  const fields = ["Guideline", "Description", "Do", "Don't", "Code Good", "Code Bad"];
  const matches = rows.filter((row) => rowFilter(row) && fields.some((field) => pattern.test(row[field] ?? "")));
  return matches.length === 1 ? matches[0]! : null;
}

/** Prefer the explicit successor row for a brand-new app on legacy-only stacks. */
function legacySuccessorGuidance(rows: readonly Row[], query: string, stack: string, rowFilter: RowFilter): Row | null {
  const normalized = normalize(String(query ?? "").toLowerCase());
  if (!LEGACY_ONLY_STACKS.has(stack) || !new RegExp(`${B_START}(?:brand new|new)\\s+(?:app|application|project)${B_END}`, "u").test(normalized)) {
    return null;
  }
  const successor = new RegExp(`${B_START}(?:prefer|choose|use)${B_END}.*${B_START}new (?:apps?|projects?)${B_END}`, "u");
  const matches = rows.filter(
    (row) => rowFilter(row) && successor.test([row["Guideline"] ?? "", row["Description"] ?? "", row["Do"] ?? ""].join(" ").toLowerCase()),
  );
  return matches.length === 1 ? matches[0]! : null;
}

/** Search stack-specific guidelines. */
export async function searchStack(
  query: string,
  stack: string,
  maxResults: number = MAX_RESULTS,
  diagnostics = false,
): Promise<SearchResult> {
  if (!validMaxResults(maxResults)) return { error: "max_results must be an integer from 1 to 20", stack };
  if (!(stack in STACK_CONFIG)) return { error: `Unknown stack: ${stack}. Available: ${AVAILABLE_STACKS.join(", ")}` };
  const file = STACK_CONFIG[stack]!.file;
  const path = join(DATA_DIR, file);
  if (!(await Bun.file(path).exists())) return { error: `Stack file not found: ${path}`, stack };

  const rows = await loadRowsOrEmpty(path);
  const [rowFilter, cacheVariant] = stackRowFilter(rows, query, stack);
  const threshold = cacheVariant === "legacy-only" ? NO_THRESHOLD : STACK_THRESHOLD;
  const exact = legacySuccessorGuidance(rows, query, stack, rowFilter) ?? exactStackIdentifier(rows, query, rowFilter);
  let results: Row[];
  let bm25: BM25 | null = null;
  let diagnostic: Diagnostic;
  if (exact) {
    results = [project(exact, STACK_COLS.outputCols)];
    diagnostic = exactMatchDiagnostic(query, "exact-identifier");
  } else {
    ({ results, index: bm25, diagnostic } = await searchCsvDetailed(
      path,
      STACK_COLS.searchCols,
      STACK_COLS.outputCols,
      query,
      maxResults,
      threshold,
      null,
      rowFilter,
      cacheVariant,
    ));
  }
  const out: SearchResult = { domain: "stack", stack, query, file, count: results.length, results };
  if (diagnostic.error) out.error = diagnostic.error;
  if (results.length === 0) out.suggestions = suggestTerms(bm25, query, 6, threshold);
  if (diagnostics) out.diagnostics = diagnostic;
  return out;
}

/** Load the brand catalog (brands.csv) as rows; empty when absent. */
export async function loadBrands(): Promise<Row[]> {
  return loadRowsOrEmpty(join(DATA_DIR, CSV_CONFIG.brand.file));
}

/** Resolve a brand row by Brand ID, Brand or Theme (case-insensitive). */
export async function findBrand(reference: string): Promise<Row | null> {
  const rows = await loadBrands();
  return exactRowIdentity(rows, reference, BRAND_IDENTITY_FIELDS) ?? rows.find((row) => row["Brand ID"] === reference) ?? null;
}
