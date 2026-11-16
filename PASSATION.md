# PASSATION — VAULT

Journal en ajout seulement. Chaque session ajoute une entrée datée en fin de fichier.

## 2026-11-16 12:08 — Cadrage produit et initialisation du dépôt

- Dépôt créé dans `Projets/vault` : squelette monorepo (crates/, apps/, packages/, bench/, docs/), README, CLAUDE.md,
  .gitignore, .editorconfig, modèle ADR, leçons héritées de la recherche (L-001 à L-004).
- Rapport de recherche copié dans `docs/recherche/` (md, pdf, docx).
- `docs/produit/cadrage-surfaces.md` : 5 surfaces (CLI, desktop, landing, web, mobile), pages et parcours, matrice
  surfaces × fonctionnalités, 11 décisions à trancher (D1–D11).
- `docs/produit/stack.md` : Rust core, Tauri 2 + React, Astro, Axum + Next.js, Flutter + flutter_rust_bridge ; briques
  GitHub identifiées (fastcdc-rs, reed-solomon-simd, fuser, winfsp-rs, FSKitSample, dwarfs en Adapt).
- Aucun code applicatif. Prochaines étapes : trancher D1–D11 (D6 licence/modèle et D11 nom avant la landing), adopter
  appfactory-project-automation pour la CI, créer `vault-core` et le protocole `bench/` (H1–H8).

## 2026-11-16 12:22 — Décisions D6/D7, shortlist de noms, rangement des fichiers de session

- D6 et D7 tranchés : open core + Apache-2.0 (`docs/adr/0001-modele-open-core-apache.md`, `LICENSE`). Cadrage et README mis à jour.
- D11 : shortlist vérifiée dans `docs/produit/nom.md` (reco : chunkvault ou vaultfs) ; VAULT reste le nom de travail.
- Fichiers de la session vidéo rangés : source HyperFrames → `apps/landing/video/` (sans rendus ni snapshots, ignorés par git),
  rendu final v5 → `docs/media/VAULT-principe-v5.mp4` (38 Mo, suivi par git : passer à Git LFS si d'autres médias arrivent),
  notes de recherche → `docs/recherche/notes/`. Rapport en doublon et anciens rendus supprimés.
- Reste : D1–D5, D8–D10 ouverts ; CI appfactory ; `vault-core` ; nom public à acheter.

## 2026-11-16 12:29 — Nom public tholosvault

- D11 tranché : nom public **tholosvault**, « by Trigenys », CLI `tholos` (ADR-0002). VAULT reste le nom de travail interne.
- `docs/produit/nom.md` : second tour de vérification (12 noms gréco-latins) ; tholosvault entièrement libre, .com inclus.
- À faire : acheter tholosvault.com/.io/.dev, réserver org GitHub + crate + npm, contrôle de marque.

## 2026-11-16 12:32 — Signature « by Trigenys Group »

- Correction : l'éditeur et la signature sont **Trigenys Group** (pas « Trigenys » seul) dans README, CLAUDE.md, cadrage, nom.md, ADR-0002.

## 2026-11-16 12:34 — Domaine : sous-domaine de trigenys.com

- Pas d'achat de tholosvault.com : landing sur `tholosvault.trigenys.com`, console sur `console.tholosvault.trigenys.com` (DNS Spaceship, CNAME à créer au déploiement). nom.md et ADR-0002 mis à jour.
