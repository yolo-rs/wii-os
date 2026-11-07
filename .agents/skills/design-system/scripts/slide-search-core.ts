/**
 * Slide Search Core - BM25 search engine for slide design databases.
 */

import { join } from "node:path";
import { createDomainSearch, loadCsv, type DomainConfig, type Row, type SearchResult } from "./lib/search-core.ts";

export const DATA_DIR = join(import.meta.dir, "..", "data");
export const MAX_RESULTS = 3;

export const CSV_CONFIG: Record<string, DomainConfig> = {
  strategy: {
    file: "slide-strategies.csv",
    search_cols: ["strategy_name", "keywords", "goal", "audience", "narrative_arc"],
    output_cols: ["strategy_name", "keywords", "slide_count", "structure", "goal", "audience", "tone", "narrative_arc", "sources"],
  },
  layout: {
    file: "slide-layouts.csv",
    search_cols: ["layout_name", "keywords", "use_case", "recommended_for"],
    output_cols: [
      "layout_name",
      "keywords",
      "use_case",
      "content_zones",
      "visual_weight",
      "cta_placement",
      "recommended_for",
      "avoid_for",
      "css_structure",
    ],
  },
  copy: {
    file: "slide-copy.csv",
    search_cols: ["formula_name", "keywords", "use_case", "emotion_trigger", "slide_type"],
    output_cols: ["formula_name", "keywords", "components", "use_case", "example_template", "emotion_trigger", "slide_type", "source"],
  },
  chart: {
    file: "slide-charts.csv",
    search_cols: ["chart_type", "keywords", "best_for", "when_to_use", "slide_context"],
    output_cols: [
      "chart_type",
      "keywords",
      "best_for",
      "data_type",
      "when_to_use",
      "when_to_avoid",
      "max_categories",
      "slide_context",
      "css_implementation",
      "accessibility_notes",
    ],
  },
};

export const AVAILABLE_DOMAINS = Object.keys(CSV_CONFIG);

const DOMAIN_KEYWORDS: Record<string, string[]> = {
  strategy: [
    "pitch", "deck", "investor", "yc", "seed", "series", "demo", "sales", "webinar",
    "conference", "board", "qbr", "all-hands", "duarte", "kawasaki", "structure",
  ],
  layout: [
    "slide", "layout", "grid", "column", "title", "hero", "section", "cta",
    "screenshot", "quote", "timeline", "comparison", "pricing", "team",
  ],
  copy: [
    "headline", "copy", "formula", "aida", "pas", "hook", "cta", "benefit",
    "objection", "proof", "testimonial", "urgency", "scarcity",
  ],
  chart: [
    "chart", "graph", "bar", "line", "pie", "funnel", "metrics", "data",
    "visualization", "kpi", "trend", "comparison", "heatmap", "gauge",
  ],
};

export const { detectDomain, search } = createDomainSearch({
  dataDir: DATA_DIR,
  config: CSV_CONFIG,
  domainKeywords: DOMAIN_KEYWORDS,
  fallbackDomain: "strategy",
});

/** Search across all domains, keeping only domains with results. */
export async function searchAll(query: string, maxResults = 2): Promise<Record<string, SearchResult>> {
  const all: Record<string, SearchResult> = {};
  for (const domain of AVAILABLE_DOMAINS) {
    const result = await search(query, domain, maxResults);
    if ((result.count ?? 0) > 0) all[domain] = result;
  }
  return all;
}

// ============ CONTEXTUAL SEARCH (Premium Slide System) ============

const DECISION_CSV_CONFIG: Record<string, { file: string; key_col: string }> = {
  "layout-logic": { file: "slide-layout-logic.csv", key_col: "goal" },
  typography: { file: "slide-typography.csv", key_col: "content_type" },
  "color-logic": { file: "slide-color-logic.csv", key_col: "emotion" },
  backgrounds: { file: "slide-backgrounds.csv", key_col: "slide_type" },
};

/** Load a decision CSV keyed by its primary column (later rows win, like a dict comprehension). */
async function loadDecisionCsv(csvType: string): Promise<Record<string, Row>> {
  const config = DECISION_CSV_CONFIG[csvType];
  if (!config) return {};
  const filepath = join(DATA_DIR, config.file);
  if (!(await Bun.file(filepath).exists())) return {};
  const out: Record<string, Row> = {};
  for (const row of await loadCsv(filepath)) {
    if (config.key_col in row) out[row[config.key_col]!] = row;
  }
  return out;
}

export async function getLayoutForGoal(goal: string, previousEmotion?: string | null): Promise<Record<string, unknown>> {
  const layouts = await loadDecisionCsv("layout-logic");
  const row = layouts[goal] ?? layouts["features"];
  const result: Record<string, unknown> = row ? { ...row } : {};
  if (result.break_pattern === "true" && previousEmotion) {
    result._pattern_break = true;
    result._contrast_with = previousEmotion;
  }
  return result;
}

