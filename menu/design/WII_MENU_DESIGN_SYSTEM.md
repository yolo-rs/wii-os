# Wii Menu — Design System & Style Extraction

This document details the extracted design tokens, component specifications, Frutiger Aero aesthetic rules, and Tailwind CSS v4 translation for the Wii Menu interface.

---

## 1. Aesthetic Foundations (Frutiger Aero & Skeuomorphism)

The Nintendo Wii interface blends domestic hardware industrial design with clean digital minimalism:
- **Glossy Plastics & Metallic Bezels**: Soft off-white surfaces (`#f5f7f8`) with brushed chrome/aluminum bottom docking bar (`#d2d6db`).
- **Signature Wii Cyan Glow**: Subtle neon cyan LED accent (`#54c5e5`) mimicking the disc drive illumination, used for hover states and focus outlines.
- **Organic TV CRT Contour**: Channel cards feature a subtle convex screen silhouette defined via `clip-path: polygon(...)`.
- **Tactile 3D Buttons**: Dual-tone gradient pill buttons with gloss specular highlights and bounce animations (`scale(1.08)`).
- **Physical Audio Feedback**: Instant auditory confirmation on hover (`button-hover.mp3`), selection (`button-select.mp3`), page transition (`zip.mp3`), and cancel (`back.mp3`).

---

## 2. Design Tokens

### Color Palette

| Token | Hex / Value | Role | WCAG Contrast |
| :--- | :--- | :--- | :--- |
| `--wii-surface-base` | `#f5f7f8` | Primary background canvas | Base surface |
| `--wii-surface-dock` | `#d2d6db` | Lower metallic control bar | 5.62:1 vs ink (AA) |
| `--wii-surface-card` | `#ffffff` | Channel tile background | 9.07:1 vs ink (AAA) |
| `--wii-led-cyan` | `#54c5e5` | Active focus ring & hover glow | Highlight accent |
| `--wii-brand-blue` | `#00a4db` | Start button & active primary accent | 4.55:1 vs white (AA) |
| `--wii-ink-primary` | `#474747` | Primary text and headings | 9.18:1 vs base (AAA) |
| `--wii-ink-secondary`| `#84868a` | Labels, clock and secondary indicators | 3.84:1 vs white (AA Large) |

### Dimensions & Layout

- **Aspect Ratio**: 16:9 fixed canvas with `@container (aspect-ratio: 16/9)`.
- **Channel Matrix**: 4 columns × 3 rows (12 channels per page).
- **Channel Tile**: `20.2cqi` width × `11.1cqi` height (aspect ratio ~1.82:1).
- **Dock Height**: `30.42cqb` (~219px at 720p).
- **Organic CRT Polygon Clip**:
  ```css
  clip-path: polygon(
    2% 7%, 6% 2%, 52% 1%, 95% 3%, 98% 8%,
    100% 49%, 98% 92%, 95% 97%, 50% 99%,
    5% 97%, 2% 93%, 0 50%
  );
  ```

---

## 3. Tailwind CSS v4 `@theme` Definition

```css
@import "tailwindcss";

@theme {
  --color-wii-base: #f5f7f8;
  --color-wii-canvas: #e8ebef;
  --color-wii-dock: #d2d6db;
  --color-wii-cyan: #54c5e5;
  --color-wii-blue: #00a4db;
  --color-wii-deep: #28536e;
  --color-wii-border: #9bbbc4;
  --color-wii-ink: #474747;
  --color-wii-ink-muted: #84868a;

  --ease-wii-spring: cubic-bezier(0.34, 1.56, 0.64, 1);
  --ease-wii-smooth: cubic-bezier(0.25, 0.1, 0.25, 1);
}

@utility wii-channel-mask {
  clip-path: polygon(
    2% 7%, 6% 2%, 52% 1%, 95% 3%, 98% 8%,
    100% 49%, 98% 92%, 95% 97%, 50% 99%,
    5% 97%, 2% 93%, 0 50%
  );
}

@utility wii-pill-button {
  background: linear-gradient(180deg, #ffffff 0%, #ebeef2 45%, #d5dadf 50%, #e8ecf0 100%);
  border: 1.5px solid #a6acb5;
  box-shadow: inset 0 1.5px 1px #ffffff, 0 3px 6px rgba(0, 0, 0, 0.15);
  transition: transform 150ms cubic-bezier(0.34, 1.56, 0.64, 1), box-shadow 150ms ease;

  &:hover {
    transform: scale(1.08);
    box-shadow: inset 0 1.5px 1px #ffffff, 0 6px 14px rgba(84, 197, 229, 0.45);
    border-color: #54c5e5;
  }
}
```

---

## 4. Component Primitives

- `WiiChannelCard`: 12-slot grid tile supporting static noise animation, WebGPU texture rendering, hover zoom (`scale(1.03)`), and cyan bezel glow.
- `WiiPillButton`: Tactile push button with curved caps, 3D gradient, sound trigger, and spring bounce.
- `WiiCornerButton`: Circular chrome button for settings and message board / mail.
- `WiiBottomDock`: Chrome navigation bar housing page selectors, digital clock, and quick system links.
- `WiiChannelSplash`: Zoom modal dialog transitioning from the channel tile into full-screen preview with 'Wii Menu' and 'Start' action controls.
