# shadcn/ui Theming & Customization

Theme configuration, CSS variables, dark mode, and component customization.

## Dark Mode Setup

### Next.js App Router

**1. Install next-themes:**
```bash
bun add next-themes
```

**2. Create theme provider:**
```tsx
// components/theme-provider.tsx
"use client"

import * as React from "react"
import { ThemeProvider as NextThemesProvider } from "next-themes"

export function ThemeProvider({
  children,
  ...props
}: React.ComponentProps<typeof NextThemesProvider>) {
  return <NextThemesProvider {...props}>{children}</NextThemesProvider>
}
```

**3. Wrap app:**
```tsx
// app/layout.tsx
import { ThemeProvider } from "@/components/theme-provider"

export default function RootLayout({ children }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body>
        <ThemeProvider
          attribute="class"
          defaultTheme="system"
          enableSystem
          disableTransitionOnChange
        >
          {children}
        </ThemeProvider>
      </body>
    </html>
  )
}
```

**4. Theme toggle component:**
```tsx
import { Moon, Sun } from "lucide-react"
import { useTheme } from "next-themes"
import { Button } from "@/components/ui/button"

export function ThemeToggle() {
  const { setTheme, theme } = useTheme()

  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={() => setTheme(theme === "light" ? "dark" : "light")}
    >
      <Sun className="h-[1.2rem] w-[1.2rem] rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
      <Moon className="absolute h-[1.2rem] w-[1.2rem] rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
      <span className="sr-only">Toggle theme</span>
    </Button>
  )
}
```

### Vite / Other Frameworks

Use similar approach with next-themes or implement custom solution:

```javascript
// Store preference
function toggleDarkMode() {
  const isDark = document.documentElement.classList.toggle('dark')
  localStorage.setItem('theme', isDark ? 'dark' : 'light')
}

// Initialize on load
if (localStorage.theme === 'dark' ||
    (!('theme' in localStorage) &&
     window.matchMedia('(prefers-color-scheme: dark)').matches)) {
  document.documentElement.classList.add('dark')
}
```

## CSS Variable System

shadcn/ui (Tailwind v4) uses CSS variables holding complete colors, defined in `globals.css` next to `@import "tailwindcss"` (current shadcn releases write `oklch()` values; any CSS color works):

```css
@import "tailwindcss";
@import "tw-animate-css";

@custom-variant dark (&:where(.dark, .dark *));

@layer base {
  :root {
    --background: hsl(0 0% 100%);
    --foreground: hsl(222.2 84% 4.9%);
    --primary: hsl(222.2 47.4% 11.2%);
    --primary-foreground: hsl(210 40% 98%);
    --secondary: hsl(210 40% 96.1%);
    --secondary-foreground: hsl(222.2 47.4% 11.2%);
    --muted: hsl(210 40% 96.1%);
    --muted-foreground: hsl(215.4 16.3% 46.9%);
    --accent: hsl(210 40% 96.1%);
    --accent-foreground: hsl(222.2 47.4% 11.2%);
    --destructive: hsl(0 84.2% 60.2%);
    --destructive-foreground: hsl(210 40% 98%);
    --border: hsl(214.3 31.8% 91.4%);
    --input: hsl(214.3 31.8% 91.4%);
    --ring: hsl(222.2 84% 4.9%);
    --radius: 0.5rem;
  }

  .dark {
    --background: hsl(222.2 84% 4.9%);
    --foreground: hsl(210 40% 98%);
    --primary: hsl(210 40% 98%);
    --primary-foreground: hsl(222.2 47.4% 11.2%);
    --secondary: hsl(217.2 32.6% 17.5%);
    --secondary-foreground: hsl(210 40% 98%);
    --muted: hsl(217.2 32.6% 17.5%);
    --muted-foreground: hsl(215 20.2% 65.1%);
    --accent: hsl(217.2 32.6% 17.5%);
    --accent-foreground: hsl(210 40% 98%);
    --destructive: hsl(0 62.8% 30.6%);
    --destructive-foreground: hsl(210 40% 98%);
    --border: hsl(217.2 32.6% 17.5%);
    --input: hsl(217.2 32.6% 17.5%);
    --ring: hsl(212.7 26.8% 83.9%);
  }
}
```

### Color Format

Variables hold complete colors, so Tailwind v4 opacity modifiers (`bg-primary/50`)
work through `color-mix()`:
```css
--primary: hsl(222.2 47.4% 11.2%);
--primary: oklch(0.205 0 0);   /* format written by current shadcn releases */
```

