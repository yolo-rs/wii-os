/**
 * Logo Design Core - BM25 search engine for logo design guidelines.
 */

import { join } from "node:path";
import { createDomainSearch, type DomainConfig, type Row } from "../../../design-system/scripts/lib/search-core.ts";

export const DATA_DIR = join(import.meta.dir, "..", "..", "data", "logo");
export const MAX_RESULTS = 3;

export const CSV_CONFIG: Record<string, DomainConfig> = {
  style: {
    file: "styles.csv",
    search_cols: ["Style Name", "Category", "Keywords", "Best For"],
    output_cols: ["Style Name", "Category", "Keywords", "Primary Colors", "Secondary Colors", "Typography", "Effects", "Best For", "Avoid For", "Complexity", "Era"],
  },
  color: {
    file: "colors.csv",
    search_cols: ["Palette Name", "Category", "Keywords", "Psychology", "Best For"],
    output_cols: ["Palette Name", "Category", "Keywords", "Primary Hex", "Secondary Hex", "Accent Hex", "Background Hex", "Text Hex", "Psychology", "Best For", "Avoid For"],
  },
  industry: {
    file: "industries.csv",
    search_cols: ["Industry", "Keywords", "Recommended Styles", "Mood"],
    output_cols: ["Industry", "Keywords", "Recommended Styles", "Primary Colors", "Typography", "Common Symbols", "Mood", "Best Practices", "Avoid"],
  },
};

export const { detectDomain, search } = createDomainSearch({
  dataDir: DATA_DIR,
  config: CSV_CONFIG,
  domainKeywords: {
    style: ["style", "minimalist", "vintage", "modern", "retro", "geometric", "abstract", "emblem", "badge", "wordmark", "mascot", "luxury", "playful", "corporate"],
    color: ["color", "palette", "hex", "#", "rgb", "blue", "red", "green", "gold", "warm", "cool", "vibrant", "pastel"],
    industry: ["tech", "healthcare", "finance", "legal", "restaurant", "food", "fashion", "beauty", "education", "sports", "fitness", "real estate", "crypto", "gaming"],
  },
  fallbackDomain: "style",
});

/** Search across all domains and combine non-empty results. */
export async function searchAll(query: string, maxResults = 2): Promise<Record<string, Row[]>> {
  const all: Record<string, Row[]> = {};
  for (const domain of Object.keys(CSV_CONFIG)) {
    const result = await search(query, domain, maxResults);
    if (result.results?.length) all[domain] = result.results;
  }
  return all;
}
