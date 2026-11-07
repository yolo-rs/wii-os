#!/usr/bin/env bun
/**
 * CIP Design Search CLI - Search corporate identity design guidelines.
 *
 * Examples:
 *   bun scripts/cip/search.ts "business card"
 *   bun scripts/cip/search.ts "luxury elegant" --domain style
 *   bun scripts/cip/search.ts "tech startup" --cip-brief -b "TechFlow"
 *   bun scripts/cip/search.ts "corporate professional" --all
 *   bun scripts/cip/search.ts "vehicle branding" --json
 */

import { parseArgs } from "node:util";
import type { Row } from "../../../design-system/scripts/lib/search-core.ts";
import { CSV_CONFIG, getCipBrief, search, searchAll, type CipBrief } from "./core.ts";

export function formatResults(results: Row[]): string {
  if (!results.length) return "No results found.";
  const out: string[] = [];
  results.forEach((item, i) => {
    out.push(`\n${"=".repeat(60)}`);
    out.push(`Result ${i + 1}:`);
    for (const [key, value] of Object.entries(item)) if (value) out.push(`  ${key}: ${value}`);
  });
  return out.join("\n");
}

export function formatBrief(brief: CipBrief): string {
  const na = (v: string | undefined) => v ?? "N/A";
  const out = [`\n${"=".repeat(60)}`, `CIP DESIGN BRIEF: ${brief.brand_name}`, "=".repeat(60)];
  if (Object.keys(brief.industry).length) {
    out.push(`\n📊 INDUSTRY: ${na(brief.industry.Industry)}`);
    out.push(`   Style: ${na(brief.industry["CIP Style"])}`);
    out.push(`   Mood: ${na(brief.industry.Mood)}`);
  }
  if (Object.keys(brief.style).length) {
    out.push(`\n🎨 DESIGN STYLE: ${na(brief.style["Style Name"])}`);
    out.push(`   Description: ${na(brief.style.Description)}`);
    out.push(`   Materials: ${na(brief.style.Materials)}`);
    out.push(`   Finishes: ${na(brief.style.Finishes)}`);
  }
  out.push(`\n🎯 COLOR SYSTEM:`);
  out.push(`   Primary: ${brief.color_system.primary}`);
  out.push(`   Secondary: ${brief.color_system.secondary}`);
  out.push(`\n✏️ TYPOGRAPHY: ${brief.typography}`);
  if (brief.recommended_deliverables.length) {
    out.push(`\n📦 RECOMMENDED DELIVERABLES:`);
    for (const d of brief.recommended_deliverables) {
      out.push(`   • ${na(d.Deliverable)}: ${(d.Description ?? "").slice(0, 60)}...`);
    }
  }
  return out.join("\n");
}

async function main(): Promise<void> {
  const { values, positionals } = parseArgs({
    args: process.argv.slice(2),
    allowPositionals: true,
    options: {
      domain: { type: "string", short: "d" },
      max: { type: "string", short: "m", default: "3" },
      all: { type: "boolean", short: "a" },
      "cip-brief": { type: "boolean", short: "c" },
      brand: { type: "string", short: "b", default: "BrandName" },
      style: { type: "string", short: "s" },
      json: { type: "boolean", short: "j" },
    },
  });
  const query = positionals[0];
  if (!query) {
    console.error("usage: search.ts [-d DOMAIN] [-m MAX] [-a] [-c] [-b BRAND] [-s STYLE] [-j] query");
    process.exit(2);
  }
  if (values.domain && !(values.domain in CSV_CONFIG)) {
    console.error(`error: argument --domain/-d: invalid choice: '${values.domain}' (choose from ${Object.keys(CSV_CONFIG).join(", ")})`);
    process.exit(2);
  }
  const max = Number.parseInt(String(values.max), 10);

  if (values["cip-brief"]) {
    const brief = await getCipBrief(String(values.brand), query, values.style);
    console.log(values.json ? JSON.stringify(brief, null, 2) : formatBrief(brief));
  } else if (values.all) {
    const results = await searchAll(query, max);
    if (values.json) {
      console.log(JSON.stringify(results, null, 2));
    } else {
      for (const [domain, items] of Object.entries(results)) {
        console.log(`\n${"#".repeat(60)}`);
        console.log(`# ${domain.toUpperCase()}`);
        console.log(formatResults(items));
      }
    }
  } else {
    const result = await search(query, values.domain, max);
    if (values.json) {
      console.log(JSON.stringify(result, null, 2));
    } else {
      console.log(`\nDomain: ${result.domain}`);
      console.log(`Query: ${result.query}`);
      console.log(`Results: ${result.count}`);
      console.log(formatResults(result.results ?? []));
    }
  }
}

if (import.meta.main) await main();
