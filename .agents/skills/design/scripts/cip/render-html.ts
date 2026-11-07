#!/usr/bin/env bun
/**
 * CIP HTML Presentation Renderer
 *
 * Generates a professional HTML presentation from CIP mockup images
 * (PNG, JPEG or WebP, as returned by the generator) with descriptions,
 * concepts and brand guidelines.
 *
 * Examples:
 *   bun scripts/cip/render-html.ts --brand "TopGroup" --industry "consulting" --images ./topgroup-cip
 *   bun scripts/cip/render-html.ts --brand "TopGroup" --industry "consulting" --images ./cip --output presentation.html
 */

import { stat } from "node:fs/promises";
import { basename, extname, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { getCipBrief } from "./core.ts";

interface DeliverableInfo {
  title: string;
  concept: string;
  purpose: string;
  specs: string;
}

export const DELIVERABLE_INFO: Record<string, DeliverableInfo> = {
  "business card": {
    title: "Business Card",
    concept: "First impression touchpoint for professional networking",
    purpose: "Creates memorable brand recall during business exchanges",
    specs: "Standard 3.5 x 2 inches, premium paper stock",
  },
  letterhead: {
    title: "Letterhead",
    concept: "Official correspondence identity",
    purpose: "Establishes credibility and professionalism in written communications",
    specs: "A4/Letter size, digital and print versions",
  },
  "document template": {
    title: "Document Template",
    concept: "Branded document system for internal and external use",
    purpose: "Ensures consistent brand representation across all documents",
    specs: "Multiple formats: Word, PDF, Google Docs compatible",
  },
  "reception signage": {
    title: "Reception Signage",
    concept: "Brand presence in physical office environment",
    purpose: "Creates strong first impression for visitors and reinforces brand identity",
    specs: "3D dimensional letters, backlit LED options, premium materials",
  },
  "office signage": {
    title: "Office Signage",
    concept: "Wayfinding and brand presence system",
    purpose: "Guides visitors while maintaining consistent brand experience",
    specs: "Modular system with directional and informational signs",
  },
  "polo shirt": {
    title: "Polo Shirt",
    concept: "Professional team apparel",
    purpose: "Creates unified team identity and brand ambassadorship",
    specs: "Premium pique cotton, embroidered logo on left chest",
  },
  "t-shirt": {
    title: "T-Shirt",
    concept: "Casual brand apparel",
    purpose: "Extends brand reach through everyday wear and promotional events",
    specs: "High-quality cotton, screen print or embroidery options",
  },
  vehicle: {
    title: "Vehicle Branding",
    concept: "Mobile brand advertising",
    purpose: "Transforms fleet into moving billboards for maximum visibility",
    specs: "Partial or full wrap, vinyl graphics, weather-resistant",
  },
  van: {
    title: "Van Branding",
    concept: "Commercial vehicle identity",
    purpose: "Professional fleet presence for service and delivery operations",
    specs: "Full wrap design, high-visibility contact information",
  },
  car: {
    title: "Car Branding",
    concept: "Executive vehicle identity",
    purpose: "Professional presence for corporate and sales teams",
    specs: "Subtle branding, door panels and rear window",
  },
  envelope: {
    title: "Envelope",
    concept: "Branded mail correspondence",
    purpose: "Extends brand identity to all outgoing mail",
    specs: "DL, C4, C5 sizes with logo placement",
  },
  folder: {
    title: "Presentation Folder",
    concept: "Document organization with brand identity",
    purpose: "Professional presentation of proposals and materials",
    specs: "A4/Letter pocket folder with die-cut design",
  },
};

const IMAGE_MIME: Record<string, string> = { ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp" };

const escapeHtml = (value: unknown) =>
  String(value).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#x27;");

/** Python str.title(). */
const titleCase = (text: string) => text.toLowerCase().replace(/(^|[^\p{L}])(\p{L})/gu, (_, sep: string, ch: string) => sep + ch.toUpperCase());

export function getDeliverableInfo(filename: string): DeliverableInfo {
  const lower = filename.toLowerCase();
  for (const [key, info] of Object.entries(DELIVERABLE_INFO)) {
    if (lower.includes(key.replaceAll(" ", "-")) || lower.includes(key.replaceAll(" ", "_"))) return info;
  }
  return {
    title: titleCase(filename.replaceAll("-", " ").replaceAll("_", " ")),
    concept: "Brand identity application",
    purpose: "Extends brand presence across touchpoints",
    specs: "Custom specifications",
  };
}

const CSS = `
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            background: #0a0a0a;
            color: #ffffff;
            line-height: 1.6;
        }
        .hero {
            min-height: 100vh;
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            text-align: center;
            padding: 4rem 2rem;
            background: linear-gradient(135deg, #1a1a2e 0%, #0a0a0a 100%);
        }
        .hero h1 {
            font-size: 4rem;
            font-weight: 700;
            letter-spacing: -0.02em;
            margin-bottom: 1rem;
            background: linear-gradient(135deg, #ffffff 0%, #888888 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }
        .hero .subtitle {
            font-size: 1.5rem;
            color: #888;
            margin-bottom: 3rem;
        }
        .hero .meta {
            display: flex;
            gap: 3rem;
            flex-wrap: wrap;
            justify-content: center;
        }
        .hero .meta-item {
            text-align: center;
        }
        .hero .meta-label {
            font-size: 0.75rem;
            text-transform: uppercase;
            letter-spacing: 0.1em;
            color: #666;
            margin-bottom: 0.5rem;
        }
        .hero .meta-value {
            font-size: 1rem;
            color: #ccc;
        }
        .section {
            padding: 6rem 2rem;
            max-width: 1400px;
            margin: 0 auto;
        }
        .section-title {
            font-size: 2.5rem;
            font-weight: 600;
            margin-bottom: 1rem;
            color: #fff;
        }
        .section-subtitle {
            font-size: 1.1rem;
            color: #888;
            margin-bottom: 4rem;
            max-width: 600px;
        }
        .deliverable {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 4rem;
            margin-bottom: 8rem;
            align-items: center;
        }
        .deliverable:nth-child(even) {
            direction: rtl;
        }
        .deliverable:nth-child(even) > * {
            direction: ltr;
        }
        .deliverable-image {
            position: relative;
            border-radius: 16px;
            overflow: hidden;
            box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
        }
        .deliverable-image img {
            width: 100%;
            height: auto;
            display: block;
        }
        .deliverable-content {
            padding: 2rem 0;
        }
        .deliverable-title {
            font-size: 2rem;
            font-weight: 600;
            margin-bottom: 1rem;
            color: #fff;
        }
        .deliverable-concept {
            font-size: 1.1rem;
            color: #aaa;
            margin-bottom: 1.5rem;
            font-style: italic;
        }
        .deliverable-purpose {
            font-size: 1rem;
            color: #888;
            margin-bottom: 1.5rem;
            line-height: 1.8;
        }
        .deliverable-specs {
            display: inline-block;
            padding: 0.5rem 1rem;
            background: rgba(255, 255, 255, 0.05);
            border-radius: 8px;
            font-size: 0.85rem;
            color: #666;
        }
        .color-palette {
            display: flex;
            gap: 1rem;
            margin-top: 2rem;
        }
        .color-swatch {
            width: 60px;
            height: 60px;
            border-radius: 12px;
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }
        .footer {
            text-align: center;
            padding: 4rem 2rem;
            border-top: 1px solid #222;
            color: #666;
        }
        .footer p {
            margin-bottom: 0.5rem;
        }
        @media (max-width: 900px) {
            .hero h1 {
                font-size: 2.5rem;
            }
            .deliverable {
                grid-template-columns: 1fr;
                gap: 2rem;
            }
            .deliverable:nth-child(even) {
                direction: ltr;
            }
        }
    `;

/** Generate the HTML presentation; returns the written path or null. */
export async function generateHtml(options: {
  brandName: string;
  industry: string;
  imagesDir: string;
  outputPath?: string | null;
  style?: string | null;
}): Promise<string | null> {
  const { brandName, industry, style } = options;
  const imagesDir = resolve(options.imagesDir);
  const isDir = await stat(imagesDir).then(
    (s) => s.isDirectory(),
    () => false,
  );
  if (!isDir) {
    console.log(`Error: Directory not found: ${imagesDir}`);
    return null;
  }

  const images = [...new Bun.Glob("*.{png,jpg,jpeg,webp}").scanSync({ cwd: imagesDir, onlyFiles: true })].sort();
  if (images.length === 0) {
    console.log(`Error: No PNG, JPEG or WebP images found in ${imagesDir}`);
    return null;
  }

  const brief = await getCipBrief(brandName, industry, style);
  const styleInfo = brief.style;
  const industryInfo = brief.industry;
  const brand = escapeHtml(brandName);

  const parts: string[] = [`<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>${brand} - Corporate Identity Program</title>
    <style>${CSS}</style>
</head>
<body>
    <section class="hero">
        <h1>${brand}</h1>
        <p class="subtitle">Corporate Identity Program</p>
        <div class="meta">
            <div class="meta-item">
                <div class="meta-label">Industry</div>
                <div class="meta-value">${escapeHtml(industryInfo.Industry ?? titleCase(industry))}</div>
            </div>
            <div class="meta-item">
                <div class="meta-label">Style</div>
                <div class="meta-value">${escapeHtml(styleInfo["Style Name"] ?? "Corporate")}</div>
            </div>
            <div class="meta-item">
                <div class="meta-label">Mood</div>
                <div class="meta-value">${escapeHtml(styleInfo.Mood ?? "Professional")}</div>
            </div>
            <div class="meta-item">
                <div class="meta-label">Deliverables</div>
                <div class="meta-value">${images.length} Items</div>
            </div>
        </div>
    </section>

    <section class="section">
        <h2 class="section-title">Brand Applications</h2>
        <p class="section-subtitle">
            Comprehensive identity system designed to maintain consistency
            across all brand touchpoints and communications.
        </p>
`];

  for (const name of images) {
    const imagePath = join(imagesDir, name);
    const info = getDeliverableInfo(basename(name, extname(name)));
    let src: string;
    try {
      const bytes = await Bun.file(imagePath).bytes();
      src = `data:${IMAGE_MIME[extname(name).toLowerCase()] ?? "image/png"};base64,${Buffer.from(bytes).toString("base64")}`;
    } catch (error) {
      console.log(`Warning: Could not load image ${imagePath}: ${(error as Error).message}`);
      src = imagePath;
    }
    parts.push(`
        <div class="deliverable">
            <div class="deliverable-image">
                <img src="${src}" alt="${info.title}" loading="lazy">
            </div>
            <div class="deliverable-content">
                <h3 class="deliverable-title">${info.title}</h3>
                <p class="deliverable-concept">${info.concept}</p>
                <p class="deliverable-purpose">${info.purpose}</p>
                <span class="deliverable-specs">${info.specs}</span>
            </div>
        </div>
`);
  }

  const generatedOn = new Date().toLocaleDateString("en-US", { month: "long", day: "2-digit", year: "numeric" });
  parts.push(`
    </section>

    <footer class="footer">
        <p><strong>${brand}</strong> Corporate Identity Program</p>
        <p>Generated on ${generatedOn}</p>
        <p style="margin-top: 1rem; font-size: 0.8rem;">Powered by CIP Design Skill</p>
    </footer>
</body>
</html>
`);

  const outputPath = options.outputPath ? resolve(options.outputPath) : join(imagesDir, `${brandName.toLowerCase().replaceAll(" ", "-").replace(/[^a-z0-9._-]+/g, "")}-cip-presentation.html`);
  await Bun.write(outputPath, parts.join(""));
  console.log(`✅ HTML presentation generated: ${outputPath}`);
  return outputPath;
}

async function main(): Promise<number> {
  const { values } = parseArgs({
    args: process.argv.slice(2),
    options: {
      brand: { type: "string", short: "b" },
      industry: { type: "string", short: "i", default: "technology" },
      style: { type: "string", short: "s" },
      images: { type: "string" },
      output: { type: "string", short: "o" },
    },
  });
  if (!values.brand || !values.images) {
    console.error("render-html.ts: error: the following arguments are required: --brand/-b, --images");
    return 2;
  }
  const written = await generateHtml({
    brandName: values.brand,
    industry: String(values.industry),
    imagesDir: values.images,
    outputPath: values.output,
    style: values.style,
  });
  return written ? 0 : 1;
}

if (import.meta.main) process.exit(await main());
