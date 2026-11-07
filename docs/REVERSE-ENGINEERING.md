# Rapport Technique d'Ingénierie Inverse — SystemMenu-v514.wad

Rapport d'ingénierie inverse de bout en bout du conteneur de titre officiel **Nintendo Wii System Menu 4.3E** (PAL Europe, version **v514**), analysé depuis le fichier source [`userland/data/wii/wads/System Software/SystemMenu-v514.wad`](file:///C:/src/wii/wii-os/userland/data/wii/wads/System%20Software/SystemMenu-v514.wad).

---

## 1. Fiche d'Identité & Intégrité Cryptographique

- **Title ID** : `0000000100000002` (System Menu officiel Nintendo)
- **Version** : `514` (System Menu 4.3E)
- **Sous-système d'exploitation requis** : **IOS80** (`0000000100000050`, v6944)
- **Date de compilation** : **20 avril 2010 à 05:53 UTC**
- **Utilisateur & Machine de build** : `irduser@IPLBUIL` (Nintendo Integrated Research & Development)
- **Chaîne de hachage de l'artefact original** :
  - SHA-256 : `acca9021cce25e564ca01a44052234b8bf022e49ef706f4a9984612dc52a09f0`
  - MD5 : `cad93ca3ca41704b15bbee95b2e6aae2`
  - Taille : `23 801 856` octets

---

## 2. Dérivation Cryptographique (AES-128-CBC)

Les titres de la console Wii sont scellés cryptographiquement au moyen d'un Ticket RSA-2048 et d'un manifeste TMD signés par l'autorité racine Nintendo (`Root-CA00000001-CP00000004`).

### Dérivation de la clé de titre
1. **Common Key 0** : `ebe42a225e8593e448d9c5457381aaf7`
2. **Clé chiffrée extraite du Ticket (offset 0x1BF)** : `861ea8cef9813d8340ce2d267d00b438`
3. **Vecteur d'initialisation (IV)** : `00000001000000020000000000000000` (Title ID sur 8 octets suivi de 8 zéros)
4. **Clé de titre déchiffrée** :
   $$\text{Title Key} = \mathtt{33c5c117ae51c17682ff9f1c85f8d20a}$$

### Validation des 9 contenus
Chaque contenu $i$ est déchiffré avec la clé de titre et un IV défini par $\text{IV}_i = [\text{index} \gg 8, \; \text{index} \ \& \ \text{0xFF}, \; 0_{14}]$.
Les 9 contenus présentent un hachage SHA-1 identique à 100% aux signatures du TMD.

---

## 3. Cartographie Détaillée des 9 Contenus

| Index | Contenu ID | Taille | Format | Description & Rôle Système |
| :---: | :---: | :---: | :--- | :--- |
| **0** | `00000099` | 64 o | ASCII | Chaîne d'identification interne : `systemmenu.rvl.1004200553`. |
| **1** | `0000009a` | 7.68 Mo | U8 / ASH0 | **Archive Principale UI** : 63 fichiers (layouts, graphismes, sons BRSAR, chaînes BMG). |
| **2** | `00000009` | 4.55 Mo | U8 / LZ10 | **Navigateur Opera pour Wii** : `wwwlib-rvl.lz7` (décompressé en **9.44 Mo** `wwwlib-rvl.bin`). |
| **3** | `0000000a` | 2.66 Mo | U8 / TTF | **Police TrueType officielle** : `WiiNTLG-Regular.ttc` (New Rodin de Fontworks). |
| **4** | `0000000b` | 686 Ko | U8 / 3D | **Base de données Mii** : `RFL_Res.dat` (maillages 3D officiels des visages, yeux, cheveux). |
| **5** | `0000000c` | 3.64 Mo | U8 / BRFNA | **Polices de texture GX** : `wbf1.brfna`, `wbf2.brfna` pour le rendu matériel GPU. |
| **6** | `0000001b` | 606 Ko | U8 / ARC | **Moteur de manuel électronique** : `eZTSystemEU.arc` (guide interactif). |
| **7** | `00000033` | 70 Ko | U8 / ARC | **Mentions légales européennes** : `eZTNintendoEU.arc`. |
| **8** | `0000009b` | 3.86 Mo | DOL | **Binaire Exécutable Broadway** : `main.dol` (PowerPC 750CL, point d'entrée `0x80003400`). |

---

## 4. Décompression ASH0 & Extraction Récursive des Archives

Le format **ASH0** est un algorithme de compression propriétaire Nintendo (LZ77 étendu avec codage de Huffman) utilisé pour compacter les fichiers d'interface de la Wii :
- **42 fichiers `.ash` décompressés en `.arc`** (archives U8 brutes) via [`scripts/ashdec.exe`](file:///C:/src/wii/wii-os/scripts/ashdec.exe).
- **3 338 fichiers extraits** à travers les 44 archives U8 :
  * `chanSel_extracted` : 64 fichiers (layouts `.brlyt`, animations `.brlan`, textures `.tpl` de la grille de chaînes).
  * `cmnBtn_extracted` : 45 fichiers (boutons tactiles Wii).
  * `cursor_extracted` : 27 fichiers (curseur de la télécommande Wii).
  * `board_extracted` : 184 fichiers (bureau des messages Wii).
  * `sofkeybd_extracted` : 222 fichiers (clavier virtuel officiel).
  * `iplsetting_extracted` : 1 573 fichiers (interface HTML complète des réglages Wii).

---

## 5. Désassemblage du Point d'Entrée PowerPC (`main.dol`)

- **Processeur** : Broadway PowerPC 750CL à 729 MHz.
- **Adresse mémoire de chargement** : `0x80003400` (MEM1).
- **Section de données** : `0x8132FFE0` (MEM2).
- **Section BSS** : `0x81340000` (65 536 octets).

### Séquence d'Amorce Matérielle :
```assembly
0x80003400:  3c800011    lis      r4, 0x0011
0x80003404:  38840c64    addi     r4, r4, 3172       ; Configuration registre matériel HID0
0x80003408:  7c90fba6    mtspr    hid0, r4           ; Initialisation bus et caches
0x8000340c:  3c800000    lis      r4, 0
0x80003410:  38842000    addi     r4, r4, 8192       ; r4 = 0x00002000
0x80003414:  7c800124    mtmsr    r4                 ; Activation des registres à virgule flottante (FP)
0x80003418:  7c70faa6    mfspr    r3, hid0
0x8000341c:  6064c000    ori      r4, r3, 0xc000     ; Activation caches d'instructions et de données L1
0x80003420:  7c90fba6    mtspr    hid0, r4
0x80003424:  4c00012c    isync                       ; Synchronisation complète du pipeline
```
