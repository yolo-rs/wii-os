# Logo Design Reference

AI-powered logo design with 55+ styles, 30 color palettes, 25 industry guides. Gemini Nano Banana is the default provider; Atlas Cloud and MuAPI are also available as explicit opt-in providers.

## Scripts

| Script | Purpose |
|--------|---------|
| `scripts/logo/search.ts` | Search styles, colors, industries; generate design briefs |
| `scripts/logo/generate.ts` | Generate logos with Gemini Nano Banana, Atlas Cloud, or MuAPI |
| `scripts/logo/core.ts` | BM25 search engine for logo data |

## Commands

### Design Brief (Start Here)

```bash
bun scripts/logo/search.ts "tech startup modern" --design-brief -p "BrandName"
```

### Search Domains

```bash
# Styles
bun scripts/logo/search.ts "minimalist clean" --domain style

# Color palettes
bun scripts/logo/search.ts "tech professional" --domain color

# Industry guidelines
bun scripts/logo/search.ts "healthcare medical" --domain industry
```

### Generate Logo

**ALWAYS** use white background for output logos.

```bash
bun scripts/logo/generate.ts --brand "TechFlow" --style minimalist --industry tech
bun scripts/logo/generate.ts --prompt "coffee shop vintage badge" --style vintage
bun scripts/logo/generate.ts --brand "TechFlow" --provider atlas
bun scripts/logo/generate.ts --brand "TechFlow" --provider muapi
bun scripts/logo/generate.ts --brand "TechFlow" --provider muapi --muapi-model nano-banana-pro
```

Options: `--style`, `--industry`, `--prompt`, `--provider`, `--atlas-model`, `--muapi-model`

## Available Styles

| Category | Styles |
|----------|--------|
| General | Minimalist, Wordmark, Lettermark, Pictorial Mark, Abstract Mark, Mascot, Emblem, Combination Mark |
| Aesthetic | Vintage/Retro, Art Deco, Luxury, Playful, Corporate, Organic, Neon, Grunge, Watercolor |
| Modern | Gradient, Flat Design, 3D/Isometric, Geometric, Line Art, Duotone, Motion-Ready |
| Clever | Negative Space, Monoline, Split/Fragmented, Responsive/Adaptive |

## Color Psychology

| Color | Psychology | Best For |
|-------|------------|----------|
| Blue | Trust, stability | Finance, tech, healthcare |
| Green | Growth, natural | Eco, wellness, organic |
| Red | Energy, passion | Food, sports, entertainment |
| Gold | Luxury, premium | Fashion, jewelry, hotels |
| Purple | Creative, innovative | Beauty, creative, tech |

## Industry Defaults

| Industry | Style | Colors | Typography |
|----------|-------|--------|------------|
| Tech | Minimalist, Abstract | Blues, purples, gradients | Geometric sans |
| Healthcare | Professional, Line Art | Blues, greens, teals | Clean sans |
| Finance | Corporate, Emblem | Navy, gold | Serif or clean sans |
| Food | Vintage Badge, Mascot | Warm reds, oranges | Friendly, script |
| Fashion | Wordmark, Luxury | Black, gold, white | Elegant serif |

## Workflow

1. Generate design brief → `scripts/logo/search.ts --design-brief`
2. Generate logo variations → `scripts/logo/generate.ts --brand --style --industry`
3. Ask user about HTML preview → `AskUserQuestion` tool
4. If yes, use the bundled `ui-ux-pro-max` skill for the HTML gallery

## Detailed References

- `references/logo-style-guide.md` - Detailed style descriptions
- `references/logo-color-psychology.md` - Color meanings and combinations
- `references/logo-prompt-engineering.md` - AI generation prompts

## Setup

```bash
export GEMINI_API_KEY="your-key"  # scripts run with bun, no package install

# Optional Atlas Cloud provider
export ATLASCLOUD_API_KEY="your-key"

# Optional MuAPI provider
export MUAPI_API_KEY="your-key"
```

MuAPI uses the asynchronous model endpoint and prediction result API. See the
[MuAPI API reference](https://muapi.ai/docs/api-reference) for authentication
and the [nano-banana model contract](https://api.muapi.ai/api/v1/models/nano-banana)
or [nano-banana-pro model contract](https://api.muapi.ai/api/v1/models/nano-banana-pro)
for the current model-specific schemas. The logo generator supports both documented
model slugs and sends their shared required `prompt` plus optional `aspect_ratio`
fields; the Pro model also accepts an optional `resolution` field that this focused
logo workflow leaves at the provider default.
