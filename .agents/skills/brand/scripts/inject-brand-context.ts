#!/usr/bin/env bun
/**
 * inject-brand-context.ts
 *
 * Extracts brand context from markdown brand guidelines
 * and outputs a formatted system prompt addition.
 *
 * Usage:
 *   bun scripts/inject-brand-context.ts [path-to-guidelines]
 *   bun scripts/inject-brand-context.ts --json [path-to-guidelines]
 *
 * Default path: docs/brand-guidelines.md
 */

import { isAbsolute, join } from "node:path";

const DEFAULT_GUIDELINES_PATH = "docs/brand-guidelines.md";

export interface BrandColors {
  primary: string[];
  secondary: string[];
  neutral: string[];
  semantic: string[];
}

export interface Typography {
  heading: string | null;
  body: string | null;
  mono: string | null;
}

export interface Voice {
  traits: string[];
  prohibited: string[];
  personality: string;
}

export interface ImageStyle {
  basePrompt: string;
  keywords: string[];
  mood: string[];
  donts: string[];
  examplePrompts: Array<{ type: string; prompt: string }>;
}

export interface BrandContext {
  colors: BrandColors;
  typography: Typography;
  voice: Voice;
  attributes: Array<{ name: string; description: string }>;
  imageStyle: ImageStyle;
  source?: string;
  extractedAt?: string;
}

