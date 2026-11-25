# tholosvault

**Archives montables, vérifiables et dédupliquées.** Un format ouvert et un CLI, `tholos`, by Trigenys Group.

[![CI](https://github.com/Trigenys/tholosvault/actions/workflows/ci.yml/badge.svg)](https://github.com/Trigenys/tholosvault/actions/workflows/ci.yml)
[![Licence Apache-2.0](https://img.shields.io/badge/licence-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](rust-toolchain.toml)

Pour lire un seul fichier dans une archive classique, il faut tout extraire : un quart d'heure, un disque
plein. Avec tholosvault, on branche l'archive, on ne l'ouvre pas. Chaque morceau se lit seul, chaque octet
est vérifiable, et une archive abîmée se répare sans retéléchargement.

## Pourquoi tholosvault

| Promesse | Comment |
|---|---|
| **Lire sans extraire** | Chaque morceau est compressé à part et localisé par un index en clair lisible par mmap : un accès coûte une décompression de 128 KiB, pas de l'archive entière. |
| **Dédupliquer sans configurer** | La découpe par contenu (FastCDC) coupe là où les données le décident. Deux versions d'un même dossier partagent leurs morceaux identiques. |
| **Prouver l'intégrité** | Une empreinte BLAKE3 par morceau, une racine de Merkle par fichier et pour l'archive. Un octet abîmé est localisé au morceau près. |
| **Réparer sur place** | 5 % de parité Reed-Solomon par défaut : les morceaux abîmés sont reconstruits depuis l'archive elle-même. |
| **Reproduire au bit près** | Même entrée, même archive. Deux `pack` du même dossier donnent le même fichier, donc la même racine. |

Cibles prioritaires : bibliothèques de jeux solo, indé et rétro (mods, émulateurs), artefacts de CI et arbres
de sources, archives documentaires et photothèques.

## Ce qui marche aujourd'hui

Le cœur Rust `vault-core` et le CLI `tholos` implémentent le format v1 :

- `tholos pack` : découpe FastCDC (64–256 KiB), zstd par morceau, Merkle BLAKE3, parité Reed-Solomon, trailer
  répliqué protégé par CRC32C ; écriture atomique (`<archive>.tholos-tmp` puis renommage : un échec laisse
  l'archive précédente intacte) et idempotente (la cible n'est jamais archivée elle-même, même placée dans
  le dossier source) ;
- `tholos list` et `tholos verify` : lecture des métadonnées sans décompression, vérification morceau par
  morceau avec rapport précis ;
- `tholos cat` : lecture d'une plage d'octets d'un fichier (`--offset`, `--len`) sans extraire
  l'archive, en ne décompressant que les morceaux couverts.
- `tholos repair` : reconstruction des morceaux abîmés par la parité, validée par des tests d'injection de
  corruptions ;
- `tholos extract` : extraction sûre (aucune sortie hors du dossier cible).

Testé en CI sur Linux et Windows : format, clippy en mode strict, tests unitaires et d'intégration, build release.

## Démarrer

Prérequis : Rust stable (voir `rust-toolchain.toml`).

```bash
cargo build --release -p vault-cli
./target/release/tholos pack ./mon-dossier mon-archive.tholos
./target/release/tholos list mon-archive.tholos
./target/release/tholos verify mon-archive.tholos
./target/release/tholos repair mon-archive.tholos
./target/release/tholos extract mon-archive.tholos ./restauré
```

Options utiles : `--level` (niveau zstd, 15 par défaut), `--parity` (pourcentage de parité, 5 par défaut).

La parité est encodée en parallèle (fonctionnalité `parallel` de `vault-core`, activée par défaut) ;
`cargo build -p vault-core --no-default-features` donne un cœur mono-fil à la sortie identique.

## Format

Le format est documenté et stable par version : [`docs/format/spec-v0.md`](docs/format/spec-v0.md)
(détection) et [`docs/format/spec-v1.md`](docs/format/spec-v1.md) (parité et réparation). Une archive v0 se
lit comme une archive v1 sans parité.

```text
[en-tête 64 o][frames zstd…][index morceaux][table fichiers][table hachage][parité][trailer 128 o ×2]
```

Toute la logique de format vit dans `vault-core`. Les applications sont des adaptateurs minces.

## Feuille de route

| Étape | Contenu |
|---|---|
| **v1 (actuel)** | Format, CLI, parité et réparation, CI Linux et Windows |
| **Prochaine** | Bench public H1–H8 (corpus, protocole, seuils bloquants), lecture par plage |
| **Montage** | `vault-mount` : FUSE (Linux), WinFsp (Windows), FSKit ou macFUSE (macOS) |
| **Delta** | Mise à jour entre deux versions en ne transférant que les morceaux changés |
| **Surfaces** | Application desktop (Tauri 2), console web, site |

Hors périmètre déclaré : écriture en place au montage, décodage GPU, compression neuronale.
Tout ajout passe par un ADR dans [`docs/adr/`](docs/adr/).

## Documentation

| Document | Contenu |
|---|---|
| [docs/produit/cadrage-surfaces.md](docs/produit/cadrage-surfaces.md) | Surfaces, parcours, matrice de fonctionnalités |
| [docs/produit/stack.md](docs/produit/stack.md) | Choix de stack par surface |
| [docs/recherche/](docs/recherche/) | Rapport *Archives montables et compression prospective* : état de l'art, thèses H1–H8 |
| [docs/engineering/lessons-learned.md](docs/engineering/lessons-learned.md) | Mémoire des incidents et leçons |
| [docs/adr/](docs/adr/) | Décisions d'architecture |

## Monorepo

```
crates/
  vault-core/      format : CDC, zstd, Merkle, parité, index (bibliothèque Rust)
  vault-cli/       binaire tholos
  vault-mount/     adaptateurs FUSE / WinFsp / FSKit (à venir)
apps/              desktop, landing, console web (à venir)
bench/             banc des thèses H1–H8 : corpus synthétique, seuils bloquants, profil `ci` en CI
docs/
```

## Licence

Open core ([ADR-0001](docs/adr/0001-modele-open-core-apache.md)) : le format, `vault-core`, `vault-cli` et `vault-mount` sont sous
[Apache-2.0](LICENSE). Les applications et la console sont propriétaires.

---

tholosvault, by [Trigenys Group](https://trigenys.com).
