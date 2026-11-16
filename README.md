# VAULT

Format d'archive **montable** : découpe par contenu (CDC), compression zstd par morceau avec dictionnaires,
arbre de Merkle BLAKE3, 3–5 % de parité Reed-Solomon, index lisible par mmap. L'archive se monte comme un
disque (FUSE, WinFsp, FSKit) : lecture d'un morceau en ~0,1 ms sans extraction, réparation sans
retéléchargement, mise à jour en delta.

État : **cadrage**. Aucun code applicatif pour l'instant ; ce dépôt contient le squelette du monorepo et la
documentation produit et technique.

## Documents

| Document | Contenu |
|---|---|
| [docs/produit/cadrage-surfaces.md](docs/produit/cadrage-surfaces.md) | Surfaces (landing, web, desktop, mobile, CLI), pages, parcours, matrice fonctionnalités, décisions à trancher |
| [docs/produit/stack.md](docs/produit/stack.md) | Comparaison et choix de stack par surface, briques réutilisées |
| [docs/recherche/](docs/recherche/) | Rapport *Archives montables et compression prospective* : état de l'art, thèses H1–H8, innovations E1–E5, protocole MVP |
| [docs/engineering/lessons-learned.md](docs/engineering/lessons-learned.md) | Mémoire des échecs et leçons |
| [docs/adr/](docs/adr/) | Décisions d'architecture (ADR-0001 : open core, Apache-2.0) |
| [docs/produit/nom.md](docs/produit/nom.md) | Shortlist de noms publics vérifiée (D11) |
| [apps/landing/video/](apps/landing/video/) | Source HyperFrames de la vidéo « VAULT, le principe » ; rendu final dans `docs/media/` |
| [PASSATION.md](PASSATION.md) | Journal de passation (ajout seulement) |

## Arborescence prévue

```
crates/
  vault-core/      format : CDC, zstd, Merkle, parité, index (bibliothèque Rust)
  vault-cli/       binaire `vault` : create, mount, verify, repair, delta, ls, cat
  vault-mount/     adaptateurs FUSE / WinFsp / FSKit / ProjFS
  vault-api/       API de la console web (Axum), réutilise vault-core
apps/
  desktop/         Tauri 2 + React
  landing/         Astro
  web/             console Next.js
  mobile/          Flutter + flutter_rust_bridge (plus tard)
packages/
  ui/              composants React partagés desktop / web
bench/             corpus, scripts et résultats du protocole H1–H8
docs/
```

## Prérequis (quand le code arrivera)

Rust stable, pnpm 9+, Node 22+. Montage : fuse3 (Linux), WinFsp (Windows), macFUSE ou FSKit (macOS).

## Licence

Open core (ADR-0001) : format, `vault-core`, `vault-cli` et `vault-mount` sous [Apache-2.0](LICENSE) ; apps et console propriétaires.
