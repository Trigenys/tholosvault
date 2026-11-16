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
