/**
 * CIP Design Core - BM25 search engine for Corporate Identity Program design guidelines.
 */

import { join } from "node:path";
import { createDomainSearch, type DomainConfig, type Row } from "../../../design-system/scripts/lib/search-core.ts";

export const DATA_DIR = join(import.meta.dir, "..", "..", "data", "cip");
export const MAX_RESULTS = 3;

export const CSV_CONFIG: Record<string, DomainConfig> = {
  deliverable: {
    file: "deliverables.csv",
    search_cols: ["Deliverable", "Category", "Keywords", "Description", "Mockup Context"],
    output_cols: ["Deliverable", "Category", "Keywords", "Description", "Dimensions", "File Format", "Logo Placement", "Color Usage", "Typography Notes", "Mockup Context", "Best Practices", "Avoid"],
  },
  style: {
    file: "styles.csv",
    search_cols: ["Style Name", "Category", "Keywords", "Description", "Mood"],
    output_cols: ["Style Name", "Category", "Keywords", "Description", "Primary Colors", "Secondary Colors", "Typography", "Materials", "Finishes", "Mood", "Best For", "Avoid For"],
  },
  industry: {
    file: "industries.csv",
    search_cols: ["Industry", "Keywords", "CIP Style", "Mood"],
    output_cols: ["Industry", "Keywords", "CIP Style", "Primary Colors", "Secondary Colors", "Typography", "Key Deliverables", "Mood", "Best Practices", "Avoid"],
  },
  mockup: {
    file: "mockup-contexts.csv",
    search_cols: ["Context Name", "Category", "Keywords", "Scene Description"],
    output_cols: ["Context Name", "Category", "Keywords", "Scene Description", "Lighting", "Environment", "Props", "Camera Angle", "Background", "Style Notes", "Best For", "Prompt Modifiers"],
  },
};

export const { detectDomain, search } = createDomainSearch({
  dataDir: DATA_DIR,
  config: CSV_CONFIG,
  domainKeywords: {
    deliverable: ["card", "letterhead", "envelope", "folder", "shirt", "cap", "badge", "signage", "vehicle", "car", "van", "stationery", "uniform", "merchandise", "packaging", "banner", "booth"],
    style: ["style", "minimal", "modern", "luxury", "vintage", "industrial", "elegant", "bold", "corporate", "organic", "playful"],
    industry: ["tech", "finance", "legal", "healthcare", "hospitality", "food", "fashion", "retail", "construction", "logistics"],
    mockup: ["mockup", "scene", "context", "photo", "shot", "lighting", "background", "studio", "lifestyle"],
  },
  fallbackDomain: "deliverable",
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

const first = async (query: string, domain: string): Promise<Row> => (await search(query, domain, 1)).results?.[0] ?? {};

export interface CipBrief {
  brand_name: string;
  industry: Row;
  style: Row;
  recommended_deliverables: Row[];
  color_system: { primary: string; secondary: string };
  typography: string;
  materials: string;
  finishes: string;
}

/** Generate a comprehensive CIP brief for a brand. */
export async function getCipBrief(brandName: string, industryQuery: string, styleQuery?: string | null): Promise<CipBrief> {
  const industry = await first(industryQuery, "industry");
  const style = await first(styleQuery || (industry["CIP Style"] ?? "corporate minimal"), "style");

  const keyDeliverables = (industry["Key Deliverables"] ?? "").split(/\s+/).filter(Boolean);
  const recommended: Row[] = [];
  for (const d of keyDeliverables.slice(0, 5)) {
    const result = await search(d, "deliverable", 1);
    if (result.results?.length) recommended.push(result.results[0]!);
  }

  return {
    brand_name: brandName,
    industry,
    style,
    recommended_deliverables: recommended,
    color_system: {
      primary: style["Primary Colors"] ?? industry["Primary Colors"] ?? "",
      secondary: style["Secondary Colors"] ?? industry["Secondary Colors"] ?? "",
    },
    typography: style.Typography ?? industry.Typography ?? "",
    materials: style.Materials ?? "",
    finishes: style.Finishes ?? "",
  };
}
