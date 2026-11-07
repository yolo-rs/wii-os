# Wii OS Web : Architecture Système & Production

Document d'architecture généré à partir de [replica/recon.md](file:///C:/src/wii/wii-os/replica/recon.md) et [replica/features.csv](file:///C:/src/wii/wii-os/replica/features.csv).

---

## 1. La Stack Technique

| Couche | Choix Technologique | Raison |
| :--- | :--- | :--- |
| **Interface Web & Wii Shell** | HTML5 Sémantique + CSS3 Grid/Tokens + Rust WASM (`crates/menu-web`) | Réplique pixel-perfect du Menu Wii 4.3E avec son authentique (Web Audio), shaders WebGPU et Wiimote sans framework lourd. |
| **Moteur d'Émulation Web** | Gecko WebGPU WASM (`crates/gecko-web`, Rust + wgpu) | Démarrage instantané 60 FPS dans le canvas sans plug-in avec isolation mémoire SharedArrayBuffer. |
| **Serveur d'Origine / Control Plane** | Bun v1.4.4+ Native HTTP/WebSocket (`server.mjs`) & Rust Axum (`apps/server`) | Support natif des requêtes HTTP Range 206, streaming de disques RVZ, WebSocket haute cadence et API MCP. |
| **Passerelle & Reverse Proxy Edge** | Nginx 1.28+ avec modules `http_slice_module` et `http_ssl_module` | Mise en cache par tranches de 8 Mo (`slice 8m;`) pour les images disques de plusieurs gigaoctets et en-têtes stricts COOP/COEP. |
| **Base de Données** | SQLite 3.35+ embarqué en mode WAL (`PRAGMA journal_mode = WAL;`) | Base portable sans dépendance externe, transactions ACID, accès sub-milliseconde pour salons et paramètres. |
| **Temps Réel & Présence** | WebSocket natif Bun/Axum (`/v1/ws`) | Synchronisation d'état des salons NetPlay, télémétrie GPU et chat sans surcharge serveur. |
| **Sécurité & Authentification** | Cookie de session `HttpOnly; SameSite=Strict; Secure` + Token API Bearer 32 car. | Zéro stockage de secret dans le `localStorage` du navigateur; isolation CSRF totale. |
| **Distribution & PWA** | Web App Manifest + Service Worker natif (Cache API) | Shell d'interface consultable hors ligne avec préchargement intelligent des textures et sons Wii. |

---

## 2. Le Schéma de Données (7 Tables)

