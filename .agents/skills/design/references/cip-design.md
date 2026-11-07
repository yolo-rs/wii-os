# CIP Design Reference

Corporate Identity Program design with 50+ deliverables, 20 styles, 20 industries. Generate mockups with Gemini Nano Banana (Flash/Pro).

## Scripts

| Script | Purpose |
|--------|---------|
| `scripts/cip/search.ts` | Search deliverables, styles, industries; generate CIP briefs |
| `scripts/cip/generate.ts` | Generate CIP mockups with Gemini (Flash/Pro) |
| `scripts/cip/render-html.ts` | Render HTML presentation from CIP mockups |
| `scripts/cip/core.ts` | BM25 search engine for CIP data |

## Commands

### CIP Brief (Start Here)

```bash
bun scripts/cip/search.ts "tech startup" --cip-brief -b "BrandName"
```

### Search Domains

```bash
# Deliverables
bun scripts/cip/search.ts "business card letterhead" --domain deliverable

# Design styles
bun scripts/cip/search.ts "luxury premium elegant" --domain style

# Industry guidelines
bun scripts/cip/search.ts "hospitality hotel" --domain industry

# Mockup contexts
bun scripts/cip/search.ts "office reception" --domain mockup
```

### Generate Mockups

```bash
# With logo (RECOMMENDED - uses image editing)
bun scripts/cip/generate.ts --brand "TopGroup" --logo /path/to/logo.png --deliverable "business card" --industry "consulting"

# Full CIP set with logo
bun scripts/cip/generate.ts --brand "TopGroup" --logo /path/to/logo.png --industry "consulting" --set

# Pro model for 4K text rendering
bun scripts/cip/generate.ts --brand "TopGroup" --logo logo.png --deliverable "business card" --model pro

# Custom deliverables with aspect ratio
bun scripts/cip/generate.ts --brand "GreenLeaf" --logo logo.png --industry "organic food" --deliverables "letterhead,packaging,vehicle" --ratio 16:9

# Without logo (AI generates interpretation)
bun scripts/cip/generate.ts --brand "TechFlow" --deliverable "business card" --no-logo-prompt
```

### Render HTML Presentation

```bash
bun scripts/cip/render-html.ts --brand "TopGroup" --industry "consulting" --images /path/to/cip-output
bun scripts/cip/render-html.ts --brand "TopGroup" --industry "consulting" --images ./topgroup-cip --output presentation.html
```

## Models

- `flash` (default): `gemini-2.5-flash-image` - Fast, cost-effective
- `pro`: `gemini-3-pro-image` - Quality, 4K text rendering

Set `GEMINI_FLASH_IMAGE_MODEL` or `GEMINI_PRO_IMAGE_MODEL` to use another Gemini model id.

## Deliverable Categories

| Category | Items |
|----------|-------|
| Core Identity | Logo, Logo Variations |
| Stationery | Business Card, Letterhead, Envelope, Folder, Notebook, Pen |
| Security/Access | ID Badge, Lanyard, Access Card |
| Office Environment | Reception Signage, Wayfinding, Meeting Room Signs, Wall Graphics |
| Apparel | Polo Shirt, T-Shirt, Cap, Jacket, Apron |
| Promotional | Tote Bag, Gift Box, USB Drive, Water Bottle, Mug, Umbrella |
| Vehicle | Car Sedan, Van, Truck |
| Digital | Social Media, Email Signature, PowerPoint, Document Templates |
| Product | Packaging Box, Labels, Tags, Retail Display |
| Events | Trade Show Booth, Banner Stand, Table Cover, Backdrop |

## Design Styles

| Style | Colors | Best For |
|-------|--------|----------|
| Corporate Minimal | Navy, White, Blue | Finance, Legal, Consulting |
| Modern Tech | Purple, Cyan, Green | Tech, Startups, SaaS |
| Luxury Premium | Black, Gold, White | Fashion, Jewelry, Hotels |
| Warm Organic | Brown, Green, Cream | Food, Organic, Artisan |
| Bold Dynamic | Red, Orange, Black | Sports, Entertainment |

## HTML Presentation Features

- Hero section with brand name, industry, style, mood
- Deliverable cards with mockup images
- Descriptions: concept, purpose, specifications
- Responsive desktop/mobile, dark theme
- Images embedded as base64 (single-file portable)

## Workflow

1. Generate CIP brief → `scripts/cip/search.ts --cip-brief`
2. Generate mockups with logo → `scripts/cip/generate.ts --brand --logo --industry --set`
3. Render HTML presentation → `scripts/cip/render-html.ts --brand --industry --images`

**Tip:** If no logo exists, use Logo Design (built-in) to generate one first.

## Detailed References

- `references/cip-deliverable-guide.md` - Deliverable specifications
- `references/cip-style-guide.md` - Design style descriptions
- `references/cip-prompt-engineering.md` - AI generation prompts

## Setup

```bash
export GEMINI_API_KEY="your-key"  # or GOOGLE_API_KEY; scripts run with bun, no package install
```
