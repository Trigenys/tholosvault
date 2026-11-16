# VAULT — consignes projet

- Produit : format d'archive montable (CDC + zstd + Merkle BLAKE3 + parité RS), cœur Rust unique `vault-core`.
- Langue : documentation et messages en français ; identifiants de code en anglais.
- Stack retenue : Rust (core, CLI, mount, API Axum), Tauri 2 + React (desktop), Astro (landing),
  Next.js (console web), Flutter + flutter_rust_bridge (mobile, plus tard). Détail : `docs/produit/stack.md`.
- Monorepo : Cargo workspace (`crates/`) + pnpm workspace (`apps/`, `packages/`).
- Toute logique de format vit dans `vault-core` ; les apps sont des adaptateurs minces.
- Références : `docs/produit/cadrage-surfaces.md` (surfaces, priorités MVP/V1), `docs/recherche/` (rapport,
  thèses H1–H8). Toute entrée hors périmètre déclaré (écriture en place, GPU au décodage, neuronal) passe par un ADR dans `docs/adr/`.
- Standard RAIDER (voir consignes globales). Leçons : `docs/engineering/lessons-learned.md`.
- `PASSATION.md` : ajout seulement, entrée horodatée par `date`.
- Aucune mention d'assistant ou d'IA dans les commits, PR, code ou docs.
- CI : adopter `EagleFox31/appfactory-project-automation` (impact-aware CI, releases) avant d'écrire des workflows maison.