Le schéma complet est formalisé dans [replica/schema.sql](file:///C:/src/wii/wii-os/replica/schema.sql).

### Entités principales :
1. **`sysconf_settings`** : Configuration système 4.3E (format 16:9, résolution 480p, backend vidéo WebGPU, volume BGM, sensibilité barre sensorielle).
2. **`channels`** : Grille des 48 chaînes (4 pages de 12 slots), type de contenu (`wii-disc`, `wii-wad`, `homebrew`), blocs mémoire et métadonnées.
3. **`rooms`** : Salons multijoueurs persistants (Dolphin NetPlay FFI, hôte, port UDP, buffer de latence pad, capacité joueurs).
4. **`room_messages`** : Historique de messagerie et de coordination des arènes de combat.
5. **`board_messages`** : Bureau Wii / Journal quotidien (temps de jeu enregistré, mémos utilisateur, messages système Shenron).
6. **`nand_titles`** : Comptabilité du stockage NAND système (sauvegardes, chaînes installées, blocs libres/occupés sur les 2174 blocs système).
7. **`launch_audit_logs`** : Journal d'audit des lancements (mode WebGPU vs natif, adresse IP, durée de session, statut de terminaison).

### Règles d'accès et contraintes dures :
- **Atomicité des salons** : Vérification des limites de joueurs (`max_players BETWEEN 2 AND 16`) directement dans SQLite.
- **Intégrité référentielle** : Suppression en cascade (`ON DELETE CASCADE`) des messages liés à un salon fermé.
- **Isolement des écritures** : Seul le serveur local ou un appelant authentifié (cookie sécurisé ou token) peut muter les tables `sysconf_settings`, `channels` et `rooms`.

---

## 3. L'API Système (Parcours F01 à F06)

| Méthode | Route | Rôle & Action | Accès | Entrées | Sorties | Parcours |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `GET` | `/` | Sert le shell du Menu Wii 4.3E | Public | En-têtes HTTP | Document HTML unifié | Shell |
| `GET` | `/games/budokai-tenkaichi-3` | Lecteur de jeu plein écran immédiat | Public | En-têtes WebGPU | Canvas plein écran 100vw x 100vh | **F01** |
| `GET` | `/cdn/shenron/games/:slug.rvz` | Flux binaire disque Wii avec Range 206 | Public | `Range: bytes=start-end` | HTTP 206 (tranches 8 Mo en cache) | **F01** |
| `GET` | `/menu.catalog.json` | Catalogue des 48 slots de chaînes | Public | - | JSON `Catalog` validé | **F01** |
| `POST` | `/v1/apps/:id/launch` | Ordonne le lancement d'un titre | Protégé | `{ appId, videoBackend }` | `{ pid, emulatorUrl, status }` | **F01** |
| `GET` | `/v1/rooms` | Liste des salons multijoueurs actifs | Public | - | Liste JSON des salons | **F02** |
| `POST` | `/v1/rooms` | Création d'un salon persistant | Protégé | `{ roomName, port, padBuffer }`| `{ id, status: "lobby" }` | **F02** |
| `GET` | `/v1/ws` | Connexion bidirectionnelle présence/chat | Public/Token | Upgrade WebSocket | Flux d'événements JSON | **F03** |
| `GET` | `/v1/sysconf` | Lecture des paramètres système Wii | Public | - | JSON Sysconf complet | **F04** |
| `PUT` | `/v1/sysconf` | Modification d'un paramètre (ex. WebGPU) | Protégé | Clés Sysconf à mettre à jour | Sysconf mis à jour | **F04** |
| `GET` | `/v1/nand/summary` | État des blocs NAND et sauvegardes | Public | - | `{ totalBlocks, freeBlocks, entries }` | **F04** |
| `GET` | `/manifest.webmanifest` | Manifeste PWA pour installation | Public | - | Manifeste JSON standard | **F05** |
| `GET` | `/sw.js` | Service Worker pour cache hors ligne | Public | - | Script JS Service Worker | **F05** |
| `GET` | `/auth/status` | Vérifie l'état de la session d'accès | Public | Cookie session | `{ authenticated: bool }` | **F06** |
| `POST` | `/auth/session` | Déverrouille l'instance avec le secret | Public | `{ token }` | Pose cookie `HttpOnly` | **F06** |
| `POST` | `/mcp` | Protocole Model Context Protocol local | Protégé | Requêtes JSON-RPC 2.0 MCP | Réponses JSON-RPC 2.0 | **F06** |

---

## 4. Les Pièges et Points Critiques ("The Parts That Bite")

1. **Streaming partiel HTTP 206 et découpage Nginx Slice** :
   Les images disque Wii font entre 1,4 Go et 4,7 Go (Budokai Tenkaichi 3 = 2,12 Go). Sans le module `http_slice_module` configuré à 8 Mo, Nginx mettrait en cache le fichier entier en mémoire avant de répondre ou refuserait le cache des sous-requêtes Range. La directive `slice 8m;` associée à `proxy_cache_key $uri$slice_range;` garantit un premier chargement en moins de 1 seconde.
2. **Exigences strictes du contexte WebGPU & SharedArrayBuffer** :
   Le multithreading WebAssembly du moteur Gecko requiert impérativement les en-têtes `Cross-Origin-Opener-Policy: same-origin` et `Cross-Origin-Embedder-Policy: require-corp`. Si un seul asset externe ou script tiers omet ces en-têtes, le navigateur désactive `SharedArrayBuffer` et bloque le runtime WebGPU.
3. **Démarrage audio et politique d'autoplay des navigateurs** :
   Les navigateurs modernes bloquent le son non sollicité (`AudioContext` suspendu tant qu'aucune interaction utilisateur n'a eu lieu). Le Menu Wii et le jeu capturent le premier clic ou la touche clavier pour débloquer de manière transparente l'audio (`startup.mp3`, `bg-music.mp3`, et le flux audio stéréo 48 kHz du DSP Wii).
4. **Idempotence des lancements de jeu et fuites de processus** :
   Des clics rapides répétés sur la chaîne disque pourraient tenter de lancer plusieurs instances WebGPU ou Dolphin. Le client et le serveur verrouillent l'état de démarrage via un identifiant de session unique (`launch_id`).
5. **Persistance WAL et fermeture propre de SQLite** :
   L'utilisation de SQLite avec `PRAGMA journal_mode = WAL;` nécessite que les processus lisent et écrivent sans bloquer les lecteurs. Les sauvegardes doivent copier à chaud la base et son journal `-wal` ou invoquer l'API `sqlite3_backup`.

---

## 5. Ordre de Construction (Milestones)

### Milestone 1 : Vertical Slice (Validé & Actif)
- **Objectif** : Boucle centrale de bout en bout : ouverture du Menu Wii → sélection de la Chaîne Disque → transition vers `/games/budokai-tenkaichi-3` → auto-boot WebGPU 60 FPS avec Nginx Slice Cache (`X-Cache-Status: HIT`).
- **Écrans** : S01 (Accueil Wii Menu), S02 (Lecteur Plein Écran BT3).
- **Tables** : `sysconf_settings`, `channels`.
- **Routes** : `/`, `/games/budokai-tenkaichi-3`, `/cdn/shenron/games/budokai-tenkaichi-3-eu.rvz`, `/menu.catalog.json`.

### Milestone 2 : Must-Haves (Salons & Paramètres)
- **Objectif** : Création et gestion des arènes multijoueurs NetPlay persistantes en SQLite et gestion des paramètres vidéo/système (PAL, 480p, WebGPU).
- **Écrans** : S03 (Multijoueur / Salons), S04 (Paramètres Wii).
- **Tables** : `rooms`, `room_messages`, `sysconf_settings`.
- **Routes** : `/v1/rooms`, `/v1/sysconf`, `/v1/ws`, `/v1/nand/summary`.

### Milestone 3 : Should-Haves (PWA, Recherche & Bureau Wii)
- **Objectif** : Installation PWA sur desktop/mobile, navigation complète au clavier/Wiimote (gamepad API standard), mémos du Bureau Wii.
- **Écrans** : S05 (Bureau Wii / Messages), S06 (Chaîne Mii / Profil).
- **Tables** : `board_messages`, `launch_audit_logs`.
- **Routes** : `/v1/board/messages`, `/manifest.webmanifest`, `/sw.js`.

### Milestone 4 : Could-Haves & Optimisations Avancées
- **Objectif** : Gestionnaire de sauvegardes NAND exportables au format Dolphin standard, synchronisation WebRTC entre pairs pour le NetPlay.
- **Tables** : `nand_titles` étendu, snapshots binaires.

### Milestone 5 : Architecture Pure-Rust, C/C++ & BIOS Wii 4.3E Réel (Validé & Actif)
- **Objectif** : Migration intégrale en Rust pur (Edition 2024) et C/C++ natif sans aucun JS/HTML dans le binaire d'exécution, intégration du décompresseur bitwise ASH0/LZ77 et du bytecode CHANSVm.
- **Composants Clés** :
  1. `wii-menu-core` : Machine virtuelle bytecode `ChansVm` (`crates/menu-core/src/chans_vm.rs`), mathématiques de projection 16:9 / 4:3 et validation parentale multi-régions (`c_reverse_ipl.rs`).
  2. `wii-menu-runtime` : Décompresseur bitwise `ash.rs` (LZ77/Huffman), cryptographie AES-128-CBC, unpacker WAD/U8 (`wad.rs`), VFS NAND.
  3. `gecko-core` & `gecko-backend-wgpu` : JIT PowerPC 750CL Cranelift, MMIO Hollywood/Flipper, shaders TEV WESL/WGSL, compute texture decoder.
  4. `src/dolphin` : Core C++20 amont et VideoBackends D3D12/Vulkan interopérables via `dolphin-ffi`.
- **Binaire Produit** : `wii.exe` (21,4 Mo, profil release, zero warning, 12/12 tests unitaires passants).
- **Rapports Dédiés** : [docs/ARCHITECT-MIGRATION.md](file:///C:/src/wii/wii-os/docs/ARCHITECT-MIGRATION.md) et [docs/REVERSE-ENGINEERING.md](file:///C:/src/wii/wii-os/docs/REVERSE-ENGINEERING.md).
