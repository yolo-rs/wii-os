#!/usr/bin/env bun
/**
 * Logo Design Search - CLI for searching logo design guidelines
 * Usage: bun scripts/logo/search.ts "<query>" [--domain <domain>] [--max-results 3]
 *        bun scripts/logo/search.ts "<query>" --design-brief [-p "Brand Name"]
 *
 * Domains: style, color, industry
 */

import { parseArgs } from "node:util";
import type { SearchResult } from "../../../design-system/scripts/lib/search-core.ts";
import { CSV_CONFIG, MAX_RESULTS, search, searchAll } from "./core.ts";

/** Format results for agent consumption (token-optimized). */
export function formatOutput(result: SearchResult): string {
  if (result.error) return `Error: ${result.error}`;
  const out = ["## Logo Design Search Results"];
  out.push(`**Domain:** ${result.domain} | **Query:** ${result.query}`);
  out.push(`**Source:** ${result.file} | **Found:** ${result.count} results\n`);
  (result.results ?? []).forEach((row, i) => {
    out.push(`### Result ${i + 1}`);
    for (const [key, value] of Object.entries(row)) {
      out.push(`- **${key}:** ${value.length > 300 ? value.slice(0, 300) + "..." : value}`);
    }
    out.push("");
  });
  return out.join("\n");
}

/** Comprehensive logo design brief across style, color and industry domains. */
export async function generateDesignBrief(query: string, brandName?: string | null): Promise<string> {
  const results = await searchAll(query, 2);
  const na = (v: string | undefined) => v ?? "N/A";
  const out = ["=".repeat(60)];
  out.push(brandName ? `  LOGO DESIGN BRIEF: ${brandName.toUpperCase()}` : "  LOGO DESIGN BRIEF");
  out.push("=".repeat(60), `  Query: ${query}`, "=".repeat(60), "");

  if (results.industry) {
    out.push("## INDUSTRY ANALYSIS");
    for (const r of results.industry) {
      out.push(`**Industry:** ${na(r.Industry)}`);
      out.push(`- Recommended Styles: ${na(r["Recommended Styles"])}`);
      out.push(`- Colors: ${na(r["Primary Colors"])}`);
      out.push(`- Typography: ${na(r.Typography)}`);
      out.push(`- Symbols: ${na(r["Common Symbols"])}`);
      out.push(`- Mood: ${na(r.Mood)}`);
      out.push(`- Best Practices: ${na(r["Best Practices"])}`);
      out.push(`- Avoid: ${na(r.Avoid)}`);
      out.push("");
    }
  }
  if (results.style) {
    out.push("## STYLE RECOMMENDATIONS");
    for (const r of results.style) {
      out.push(`**${na(r["Style Name"])}** (${na(r.Category)})`);
      out.push(`- Colors: ${na(r["Primary Colors"])} | ${na(r["Secondary Colors"])}`);
      out.push(`- Typography: ${na(r.Typography)}`);
      out.push(`- Effects: ${na(r.Effects)}`);
      out.push(`- Best For: ${na(r["Best For"])}`);
      out.push(`- Complexity: ${na(r.Complexity)}`);
      out.push("");
    }
  }
  if (results.color) {
    out.push("## COLOR PALETTE OPTIONS");
    for (const r of results.color) {
      out.push(`**${na(r["Palette Name"])}**`);
      out.push(`- Primary: ${na(r["Primary Hex"])}`);
      out.push(`- Secondary: ${na(r["Secondary Hex"])}`);
      out.push(`- Accent: ${na(r["Accent Hex"])}`);
      out.push(`- Background: ${na(r["Background Hex"])}`);
      out.push(`- Psychology: ${na(r.Psychology)}`);
      out.push("");
    }
  }
  out.push("=".repeat(60));
  return out.join("\n");
}

async function main(): Promise<void> {
  const { values, positionals } = parseArgs({
    args: process.argv.slice(2),
    allowPositionals: true,
    options: {
      domain: { type: "string", short: "d" },
      "max-results": { type: "string", short: "n" },
      json: { type: "boolean" },
      "design-brief": { type: "boolean" },
      db: { type: "boolean" },
      "brand-name": { type: "string", short: "p" },
    },
  });
  const query = positionals[0];
  if (!query) {
    console.error("usage: search.ts [-d {style,color,industry}] [-n MAX_RESULTS] [--json] [--design-brief] [-p BRAND_NAME] query");
    process.exit(2);
  }
  if (values.domain && !(values.domain in CSV_CONFIG)) {
    console.error(`error: argument --domain/-d: invalid choice: '${values.domain}' (choose from ${Object.keys(CSV_CONFIG).join(", ")})`);
    process.exit(2);
  }
  if (values["design-brief"] || values.db) {
    console.log(await generateDesignBrief(query, values["brand-name"]));
    return;
  }
  const maxResults = values["max-results"] !== undefined ? Number.parseInt(values["max-results"], 10) : MAX_RESULTS;
  const result = await search(query, values.domain, maxResults);
  console.log(values.json ? JSON.stringify(result, null, 2) : formatOutput(result));
}

if (import.meta.main) await main();
