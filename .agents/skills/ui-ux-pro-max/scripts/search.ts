#!/usr/bin/env bun
/**
 * UI/UX Pro Max search: calibrated BM25 search over the local design catalogs.
 *
 *   bun scripts/search.ts "<query>" [--domain <domain>] [--stack <stack>] [-n 3] [--json] [--full]
 *   bun scripts/search.ts "<query>" --design-system [-p "Project Name"] [-f ascii|markdown]
 *   bun scripts/search.ts "<query>" --design-system --brand rose-griffon-roy
 *   bun scripts/search.ts "<query>" --design-system --persist -p "Project" --output-dir "<project-root>" [--page dashboard] [--force]
 *   bun scripts/search.ts "<query>" --design-system --variance 8 --motion 9 --density 7
 *
 * Domains: style, color, chart, landing, product, ux, typography, google-fonts, icons, gsap, react, web, brand
 * Stacks: see --help (includes m3 and rg-ui for the Bun OS / Aphrody workspace)
 */

import { parseArgs } from "node:util";
import { AVAILABLE_STACKS, CSV_CONFIG, MAX_RESULTS, UNTRUNCATED_COLS, search, searchStack, type SearchResult } from "./core.ts";
import { generateDesignSystem } from "./design-system.ts";

const TRUNCATE_AT = 300;

/** Format results for agent consumption (token-optimized). */
export function formatOutput(result: SearchResult, full = false): string {
  if (result.error) return `Error: ${result.error}`;
  const output: string[] = [];
  if (result.stack) {
    output.push("## UI Pro Max Stack Guidelines");
    output.push(`**Stack:** ${result.stack} | **Query:** ${result.query}`);
  } else {
    output.push("## UI Pro Max Search Results");
    let domainNote = String(result.domain);
    if (result.auto_detected) {
      domainNote += " (auto-detected";
      if (result.runner_up_domain) domainNote += `, runner-up: ${result.runner_up_domain}`;
      domainNote += ")";
    }
    output.push(`**Domain:** ${domainNote} | **Query:** ${result.query}`);
  }
  output.push(`**Source:** ${result.file} | **Found:** ${result.count} results\n`);

  if (result.count === 0) {
    if (result.redirect) {
      output.push(
        "This legacy style label is now modeled in the " +
          `\`${result.redirect.domain}\` domain as \`${result.redirect.id}\`. ` +
          "Search that domain instead of treating a page composition as a visual style.",
      );
      return output.join("\n");
    }
    output.push(
      "No matches. This is not a match with an empty value -- the query " +
        "did not hit the database. Retry with broader/different keywords " +
        "before falling back to general defaults, and say explicitly that " +
        "no database match was found if you do fall back.",
    );
    if (result.suggestions?.length) output.push(`**Closest known terms:** ${result.suggestions.join(", ")}`);
    return output.join("\n");
  }

  (result.results ?? []).forEach((row, index) => {
    output.push(`### Result ${index + 1}`);
    for (const [key, value] of Object.entries(row)) {
      let text = String(value);
      if (!full && !UNTRUNCATED_COLS.has(key) && Array.from(text).length > TRUNCATE_AT) {
        text = Array.from(text).slice(0, TRUNCATE_AT).join("") + "...";
      }
      output.push(`- **${key}:** ${text}`);
    }
    output.push("");
  });
  return output.join("\n");
}

const USAGE = `usage: search.ts [-h] [--domain DOMAIN] [--stack STACK] [--max-results 1-20] [--json] [--full]
                 [--design-system] [--project-name PROJECT_NAME] [--format {ascii,markdown}]
                 [--brand BRAND] [--persist] [--page PAGE] [--output-dir OUTPUT_DIR] [--force]
                 [--variance 1-10] [--motion 1-10] [--density 1-10]
                 query`;

const HELP = `${USAGE}

UI Pro Max Search (Bun)

positional arguments:
  query                 Search query

options:
  -h, --help            show this help message and exit
  -d, --domain DOMAIN   Search domain: ${Object.keys(CSV_CONFIG).join(", ")}
  -s, --stack STACK     Stack-specific search. Available: ${AVAILABLE_STACKS.join(", ")}
  -n, --max-results N   Max results, 1-20 (default: ${MAX_RESULTS})
  --json                Output as JSON
  --full                Do not truncate long field values in text output
  -ds, --design-system  Generate complete design system recommendation
  -p, --project-name    Project name for design system output
  -f, --format          Output format for design system: ascii (default) or markdown (ignored with --json)
  -b, --brand BRAND     Use a real brand theme from brands.csv (e.g. rose-griffon-roy, azalee-light)
  --persist             Save to design-system/<project-slug>/MASTER.md
  --page PAGE           Also create design-system/<project-slug>/pages/<page>.md
  -o, --output-dir DIR  Directory the design-system/ folder is created under (pass the project root)
  --force               Overwrite an existing MASTER.md when persisting
  --variance 1-10       DESIGN_VARIANCE dial: 1=centered/minimal, 10=bold/asymmetric
  --motion 1-10         MOTION_INTENSITY dial: 1=subtle, 10=complex (attaches a GSAP snippet)
  --density 1-10        VISUAL_DENSITY dial: 1=spacious, 10=dense/dashboard (overrides spacing)`;

