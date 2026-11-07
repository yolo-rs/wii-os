# Wii Menu — Spécification Complète du Design System

Cette spécification définit les principes visuels, les tokens de design (CSS & JSON), la configuration **Tailwind CSS v4** (`@theme`) et les spécifications de composants **shadcn/ui** conformes à l'esthétique authentique du **Menu Nintendo Wii**.

---

## 1. Philosophie Visuelle (Frutiger Aero & Skeuomorphisme Épuré)

L'interface du menu Wii est une œuvre pionnière du mouvement **Frutiger Aero** (2005–2012) :
- **Matériaux Physiques** : Blanc éclatant brillant, plastiques moulés de haute qualité, docking bar inférieure en aluminium brossé (`#d2d6db`).
- **Glow Néon Cyan Wii** : Accentuation lumineuse néon cyan (`#54c5e5`) rappelant la diode bleue du lecteur de disque.
- **Forme Organique Convexe CRT** : Chaque vignette de chaîne adopte une découpe géométrique spécifique simulant un tube cathodique bombé (`clip-path: polygon(...)`).
- **Boutons Tactiles Bombés** : Dégradés 3D bi-tons avec reflets de brillance supérieure et physique de rebond (`scale(1.08)` au survol).
- **Rétroaction Auditive Immédiate** : Chaque action (survol, sélection, annulation, défilement) possède une signature sonore dédiée issue de `IplSound.brsar`.

---

## 2. Tokens de Design

### Palette Couleur & Rôles Sémantiques

| Token CSS | Valeur Hexadécimale | Rôle | Ratio de Contraste WCAG |
| :--- | :--- | :--- | :--- |
| `--wii-surface-base` | `#f5f7f8` | Toile de fond principale | Fond de scène |
| `--wii-surface-dock` | `#d2d6db` | Barre de navigation basse | 5.62:1 vs texte (AA) |
| `--wii-surface-card` | `#ffffff` | Surface des cartes de chaînes | 9.07:1 vs texte (AAA) |
| `--wii-led-cyan` | `#54c5e5` | Anneau de focus & halo au survol | Accent lumineux |
| `--wii-brand-blue` | `#00a4db` | Bouton "Démarrer" principal | 4.55:1 vs blanc (AA) |
| `--wii-ink-primary` | `#474747` | Typographie principale | 9.18:1 vs fond (AAA) |
| `--wii-ink-secondary`| `#84868a` | Date, heure, labels secondaires | 3.84:1 vs blanc (AA Large) |

### Dimensions & Grille 16:9

- **Scène** : Format 16:9 strict contraint par requête de conteneur (`container-type: size`).
- **Matrice** : 4 colonnes $\times$ 3 rangées = 12 chaînes par page.
- **Vignette de chaîne** : Largeur `20.2cqi` $\times$ Hauteur `11.1cqi` (ratio 1.82:1).
- **Hauteur du Dock** : `30.42cqb` (~219px à 720p).

---

## 3. Configuration Tailwind CSS v4 (`@theme`)

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

## 4. Composants Primitifs (shadcn/ui + Radix UI)

1. **`WiiChannelCard`** : Carte de chaîne 12-slots avec biseau externe blanc métallisé, masque polygonal organique, reflet en verre et halo néon cyan au focus.
2. **`WiiButton`** : Bouton poussoir tactile disponible en variantes `primary` (pilule blanche), `start` (bleu Wii étincelant) et `circular` (bouton paramètres / messages).
3. **`WiiChannelSplash`** : Dialogue modal à transition zoom (agrandissement de 0.3x à 1.0x en 400ms) affichant l'animation de la chaîne et la barre d'action inférieure.
4. **`WiiMenuLayout`** : Conteneur responsive 16:9 appliquant automatiquement les contraintes d'échelle, le fond texturé et les curseurs personnalisés.
