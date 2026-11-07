#!/usr/bin/env bun
/**
 * Slide Search CLI - Search slide design databases for strategies, layouts, copy, and charts.
 *
 * Examples:
 *   bun scripts/search-slides.ts "investor pitch"           # Auto-detect domain (strategy)
 *   bun scripts/search-slides.ts "funnel conversion" -d chart
 *   bun scripts/search-slides.ts "headline hook" -d copy
 *   bun scripts/search-slides.ts "two column" -d layout
 *   bun scripts/search-slides.ts "startup funding" --all    # Search all domains
 *   bun scripts/search-slides.ts "metrics dashboard" --json # JSON output
 *
 * Contextual Search (Premium System):
 *   bun scripts/search-slides.ts "problem slide" --context --position 2 --total 9
 *   bun scripts/search-slides.ts "cta" --context --position 9 --total 9 --prev-emotion frustration
 */

import { parseArgs } from "node:util";
import type { Row } from "./lib/search-core.ts";
import { AVAILABLE_DOMAINS, search, searchAll, searchWithContext } from "./slide-search-core.ts";

const FIELDS: Record<string, Array<[string, string]>> = {
  strategy: [
    ["Slides", "slide_count"],
    ["Structure", "structure"],
    ["Goal", "goal"],
    ["Audience", "audience"],
    ["Tone", "tone"],
    ["Arc", "narrative_arc"],
    ["Source", "sources"],
  ],
  layout: [
    ["Use case", "use_case"],
    ["Zones", "content_zones"],
    ["Visual weight", "visual_weight"],
    ["CTA", "cta_placement"],
    ["Recommended", "recommended_for"],
    ["Avoid", "avoid_for"],
    ["CSS", "css_structure"],
  ],
  copy: [
    ["Components", "components"],
    ["Use case", "use_case"],
    ["Template", "example_template"],
    ["Emotion", "emotion_trigger"],
    ["Slide type", "slide_type"],
    ["Source", "source"],
  ],
  chart: [
    ["Best for", "best_for"],
    ["Data type", "data_type"],
    ["When to use", "when_to_use"],
    ["When to avoid", "when_to_avoid"],
    ["Max categories", "max_categories"],
    ["Slide context", "slide_context"],
    ["CSS", "css_implementation"],
    ["Accessibility", "accessibility_notes"],
  ],
};

const TITLE_FIELD: Record<string, string> = {
  strategy: "strategy_name",
  layout: "layout_name",
  copy: "formula_name",
  chart: "chart_type",
};

export function formatResult(result: Row, domain: string): string {
  const title = TITLE_FIELD[domain];
  if (!title) return "";
  const lines = [`**${result[title] ?? "N/A"}**`];
  for (const [label, key] of FIELDS[domain] ?? []) lines.push(`  ${label}: ${result[key] ?? "N/A"}`);
  return lines.join("\n");
}

export function formatContext(context: Record<string, any>): string {
  const out: string[] = [];
  out.push(`\n=== CONTEXTUAL RECOMMENDATIONS ===`);
  out.push(`Inferred Goal: ${context.inferred_goal ?? "N/A"}`);
  out.push(`Position: Slide ${context.slide_position} of ${context.total_slides}`);

  if (context.recommended_layout) {
    out.push(`\n📐 Layout: ${context.recommended_layout}`);
    out.push(`   Direction: ${context.layout_direction ?? "N/A"}`);
    out.push(`   Visual Weight: ${context.visual_weight ?? "N/A"}`);
  }
  if (context.typography) {
    const typo = context.typography;
    out.push(`\n📝 Typography:`);
    out.push(`   Primary: ${typo.primary_size ?? "N/A"}`);
    out.push(`   Secondary: ${typo.secondary_size ?? "N/A"}`);
    out.push(`   Contrast: ${typo.weight_contrast ?? "N/A"}`);
  }
  if (context.color_treatment) {
    const color = context.color_treatment;
    out.push(`\n🎨 Color Treatment:`);
    out.push(`   Background: ${color.background ?? "N/A"}`);
    out.push(`   Text: ${color.text_color ?? "N/A"}`);
    out.push(`   Accent: ${color.accent_usage ?? "N/A"}`);
  }
  if (context.should_break_pattern) out.push(`\n⚡ Pattern Break: YES (use contrasting layout)`);
  if (context.should_use_full_bleed) out.push(`\n🖼️ Full Bleed: Recommended for emotional impact`);
  if (context.use_background_image && context.background) {
    const bg = context.background;
    out.push(`\n📸 Background Image:`);
    out.push(`   Category: ${bg.image_category ?? "N/A"}`);
    out.push(`   Overlay: ${bg.overlay_style ?? "N/A"}`);
    out.push(`   Keywords: ${bg.search_keywords ?? "N/A"}`);
  }
  out.push(`\n✨ Animation: ${context.animation_class ?? "animate-fade-up"}`);
  return out.join("\n");
}