function fail(message: string): never {
  process.stderr.write(`${USAGE}\nsearch.ts: error: ${message}\n`);
  process.exit(2);
}

function intInRange(name: string, value: string | undefined, lo: number, hi: number): number | null {
  if (value === undefined) return null;
  if (!/^[+-]?\d+$/.test(value.trim())) fail(`argument ${name}: invalid int value: '${value}'`);
  const parsed = Number.parseInt(value, 10);
  if (parsed < lo || parsed > hi) fail(`argument ${name}: invalid choice: ${parsed} (choose from ${lo}-${hi})`);
  return parsed;
}

function choice(name: string, value: string | undefined, choices: readonly string[]): string | null {
  if (value === undefined) return null;
  if (!choices.includes(value)) fail(`argument ${name}: invalid choice: '${value}' (choose from ${choices.map((c) => `'${c}'`).join(", ")})`);
  return value;
}

export async function main(argv: string[]): Promise<number> {
  const args = argv.map((arg) => (arg === "-ds" ? "--design-system" : arg));
  let parsed;
  try {
    parsed = parseArgs({
      args,
      allowPositionals: true,
      strict: true,
      options: {
        help: { type: "boolean", short: "h" },
        domain: { type: "string", short: "d" },
        stack: { type: "string", short: "s" },
        "max-results": { type: "string", short: "n" },
        json: { type: "boolean" },
        full: { type: "boolean" },
        "design-system": { type: "boolean" },
        "project-name": { type: "string", short: "p" },
        format: { type: "string", short: "f" },
        brand: { type: "string", short: "b" },
        persist: { type: "boolean" },
        page: { type: "string" },
        "output-dir": { type: "string", short: "o" },
        force: { type: "boolean" },
        variance: { type: "string" },
        motion: { type: "string" },
        density: { type: "string" },
      },
    });
  } catch (error) {
    fail((error as Error).message);
  }
  const { values, positionals } = parsed;
  if (values.help) {
    console.log(HELP);
    return 0;
  }
  if (positionals.length === 0) fail("the following arguments are required: query");
  if (positionals.length > 1) fail(`unrecognized arguments: ${positionals.slice(1).join(" ")}`);
  const query = positionals[0]!;
  const domain = choice("--domain/-d", values.domain, Object.keys(CSV_CONFIG));
  const stack = choice("--stack/-s", values.stack, AVAILABLE_STACKS);
  const maxResults = intInRange("--max-results/-n", values["max-results"], 1, 20) ?? MAX_RESULTS;
  const format = (choice("--format/-f", values.format, ["ascii", "markdown"]) ?? "ascii") as "ascii" | "markdown";
  const variance = intInRange("--variance", values.variance, 1, 10);
  const motion = intInRange("--motion", values.motion, 1, 10);
  const density = intInRange("--density", values.density, 1, 10);

  if (values["design-system"]) {
    if (stack) {
      console.error(`note: --stack ${stack} is ignored in --design-system mode; run a separate --stack query for stack-specific guidelines`);
    }
    let result;
    try {
      result = await generateDesignSystem(query, {
        projectName: values["project-name"] ?? null,
        format,
        persist: values.persist ?? false,
        page: values.page ?? null,
        outputDir: values["output-dir"] ?? null,
        variance,
        motion,
        density,
        force: values.force ?? false,
        brand: values.brand ?? null,
      });
    } catch (error) {
      console.error(`Error: ${(error as Error).message}`);
      return 1;
    }
    if (values.json) {
      console.log(JSON.stringify({ design_system: result.design_system, persistence: result.persistence }, null, 2));
      return 0;
    }
    console.log(result.text);
    if (values.persist) {
      const persistence = result.persistence;
      console.log("\n" + "=".repeat(60));
      if (persistence?.status === "skipped_exists") {
        console.log(`⚠️  ${persistence.message ?? "MASTER.md already exists; not overwritten."}`);
      } else {
        const dir = persistence?.design_system_dir ?? "design-system/<project>";
        console.log(`✅ Design system persisted to ${dir}/`);
        for (const file of persistence?.created_files ?? []) console.log(`   📄 ${file}`);
        console.log("");
        console.log(`📖 Usage: When building a page, check ${dir}/pages/[page].md first.`);
        console.log("   If it exists, its rules override MASTER.md. Otherwise, use MASTER.md.");
      }
      console.log("=".repeat(60));
    }
    return 0;
  }

  const result = stack ? await searchStack(query, stack, maxResults) : await search(query, domain, maxResults);
  console.log(values.json ? JSON.stringify(result, null, 2) : formatOutput(result, values.full ?? false));
  return 0;
}

if (import.meta.main) {
  process.exitCode = await main(Bun.argv.slice(2));
}
