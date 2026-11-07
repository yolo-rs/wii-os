# Wii OS — Documentation & Knowledge Base

Bienvenue dans la documentation technique centrale du projet **Wii OS**, un environnement système complet unifiant l'émulation, la décompilation/recompilation native et WebGPU du BIOS Wii (System Menu 4.3E), de son écosystème de chaînes et de son catalogue de jeux.

---

## 🗺️ Index de la Documentation

| Guide | Description | Rôle Principal |
| :--- | :--- | :--- |
| 📖 [**REVERSE-ENGINEERING.md**](REVERSE-ENGINEERING.md) | Rapport technique complet d'ingénierie inverse du WAD officiel System Menu 4.3E. | Cryptographie AES-128, Décompression ASH0 / U8, Broadway PowerPC 750CL. |
| 🎨 [**DESIGN-SYSTEM.md**](DESIGN-SYSTEM.md) | Spécification du Design System Wii Menu (Frutiger Aero / Skeuomorphisme). | Tokens CSS, Tailwind v4 `@theme`, Composants shadcn/ui & Radix UI. |
| 📦 [**ASSET-CATALOG.md**](ASSET-CATALOG.md) | Inventaire et cartographie des assets officiels et dérivés avec manifeste JSON. | Polices TrueType, Banque Audio BRSAR, Mii 3D, Moteur Opera, Textes BMG. |
| ⚙️ [**PIPELINE.md**](PIPELINE.md) | Guide d'automatisation de la pipeline d'extraction, décompression et déploiement. | Reproductibilité, scripts idempotents, `bun run pipeline:assets`. |
| 🎮 [**BUDOKAI-TENKAICHI-3-PORT-AND-WEBGPU.md**](BUDOKAI-TENKAICHI-3-PORT-AND-WEBGPU.md) | Pipeline de décompilation et portage WebGPU de Dragon Ball Z: Budokai Tenkaichi 3. | Décompilation DOL, Assets TX2D / ADX, Pipeline WebGPU native. |
| 🏛️ [**ARCHITECTURE.md**](../ARCHITECTURE.md) | Architecture globale du système Wii OS, micro-noyau et services. | Rust multi-plateforme, winit/wgpu, IPC et émulation Dolphin. |

---

## 📂 Organisation des Dossiers d'Assets et de Code

```
wii-os/
├── assets/                                 # Dossier d'assets consolidés & validés
│   ├── manifest.json                       # Manifeste d'intégrité JSON machine-readable
│   └── system-menu/                        # Données officielles décompressées & décodées
│       ├── fonts/                          # WiiNTLG-Regular.ttc, wbf1/wbf2.brfna
│       ├── audio/                          # IplSound.brsar, sound_symbols.json
│       ├── locales/                        # strings_fra.json (458 chaînes), ipl_common_noe.bmg
│       ├── models/                         # RFL_Res.dat (modèles 3D Mii)
│       ├── tokens/                         # tokens.json, tokens.css
│       ├── browser/                        # wwwlib-rvl.bin (moteur Opera décompressé 9.44 Mo)
│       └── executable/                     # main.dol (exécutable Broadway 3.86 Mo)
│
├── docs/                                   # Documentation unifiée du projet
│   ├── README.md                           # Le présent index
│   ├── REVERSE-ENGINEERING.md              # Ledger d'ingénierie inverse WAD
│   ├── DESIGN-SYSTEM.md                    # Spécifications Frutiger Aero & Tailwind v4
│   ├── ASSET-CATALOG.md                    # Catalogue des assets & manifeste
│   ├── PIPELINE.md                         # Pipeline reproductible d'assets
│   └── BUDOKAI-TENKAICHI-3-PORT-AND-WEBGPU.md
│
├── frontend/                               # Assets web authentiques (zéro HTML, zéro JS)
│   └── assets/                             # Styles CSS, tokens, textures PNG et audio MP3
│
├── scripts/                                # Scripts d'émulation et hooks
│   └── gecko/                              # Scripts Lua d'inspection d'état Dolphin / Gecko
│
├── userland/                               # Données locales, ROMs, WADs et builds
└── wii.exe                                 # Binaire unifié autonome Release (21.4 Mo)
│
└── userland/build/re/                      # Espace de travail de décompilation & staging
    └── system-menu-v514/                   # Extraction intermédiaire complète (3 338+ fichiers)
```

---

## 🚀 Démarrage Rapide de la Pipeline

Pour extraire, déchiffrer, décompresser et regénérer l'ensemble des assets et le manifeste de manière 100% automatisée et reproductible :

```bash
bun run pipeline:assets
```
*(ou directement : `bun scripts/pipeline-wad-assets.ts`)*
