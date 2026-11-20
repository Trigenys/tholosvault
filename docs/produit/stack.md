# VAULT — Choix de stack (reuse-first)

Version 0.1 · 2026-11-16 · Statut : proposition (décision D1 de `cadrage-surfaces.md`).

Principe : **un seul cœur Rust** (`vault-core`) porte tout le format ; chaque surface est un adaptateur mince.
Aucune surface ne réimplémente la découpe, la compression, le Merkle ou la parité. Recherche faite sur GitHub
(`gh search repos`) le 2026-11-16 ; les étoiles sont indicatives.

## 1. Cœur (format, CLI)

| Option | Pour | Contre |
|---|---|---|
| **Rust** | Écosystème complet déjà disponible : `fastcdc-rs` (★220), `blake3`, `zstd` (bindings zstd-safe, seekable), `reed-solomon-simd` (★102, GF(2¹⁶), O(n log n)) ou `reed-solomon-erasure` (★220, cherche mainteneurs), `fuser` (★1 308), `winfsp-rs` (★71), `uniffi` pour mobile ; binaire unique, pas de GC, `mmap` sans friction | Temps de compilation, courbe d'apprentissage |
| Go | Compilation rapide, `zstd-seekable-format-go` (★142), bon pour un service | FUSE/WinFsp moins mûrs, CGo pour SIMD, GC sur le chemin chaud du montage |

**Retenu : Rust.** C'est aussi la conclusion du rapport (MVP = `vault-core` + CLI Rust).

Briques (Adopt / Adapt) :

