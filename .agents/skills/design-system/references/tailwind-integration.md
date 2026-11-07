# Tailwind Integration

Map design system tokens to Tailwind CSS v4. Tailwind v4 is configured in CSS:
semantic tokens are CSS custom properties, `@theme inline` exposes them as
utilities, and there is no `tailwind.config.js`.

## CSS Variables Setup

### Entry Stylesheet

```css
/* app/globals.css (Next.js) or src/index.css (Vite / Bun) */
@import "tailwindcss" source("..");
@import "tw-animate-css";
@source "../../packages/ui/src";          /* workspace UI packages outside the app */

@custom-variant dark (&:where(.dark, .dark *));

:root {
  color-scheme: light;

  /* Primitives */
  --color-blue-600: hsl(217 91% 60%);

  /* Semantic */
  --background: hsl(0 0% 100%);
  --foreground: hsl(222 47% 11%);
  --primary: hsl(217 91% 60%);
  --primary-foreground: hsl(0 0% 100%);
  --secondary: hsl(220 14% 96%);
  --secondary-foreground: hsl(222 47% 11%);
  --muted: hsl(220 14% 96%);
  --muted-foreground: hsl(220 9% 46%);
  --accent: hsl(220 14% 96%);
  --accent-foreground: hsl(222 47% 11%);
  --destructive: hsl(0 84% 60%);
  --destructive-foreground: hsl(0 0% 100%);
  --card: hsl(0 0% 100%);
  --card-foreground: hsl(222 47% 11%);
  --border: hsl(220 13% 91%);
  --input: hsl(220 13% 91%);
  --ring: hsl(217 91% 60%);
  --radius: 0.5rem;
}

.dark {
  color-scheme: dark;
  --background: hsl(222 47% 4%);
  --foreground: hsl(210 40% 98%);
  --primary: hsl(217 91% 60%);
  --primary-foreground: hsl(0 0% 100%);
  --secondary: hsl(217 33% 17%);
  --secondary-foreground: hsl(210 40% 98%);
  --muted: hsl(217 33% 17%);
  --muted-foreground: hsl(215 20% 65%);
  --accent: hsl(217 33% 17%);
  --accent-foreground: hsl(210 40% 98%);
  --destructive: hsl(0 62% 30%);
  --destructive-foreground: hsl(0 0% 100%);
  --card: hsl(222 47% 6%);
  --card-foreground: hsl(210 40% 98%);
  --border: hsl(217 33% 17%);
  --input: hsl(217 33% 17%);
  --ring: hsl(217 91% 60%);
}
```

## Theme Mapping

### @theme inline

`inline` makes utilities reference the variables (so `.dark` switches them)
instead of copying their values:

```css
@theme inline {
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-muted: var(--muted);
  --color-muted-foreground: var(--muted-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --color-destructive: var(--destructive);
  --color-destructive-foreground: var(--destructive-foreground);
  --color-card: var(--card);
  --color-card-foreground: var(--card-foreground);
  --color-border: var(--border);
  --color-input: var(--input);
  --color-ring: var(--ring);
  --radius-lg: var(--radius);
  --radius-md: calc(var(--radius) - 2px);
  --radius-sm: calc(var(--radius) - 4px);
}
```

`bun scripts/generate-tokens.ts --config tokens.json --format tailwind -o assets/design-tokens.theme.css`
produces this block from a design-tokens JSON file.

## Opacity Modifiers

Tailwind v4 applies opacity with `color-mix()`, so variables can hold complete
colors in any format (hex, `hsl()`, `oklch()`):

```tsx
<div className="bg-primary/50">   // 50% opacity
<div className="text-primary/80"> // 80% opacity

// CSS output
background-color: color-mix(in oklab, var(--color-primary) 50%, transparent);
```

## Component Classes

### Button Example

```css
@layer components {
  .btn {
    @apply inline-flex items-center justify-center
           rounded-md font-medium
           transition-colors motion-reduce:transition-none
           focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring
           disabled:pointer-events-none disabled:opacity-50;
  }

  .btn-default {
    @apply bg-primary text-primary-foreground
           hover:bg-primary/90;
  }

  .btn-secondary {
    @apply bg-secondary text-secondary-foreground
           hover:bg-secondary/80;
  }

  .btn-outline {
    @apply border border-input bg-background
           hover:bg-accent hover:text-accent-foreground;
  }

  .btn-ghost {
    @apply hover:bg-accent hover:text-accent-foreground;
  }

  .btn-destructive {
    @apply bg-destructive text-destructive-foreground
           hover:bg-destructive/90;
  }

  /* Sizes */
  .btn-sm { @apply h-8 px-3 text-xs; }
  .btn-md { @apply h-10 px-4 text-sm; }
  .btn-lg { @apply h-12 px-6 text-base; }
}
```

Use a real `outline` for focus (not only `ring`/`box-shadow`): Windows forced-colors
mode drops box shadows but keeps outlines.

## Spacing Integration

```css
@theme {
  --spacing-section: var(--space-section, 6rem);
  --spacing-component: var(--space-component, 1.5rem);
}
```

## Animation Tokens

```css
@theme {
  --ease-standard: cubic-bezier(0.2, 0, 0, 1);
  --animate-accordion-down: accordion-down 0.2s ease-out;
  --animate-accordion-up: accordion-up 0.2s ease-out;

  @keyframes accordion-down {
    from { height: 0; }
    to { height: var(--radix-accordion-content-height); }
  }
  @keyframes accordion-up {
    from { height: var(--radix-accordion-content-height); }
    to { height: 0; }
  }
}

@media (prefers-reduced-motion: reduce) {
  *, ::before, ::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

Duration utilities (`duration-150`, `duration-200`, `duration-300`) are built in.

## Dark Mode Toggle

```typescript
// Toggle dark mode
function toggleDarkMode() {
  document.documentElement.classList.toggle('dark')
}

// System preference
if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
  document.documentElement.classList.add('dark')
}
```

## Bun Builds

`bun build` and `Bun.serve` HTML imports process the entry stylesheet with a
Tailwind Bun plugin. The Aphrody / Bun OS workspace uses
`@aphrody/bun-plugin-tailwind`; Next.js apps use `@tailwindcss/postcss`.

## shadcn/ui Alignment

This configuration aligns with shadcn/ui conventions for Tailwind v4:

- Same CSS variable naming (`--primary`, `--primary-foreground`, ...)
- Complete color values in variables, mapped with `@theme inline`
- `@custom-variant dark` for the `.dark` class
- Compatible with `bunx --bun shadcn@latest add` commands

### Using with shadcn/ui

```bash
# Initialize (uses same token structure)
bunx --bun shadcn@latest init

# Add components (styled with these tokens)
bunx --bun shadcn@latest add button card input
```