const TYPOGRAPHY_TYPE_MAP: Record<string, string> = {
  hero: "hero-statement",
  hook: "hero-statement",
  title: "title-only",
  problem: "subtitle-heavy",
  agitation: "metric-callout",
  solution: "subtitle-heavy",
  features: "feature-grid",
  proof: "metric-callout",
  traction: "data-insight",
  social: "quote-block",
  testimonial: "testimonial",
  pricing: "pricing",
  team: "team",
  cta: "cta-action",
  comparison: "comparison",
  timeline: "timeline",
};

export async function getTypographyForSlide(slideType: string, hasMetrics = false, hasQuote = false): Promise<Row> {
  const typography = await loadDecisionCsv("typography");
  if (hasMetrics) return typography["metric-callout"] ?? {};
  if (hasQuote) return typography["quote-block"] ?? {};
  return typography[TYPOGRAPHY_TYPE_MAP[slideType] ?? "feature-grid"] ?? {};
}

export async function getColorForEmotion(emotion: string): Promise<Row> {
  const colors = await loadDecisionCsv("color-logic");
  return colors[emotion] ?? colors["clarity"] ?? {};
}

export async function getBackgroundConfig(slideType: string): Promise<Row> {
  const backgrounds = await loadDecisionCsv("backgrounds");
  return backgrounds[slideType] ?? {};
}

/**
 * Premium decks use 2-3 full-bleed slides strategically: never consecutive,
 * spread across the deck, reserved for high-emotion beats.
 */
export function shouldUseFullBleed(slideIndex: number, totalSlides: number, emotion: string): boolean {
  if (!["hope", "urgency", "fear", "curiosity"].includes(emotion)) return false;
  if (totalSlides < 3) return false;
  const third = Math.floor(totalSlides / 3);
  return [1, third, third * 2, totalSlides - 1].includes(slideIndex);
}

/** Pattern break for emotional contrast (Duarte Sparkline technique). */
export function calculatePatternBreak(slideIndex: number, totalSlides: number, previousEmotion?: string | null): boolean {
  if (totalSlides < 5) return false;
  const third = Math.floor(totalSlides / 3);
  if (slideIndex === third || slideIndex === third * 2) return true;
  return previousEmotion != null && ["frustration", "hope", "fear"].includes(previousEmotion);
}

const ANIMATION_MAP: Record<string, string> = {
  hook: "animate-fade-up",
  problem: "animate-fade-up",
  agitation: "animate-count animate-stagger",
  solution: "animate-scale",
  features: "animate-stagger",
  traction: "animate-chart animate-count",
  proof: "animate-stagger-scale",
  social: "animate-fade-up",
  cta: "animate-pulse",
};

/** Search enriched with deck-context recommendations. */
export async function searchWithContext(
  query: string,
  slidePosition = 1,
  totalSlides = 9,
  previousEmotion: string | null = null,
) {
  const baseResults = await searchAll(query, 2);
  const q = query.toLowerCase();

  let goal = detectDomain(q);
  if (q.includes("problem")) goal = "problem";
  else if (q.includes("solution")) goal = "solution";
  else if (q.includes("cta") || q.includes("call to action")) goal = "cta";
  else if (q.includes("hook") || q.includes("title")) goal = "hook";
  else if (q.includes("traction") || q.includes("metric")) goal = "traction";

  const context: Record<string, any> = {
    slide_position: slidePosition,
    total_slides: totalSlides,
    previous_emotion: previousEmotion,
    inferred_goal: goal,
  };

  const layout = await getLayoutForGoal(goal, previousEmotion);
  const hasLayout = Object.keys(layout).length > 0;
  if (hasLayout) {
    context.recommended_layout = layout.layout_pattern;
    context.layout_direction = layout.direction;
    context.visual_weight = layout.visual_weight;
    context.use_background_image = layout.use_bg_image === "true";
  }

  const typography = await getTypographyForSlide(goal);
  if (Object.keys(typography).length > 0) {
    context.typography = {
      primary_size: typography.primary_size,
      secondary_size: typography.secondary_size,
      weight_contrast: typography.weight_contrast,
    };
  }

  const emotion = hasLayout ? String(layout.emotion ?? "clarity") : "clarity";
  const color = await getColorForEmotion(emotion);
  if (Object.keys(color).length > 0) {
    context.color_treatment = {
      background: color.background,
      text_color: color.text_color,
      accent_usage: color.accent_usage,
      card_style: color.card_style,
    };
  }

  context.should_break_pattern = calculatePatternBreak(slidePosition, totalSlides, previousEmotion);
  context.should_use_full_bleed = shouldUseFullBleed(slidePosition, totalSlides, emotion);

  if (context.use_background_image) {
    const bg = await getBackgroundConfig(goal);
    if (Object.keys(bg).length > 0) {
      context.background = {
        image_category: bg.image_category,
        overlay_style: bg.overlay_style,
        search_keywords: bg.search_keywords,
      };
    }
  }

  context.animation_class = ANIMATION_MAP[goal] ?? "animate-fade-up";
  return { query, context, base_results: baseResults };
}