| Besoin | Brique | Décision |
|---|---|---|
| CDC | `nlfiedler/fastcdc-rs` | Adopt (v2020/2016, normalisation) ; Adapt plus tard pour un chunker AES à clé (H2, sécurité) |
| Hash | `BLAKE3-team/BLAKE3` crate `blake3` | Adopt |
| Compression | crate `zstd` + format seekable (skippable frames) ; `kylewlacy/zstd-framed` (★6) comme référence d'implémentation seekable | Adopt `zstd`, Learn `zstd-framed` |
| Parité | `AndersTrier/reed-solomon-simd` (actif, SIMD, GF(2¹⁶)) | Adopt ; `reed-solomon-erasure` en repli si API insuffisante |
| Montage Linux/macOS | `cberner/fuser` (0.18, pur Rust, `fusermount3` à l'exécution, MSRV 1.85) | Adopté dans `vault-mount` (feature `fuse`) |
| Montage Windows | `SnowflakePowered/winfsp-rs` (0.13) ; `antimatter-studios/winfsp-fs-skeleton` (★3, squelette service auto-mount) | Adopt `winfsp-rs` (adaptateur à venir derrière une feature `winfsp` de `vault-mount`), Learn le squelette |
| macOS sans kext | `KhaosT/FSKitSample` (★106), `debox-network/FSKitBridge` (★21, pont FSKit → backend Rust/C) | Learn ; FUSE-T en repli, comme le rapport |
| Algorithmes de layout | `mhx/dwarfs` (★2 635) : nilsimsa, segmentation, index non compressé | Adapt (réimplémentation Rust des idées, pas de dépendance C++) |
| Recompression | `preflate-rs`, `lepton_jpeg` / JXL jbrd | Adopt en V1 (E3) |

## 2. App desktop

| Option | Pour | Contre |
|---|---|---|
| **Tauri 2** | Backend Rust natif : `vault-core` appelé directement, sans IPC sérialisé lourd ; binaires de 5–15 Mo ; mises à jour signées intégrées (`tauri-plugin-updater`) ; plugins `fs`, `dialog`, `shell` officiels ; un seul langage côté système | Webview système (différences de rendu), écosystème plus jeune qu'Electron |
| Electron | Maturité, outillage, rendu identique partout | 150 Mo+, Node ↔ Rust via N-API ou processus séparé, deux runtimes à maintenir |

**Retenu : Tauri 2 + React + TypeScript** (UI), backend Rust = `vault-core` lié statiquement.
Les opérations longues (build, vérification, réparation) tournent en tâches Rust avec événements de progression vers l'UI.
Le montage lui-même reste un processus séparé (`vault-mount`), supervisé par l'app, pour survivre à sa fermeture.

## 3. Landing page

| Option | Pour | Contre |
|---|---|---|
| **Astro** | Zéro JS par défaut, content collections pour `/docs` et `/blog`, déploiement statique partout ; starters disponibles (`surjithctly/astroship` ★2 001, `cssninjaStudio/fresh` ★672) ; îlots React si besoin pour un comparatif interactif | Moins adapté à une app connectée (pas le besoin ici) |
| Next.js | Un seul framework landing + console, écosystème énorme | Plus lourd pour un site statique ; verrouille vers Vercel ou un serveur Node |

**Retenu : Astro** (statique, Cloudflare Pages ou VPS Caddy), design propre (pas de template appliqué tel quel ; `astroship` sert de référence de structure).

## 4. App web (console / compte)

| Option | Pour | Contre |
|---|---|---|
| **Next.js (App Router) + Better Auth + Drizzle + PostgreSQL** | Même langage que l'UI desktop (React/TS), auth auto-hébergée (magic link, passkeys), ORM typé, déployable en conteneur sur le VPS | Framework lourd pour une V1 petite |
| Rust (Axum) + HTMX | Un seul langage, réutilise `vault-core` pour vérifier les index côté serveur, très léger en RAM (contrainte VPS) | Deux équipes de compétences si l'UI desktop est en React ; moins d'écosystème UI |

**Retenu : API Rust (Axum) + front Next.js.** L'API doit vérifier des racines Merkle et lire des index : `vault-core` y est réutilisé tel quel.
Le front reste en React/TS pour partager des composants avec l'app desktop (`packages/ui`). Postgres pour les métadonnées ; S3 compatible plus tard pour les morceaux.
Si la V1 doit être minimale, l'API Axum peut servir des pages HTMX et le front Next.js arrive après : décision au moment de la V1.

## 5. App mobile

| Option | Pour | Contre |
|---|---|---|
| **Flutter + `flutter_rust_bridge`** (★5 492, actif) | Pont Rust ↔ Dart généré, un code UI pour Android/iOS, bonnes performances pour l'aperçu de fichiers, déjà pratiqué par Trigenys (agents `dart analyze`, `flutter test`) | Écosystème Dart distinct de React |
| React Native + `uniffi` | Partage de l'habitude React/TS avec desktop et web | Pont Rust moins intégré (`uniffi-bindgen-react-native` jeune), new architecture encore mouvante |

**Retenu : Flutter + flutter_rust_bridge**, quand la surface mobile sera lancée (Plus tard). `vault-core` doit rester `no_std`-friendly sur son chemin de lecture pour compiler en bibliothèque Android/iOS.

## 6. Monorepo et outillage

| Option | Pour | Contre |
|---|---|---|
| **Cargo workspace + pnpm workspace** (modèle `CyanFroste/rust-typescript-workspace`) | Deux gestionnaires natifs, pas de couche supplémentaire ; Turborepo optionnel pour le cache JS | Deux lockfiles |
| Nx / Bazel | Graphe unifié | Sur-outillage pour l'équipe actuelle |

**Retenu : Cargo workspace + pnpm workspace**, CI change-scoped via `appfactory-project-automation` (impact-aware CI, releases Release Please, variante Tauri), conformément à la règle reuse-first du registre.

## 7. Récapitulatif

| Surface | Stack retenue | Alternative écartée |
|---|---|---|
| Cœur / CLI | Rust (`fastcdc-rs`, `blake3`, `zstd`, `reed-solomon-simd`, `fuser`, `winfsp-rs`) | Go |
| Desktop | Tauri 2 + React/TS, `vault-core` lié | Electron |
| Landing | Astro statique | Next.js |
| Web console | API Axum (Rust) + Next.js | Tout Next.js / tout Axum+HTMX |
| Mobile | Flutter + flutter_rust_bridge | React Native + uniffi |
| Monorepo | Cargo + pnpm workspaces, CI appfactory | Nx / Bazel |