function intOption(value: unknown, fallback: number, name: string): number {
  if (value === undefined) return fallback;
  const parsed = Number(value);
  if (!Number.isInteger(parsed)) {
    console.error(`error: argument ${name}: invalid int value: '${value}'`);
    process.exit(2);
  }
  return parsed;
}

async function main(): Promise<void> {
  const { values, positionals } = parseArgs({
    args: process.argv.slice(2),
    allowPositionals: true,
    options: {
      domain: { type: "string", short: "d" },
      "max-results": { type: "string", short: "n" },
      all: { type: "boolean" },
      json: { type: "boolean" },
      context: { type: "boolean" },
      position: { type: "string" },
      total: { type: "string" },
      "prev-emotion": { type: "string" },
    },
  });

  const query = positionals[0];
  if (!query) {
    console.error("usage: search-slides.ts [-h] [-d {strategy,layout,copy,chart}] [-n MAX_RESULTS] [--all] [--json] [--context] query");
    process.exit(2);
  }
  if (values.domain && !AVAILABLE_DOMAINS.includes(values.domain)) {
    console.error(`error: argument -d/--domain: invalid choice: '${values.domain}' (choose from ${AVAILABLE_DOMAINS.join(", ")})`);
    process.exit(2);
  }
  const maxResults = intOption(values["max-results"], 3, "-n/--max-results");

  if (values.context) {
    const result = await searchWithContext(
      query,
      intOption(values.position, 1, "--position"),
      intOption(values.total, 9, "--total"),
      values["prev-emotion"] ?? null,
    );
    if (values.json) {
      console.log(JSON.stringify(result, null, 2));
      return;
    }
    console.log(formatContext(result.context));
    if (Object.keys(result.base_results).length > 0) {
      console.log("\n\n=== RELATED SEARCH RESULTS ===");
      for (const [domain, data] of Object.entries(result.base_results)) {
        console.log(`\n--- ${domain.toUpperCase()} ---`);
        for (const item of data.results ?? []) {
          console.log(formatResult(item, domain));
          console.log();
        }
      }
    }
    return;
  }

  if (values.all) {
    const results = await searchAll(query, maxResults);
    if (values.json) {
      console.log(JSON.stringify(results, null, 2));
      return;
    }
    if (Object.keys(results).length === 0) {
      console.log(`No results found for: ${query}`);
      return;
    }
    for (const [domain, data] of Object.entries(results)) {
      console.log(`\n=== ${domain.toUpperCase()} ===`);
      console.log(`File: ${data.file}`);
      console.log(`Results: ${data.count}`);
      console.log();
      for (const item of data.results ?? []) {
        console.log(formatResult(item, domain));
        console.log();
      }
    }
    return;
  }

  const result = await search(query, values.domain, maxResults);
  if (values.json) {
    console.log(JSON.stringify(result, null, 2));
    return;
  }
  if (result.error) {
    console.log(`Error: ${result.error}`);
    return;
  }
  console.log(`Domain: ${result.domain}`);
  console.log(`Query: ${result.query}`);
  console.log(`File: ${result.file}`);
  console.log(`Results: ${result.count}`);
  console.log();
  if (result.count === 0) {
    console.log("No matching results found.");
    return;
  }
  (result.results ?? []).forEach((item, i) => {
    console.log(`--- Result ${i + 1} ---`);
    console.log(formatResult(item, result.domain));
    console.log();
  });
}

if (import.meta.main) await main();
