# Catalogue des Assets & Manifeste Système

Ce document répertorie l'inventaire officiel des assets extraits du WAD console ainsi que leur indexation dans [`assets/manifest.json`](file:///C:/src/wii/wii-os/assets/manifest.json).

---

## 1. Vue d'Ensemble & Catégories

Tous les assets consolidés sont stockés de façon normalisée dans :
📁 [`assets/system-menu/`](file:///C:/src/wii/wii-os/assets/system-menu/)

| Catégorie | Emplacement | Nombre d'Éléments | Description |
| :--- | :--- | :---: | :--- |
| **Typographie** | `assets/system-menu/fonts/` | 3 | Police TrueType système (`WiiNTLG-Regular.ttc`) et polices GX (`wbf1/wbf2.brfna`). |
| **Audio** | `assets/system-menu/audio/` | 2 | Banque audio brute (`IplSound.brsar`) et dictionnaire des 113 symboles (`sound_symbols.json`). |
| **Localisation** | `assets/system-menu/locales/` | 2 | Fichier binaire BMG et dictionnaire des 458 traductions françaises (`strings_fra.json`). |
| **Modèles 3D** | `assets/system-menu/models/` | 1 | Base géométrique 3D officielle de la Revolution Face Library (`RFL_Res.dat`). |
| **Design Tokens** | `assets/system-menu/tokens/` | 2 | Tokens normalisés en CSS (`tokens.css`) et JSON (`tokens.json`). |
| **Moteur Web** | `assets/system-menu/browser/` | 2 | Moteur Opera LZ77 original (`wwwlib-rvl.lz7`) et binaire décompressé 9.44 Mo (`wwwlib-rvl.bin`). |
| **Exécutable** | `assets/system-menu/executable/` | 1 | Binaire PowerPC Broadway `main.dol` (3.86 Mo). |

---

## 2. Table des Empreintes d'Intégrité (SHA-256)

Extrait du fichier [`assets/manifest.json`](file:///C:/src/wii/wii-os/assets/manifest.json) :

| Fichier | Taille | SHA-256 | Rôle Système |
| :--- | :--- | :--- | :--- |
| `fonts/WiiNTLG-Regular.ttc` | 2 668 948 o | `006abc0517bba72d6135ad1d04f911e99fafc6f6aff27ce71917a38acd8803f2` | Police système officielle New Rodin |
| `fonts/wbf1.brfna` | 1 839 732 o | `cf0e4f41fd53bd9b68b4a0691795060b3ea2120d8bf60eb825a718cb0611b596` | Texture de police GPU 1 |
| `fonts/wbf2.brfna` | 1 809 180 o | `a41abda00977b96bc09effbafd393cfa949e93504b875aa86c5e1d7741115d3a` | Texture de police GPU 2 |
| `audio/IplSound.brsar` | 2 927 232 o | `78c62ce1df5198bd4bb87284c6a8943ff5bb6fca7de7805080271d534d52bd78` | Banque audio BRSAR native |
| `audio/sound_symbols.json`| 2 708 o | `b8acf2faf1e4f5f293819e3bc151190f61af29a98a1a9c245057bad9128224fc` | Noms des 113 bruitages du menu |
| `locales/strings_fra.json`| 22 749 o | `6957d62f81b51cc03e9e3349323c301e63524f8ed01286cac6a46bc8e4da78d8` | 458 chaînes françaises traduites |
| `models/RFL_Res.dat` | 686 372 o | `7af8e9389f27b7adefb05d883c1bef76865a30d026d9754640c6413f56417223` | Bibliothèque géométrique 3D Mii |
| `browser/wwwlib-rvl.bin` | 9 446 432 o | `5f3a44ccc127fd153c90eae8e876cda3582ce65c9dfca61f864088fd2a1d1f0b` | Moteur Opera décompressé |
| `executable/main.dol` | 3 867 904 o | `04b0703ba30bb08886db4fe204f1473c3c61170f869cc1615d76ed367d621853` | Binaire exécutable PowerPC |
| `tokens/tokens.json` | 965 o | `ae09843fd72d7ef026d795803feca66c80962370cb2677b17893e44327da5797` | Tokens de couleurs et espacements |

---

## 3. Utilisation dans les Différentes Cibles

1. **Interface Web (`frontend/`)** :
   - Exploite [`tokens.css`](file:///C:/src/wii/wii-os/assets/system-menu/tokens/tokens.css) et [`strings_fra.json`](file:///C:/src/wii/wii-os/assets/system-menu/locales/strings_fra.json) pour l'affichage multilingue et le thème visuel.
   - Les sons MP3 de [`frontend/assets/audio/`](file:///C:/src/wii/wii-os/frontend/assets/audio/) correspondent exactement aux symboles de `IplSound.brsar`.
2. **Exécutable Natif (`wii.exe` / wgpu)** :
   - Charge [`WiiNTLG-Regular.ttc`](file:///C:/src/wii/wii-os/assets/system-menu/fonts/WiiNTLG-Regular.ttc) pour le rendu vectoriel des glyphes via FreeType ou glyphon.
   - Charge [`RFL_Res.dat`](file:///C:/src/wii/wii-os/assets/system-menu/models/RFL_Res.dat) pour instancier les maillages Mii dans le pipeline de rendu WebGPU / DirectX 12.