export function extractHexColors(text: string): string[] {
  return [...new Set(text.match(/#[0-9A-Fa-f]{6}\b/g) ?? [])];
}

export function extractColorsFromTable(content: string): BrandColors {
  const colors: BrandColors = { primary: [], secondary: [], neutral: [], semantic: [] };
  const primaryMatch = content.match(/### Primary Colors[\s\S]*?\|[\s\S]*?(?=###|$)/i);
  const secondaryMatch = content.match(/### Secondary Colors[\s\S]*?\|[\s\S]*?(?=###|$)/i);
  const neutralMatch = content.match(/### Neutral[\s\S]*?\|[\s\S]*?(?=###|$)/i);
  const semanticMatch = content.match(/### Semantic[\s\S]*?\|[\s\S]*?(?=###|$)/i);
  if (primaryMatch) colors.primary = extractHexColors(primaryMatch[0]);
  if (secondaryMatch) colors.secondary = extractHexColors(secondaryMatch[0]);
  if (neutralMatch) colors.neutral = extractHexColors(neutralMatch[0]);
  if (semanticMatch) colors.semantic = extractHexColors(semanticMatch[0]);
  return colors;
}

export function extractTypography(content: string): Typography {
  const typography: Typography = { heading: null, body: null, mono: null };
  const headingMatch = content.match(/--font-heading:\s*['"]([^'"]+)['"]/);
  const bodyMatch = content.match(/--font-body:\s*['"]([^'"]+)['"]/);
  const monoMatch = content.match(/--font-mono:\s*['"]([^'"]+)['"]/);

  const fontStackMatch = content.match(/### Font Stack[\s\S]*?(?=###|##|$)/i);
  if (fontStackMatch) {
    const stackText = fontStackMatch[0];
    const headingAlt = stackText.match(/heading[^']*['"]([^'"]+)['"]/i);
    const bodyAlt = stackText.match(/body[^']*['"]([^'"]+)['"]/i);
    if (headingAlt) typography.heading = headingAlt[1]!;
    if (bodyAlt) typography.body = bodyAlt[1]!;
  }
  if (headingMatch) typography.heading = headingMatch[1]!;
  if (bodyMatch) typography.body = bodyMatch[1]!;
  if (monoMatch) typography.mono = monoMatch[1]!;
  return typography;
}

export function extractVoice(content: string): Voice {
  const voice: Voice = { traits: [], prohibited: [], personality: "" };

  const personalityMatch = content.match(/### Brand Personality[\s\S]*?\|[\s\S]*?(?=###|##|$)/i);
  if (personalityMatch) {
    const traits = personalityMatch[0].match(/\*\*([^*]+)\*\*\s*\|\s*([^|]+)/g);
    if (traits) {
      voice.traits = traits
        .map((t) => {
          const match = t.match(/\*\*([^*]+)\*\*/);
          return match ? match[1]!.trim() : "";
        })
        .filter(Boolean);
    }
  }

  const prohibitedMatch = content.match(/### Prohibited[\s\S]*?(?=###|##|$)/i);
  if (prohibitedMatch) {
    const terms = prohibitedMatch[0].match(/\|\s*([^|]+)\s*\|/g);
    if (terms) {
      voice.prohibited = terms
        .map((t) => t.replace(/\|/g, "").trim())
        .filter((t) => t && !t.includes("Avoid") && !t.includes("---"));
    }
  }

  const forbiddenMatch = content.match(/### Forbidden Phrases[\s\S]*?(?=###|##|$)/i);
  if (forbiddenMatch && voice.prohibited.length === 0) {
    const items = forbiddenMatch[0].match(/-\s*["']?([^"'\n(]+)/g);
    if (items) {
      voice.prohibited = items.map((item) => item.replace(/^-\s*["']?/, "").trim()).filter(Boolean);
    }
  }

  voice.personality = voice.traits.join(", ");
  return voice;
}

export function extractCoreAttributes(content: string): Array<{ name: string; description: string }> {
  const attributes: Array<{ name: string; description: string }> = [];
  const attributesMatch = content.match(/### Core Attributes[\s\S]*?\|[\s\S]*?(?=###|##|$)/i);
  if (attributesMatch) {
    const rows = attributesMatch[0].match(/\|\s*\*\*([^*]+)\*\*\s*\|\s*([^|]+)\|/g);
    for (const row of rows ?? []) {
      const match = row.match(/\*\*([^*]+)\*\*\s*\|\s*([^|]+)/);
      if (match) attributes.push({ name: match[1]!.trim(), description: match[2]!.trim() });
    }
  }
  return attributes;
}

export function extractImageStyle(content: string): ImageStyle {
  const imageStyle: ImageStyle = { basePrompt: "", keywords: [], mood: [], donts: [], examplePrompts: [] };

  const basePromptMatch = content.match(/### Base Prompt Template[\s\S]*?```\n?([\s\S]*?)```/i);
  if (basePromptMatch) imageStyle.basePrompt = basePromptMatch[1]!.trim().replace(/\n/g, " ");

  const keywordsMatch = content.match(/### Style Keywords[\s\S]*?\|[\s\S]*?(?=###|##|$)/i);
  if (keywordsMatch) {
    const keywordRows = keywordsMatch[0].match(/\|\s*\*\*[^*]+\*\*\s*\|\s*([^|]+)\|/g);
    for (const row of keywordRows ?? []) {
      const match = row.match(/\|\s*\*\*[^*]+\*\*\s*\|\s*([^|]+)\|/);
      if (match) {
        imageStyle.keywords.push(
          ...match[1]!
            .split(",")
            .map((k) => k.trim())
            .filter(Boolean),
        );
      }
    }
  }

  const moodMatch = content.match(/### Visual Mood Descriptors[\s\S]*?(?=###|##|$)/i);
  if (moodMatch) {
    const moodItems = moodMatch[0].match(/-\s*([^\n]+)/g);
    if (moodItems) imageStyle.mood = moodItems.map((item) => item.replace(/^-\s*/, "").trim());
  }

  const dontsMatch = content.match(/### Visual Don'ts[\s\S]*?\|[\s\S]*?(?=###|##|$)/i);
  if (dontsMatch) {
    const dontRows = dontsMatch[0].match(/\|\s*([^|]+)\s*\|\s*([^|]+)\s*\|/g);
    for (const row of dontRows ?? []) {
      const match = row.match(/\|\s*([^|]+)\s*\|\s*([^|]+)\s*\|/);
      if (match && !match[1]!.includes("Avoid") && !match[1]!.includes("---")) {
        imageStyle.donts.push(match[1]!.trim());
      }
    }
  }

  const exampleMatch = content.match(/### Example Prompts[\s\S]*?(?=##|$)/i);
  if (exampleMatch) {
    const prompts = exampleMatch[0].match(/\*\*([^*]+)\*\*:\s*```\n?([\s\S]*?)```/g);
    for (const p of prompts ?? []) {
      const match = p.match(/\*\*([^*]+)\*\*:\s*```\n?([\s\S]*?)```/);
      if (match) {
        imageStyle.examplePrompts.push({ type: match[1]!.trim(), prompt: match[2]!.trim().replace(/\n/g, " ") });
      }
    }
  }
  return imageStyle;
}

export function extractBrandContext(content: string): BrandContext {
  return {
    colors: extractColorsFromTable(content),
    typography: extractTypography(content),
    voice: extractVoice(content),
    attributes: extractCoreAttributes(content),
    imageStyle: extractImageStyle(content),
  };
}

export function generatePromptAddition(brandContext: BrandContext): string {
  const { colors, typography, voice, attributes, imageStyle } = brandContext;

  let prompt = `
BRAND CONTEXT:
==============

VISUAL IDENTITY:
- Primary Colors: ${colors.primary.join(", ") || "Not specified"}
- Secondary Colors: ${colors.secondary.join(", ") || "Not specified"}
- Typography: ${typography.heading || typography.body || "System fonts"}

BRAND VOICE:
- Personality: ${voice.personality || "Professional"}
- Core Attributes: ${attributes.map((a) => a.name).join(", ") || "Not specified"}

CONTENT RULES:
- Prohibited Terms: ${voice.prohibited.join(", ") || "None specified"}
`;

  if (imageStyle?.basePrompt) {
    prompt += `
IMAGE GENERATION:
- Base Prompt: ${imageStyle.basePrompt}
- Style Keywords: ${imageStyle.keywords.slice(0, 10).join(", ") || "Not specified"}
- Visual Mood: ${imageStyle.mood.slice(0, 5).join("; ") || "Not specified"}
- Avoid: ${imageStyle.donts.join(", ") || "None specified"}
`;
  }

  prompt += `
Apply these brand guidelines to all generated content.
Maintain consistent voice, colors, and messaging.
`;
  return prompt.trim();
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const jsonOutput = args.includes("--json");
  const guidelinesPath = args.find((a) => !a.startsWith("--")) || DEFAULT_GUIDELINES_PATH;
  const resolvedPath = isAbsolute(guidelinesPath) ? guidelinesPath : join(process.cwd(), guidelinesPath);

  const file = Bun.file(resolvedPath);
  if (!(await file.exists())) {
    console.error(`Error: Brand guidelines not found at ${resolvedPath}`);
    console.error(`Create brand guidelines at ${DEFAULT_GUIDELINES_PATH} or specify a path.`);
    process.exit(1);
  }

  const brandContext: BrandContext = {
    ...extractBrandContext(await file.text()),
    source: resolvedPath,
    extractedAt: new Date().toISOString(),
  };

  console.log(jsonOutput ? JSON.stringify(brandContext, null, 2) : generatePromptAddition(brandContext));
}

if (import.meta.main) await main();