## Tailwind Configuration

Map CSS variables to Tailwind utilities in CSS with `@theme inline` (no
`tailwind.config.ts`; Bun builds process the stylesheet with a Tailwind Bun
plugin such as `@aphrody/bun-plugin-tailwind`):

```css
@theme inline {
  --color-border: var(--border);
  --color-input: var(--input);
  --color-ring: var(--ring);
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-destructive: var(--destructive);
  --color-destructive-foreground: var(--destructive-foreground);
  --color-muted: var(--muted);
  --color-muted-foreground: var(--muted-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --radius-lg: var(--radius);
  --radius-md: calc(var(--radius) - 2px);
  --radius-sm: calc(var(--radius) - 4px);
}
```

## Color Customization

### Method 1: Update CSS Variables

Change colors by modifying CSS variables in `globals.css`:

```css
:root {
  --primary: hsl(262.1 83.3% 57.8%);  /* Purple */
  --primary-foreground: hsl(210 20% 98%);
}

.dark {
  --primary: hsl(263.4 70% 50.4%);  /* Darker purple */
  --primary-foreground: hsl(210 20% 98%);
}
```

### Method 2: Theme Generator

Use shadcn/ui theme generator: https://ui.shadcn.com/themes

Select base color, generate theme, copy CSS variables.

### Method 3: Multiple Themes

Create theme variants with data attributes:

```css
[data-theme="violet"] {
  --primary: hsl(262.1 83.3% 57.8%);
  --primary-foreground: hsl(210 20% 98%);
}

[data-theme="rose"] {
  --primary: hsl(346.8 77.2% 49.8%);
  --primary-foreground: hsl(355.7 100% 97.3%);
}
```

Apply theme:
```tsx
<div data-theme="violet">
  <Button>Violet theme</Button>
</div>
```

## Component Customization

Components live in your codebase - modify directly.

### Customize Variants

```tsx
// components/ui/button.tsx
const buttonVariants = cva(
  "inline-flex items-center justify-center rounded-md text-sm font-medium",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground",
        destructive: "bg-destructive text-destructive-foreground",
        outline: "border border-input bg-background",
        // Add custom variant
        gradient: "bg-gradient-to-r from-purple-500 to-pink-500 text-white",
      },
      size: {
        default: "h-10 px-4 py-2",
        sm: "h-9 rounded-md px-3",
        lg: "h-11 rounded-md px-8",
        // Add custom size
        xl: "h-14 rounded-md px-10 text-lg",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)
```

Usage:
```tsx
<Button variant="gradient" size="xl">Custom Button</Button>
```

### Customize Styles

Modify base styles in component:

```tsx
// components/ui/card.tsx
const Card = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn(
      "rounded-xl border bg-card text-card-foreground shadow-lg",  // Modified
      className
    )}
    {...props}
  />
))
```

### Override with className

Pass additional classes to override:

```tsx
<Card className="border-2 border-purple-500 shadow-2xl hover:scale-105 transition-transform">
  Custom styled card
</Card>
```

## Base Color Presets

shadcn/ui provides base color presets during `init`:

- **Slate**: Cool gray tones
- **Gray**: Neutral gray
- **Zinc**: Warm gray
- **Neutral**: Balanced gray
- **Stone**: Earthy gray

Select during setup or change later by updating CSS variables.

## Style Variants

Two component styles available:

- **Default**: Softer, more rounded
- **New York**: Sharp, more contrast

Select during `init` or in `components.json`:

```json
{
  "style": "new-york",
  "tailwind": {
    "cssVariables": true
  }
}
```

## Radius Customization

Control border radius globally:

```css
:root {
  --radius: 0.5rem;  /* Default */
  --radius: 0rem;    /* Sharp corners */
  --radius: 1rem;    /* Rounded */
}
```

Components use radius variable:
```tsx
className="rounded-lg"  /* Uses --radius-lg, mapped to var(--radius) */
```

## Best Practices

1. **Use CSS Variables**: Enables runtime theme switching
2. **Consistent Foreground Colors**: Pair each color with appropriate foreground
3. **Test Both Themes**: Verify components in light and dark modes
4. **Semantic Naming**: Use `destructive` not `red`, `muted` not `gray`
5. **Accessibility**: Maintain sufficient color contrast (WCAG AA minimum)
6. **Component Overrides**: Use `className` prop for one-off customization
7. **Extract Patterns**: Create custom variants for repeated customizations
