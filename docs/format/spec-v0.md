# Format tholosvault — version 0 (brouillon)

Statut : **brouillon, non publié**. Toute archive v0 peut être invalidée par la v1. Le format est figé à la
première publication du CLI ; jusque-là, cette page suit le code de `crates/vault-core`.

## Objectifs couverts par la v0

- Découpage content-defined (FastCDC 2020, 64 / 128 / 256 KiB par défaut) et déduplication par BLAKE3.
- Compression zstd par morceau, en frames autonomes ; version de zstd et niveau enregistrés (L-001).
- Arbre de Merkle BLAKE3 par fichier et racine d'archive.
- Métadonnées en clair, en fin de fichier, lisibles par `mmap` sans décompression (L-002).
- Trailer répliqué (deux copies, CRC32C).

Hors v0, prévus par le rapport et réservés dans l'en-tête : parité Reed-Solomon sur stripes entrelacées
(L-004), tier froid en groupes solides, dictionnaires zstd (champ `dict_id`), chiffrement (chunker à clé,
BLAKE3 keyed), recompression sémantique.

## Agencement

Entiers little-endian. Longueurs en octets.

```text
┌──────────────┬───────────────────────┬────────────────┬───────────────┬───────────────┬─────────┬─────────┐
│ en-tête (64) │ frames zstd, 1/morceau│ index morceaux │ table fichiers│ table hachage │ trailer │ trailer │
└──────────────┴───────────────────────┴────────────────┴───────────────┴───────────────┴─────────┴─────────┘
```

### En-tête (64)

| Offset | Taille | Champ | Valeur v0 |
|---|---|---|---|
| 0 | 8 | magique | `THOLOSVT` |
| 8 | 2 | version de format | 0 |
| 10 | 2 | drapeaux | 0 |
| 12 | 1 | codec | 1 = zstd |
| 13 | 1 | niveau (i8) | 15 par défaut |
| 14 | 2 | réservé | 0 |
| 16 | 4 | version du codec | `ZSTD_versionNumber()` |
| 20 | 4 | CDC min | 65 536 |
| 24 | 4 | CDC avg | 131 072 |
| 28 | 4 | CDC max | 262 144 |
| 32 | 32 | identifiant de dictionnaire | zéros = aucun |

### Index des morceaux (48 × n, ordre d'offset croissant)

`offset u64 · longueur compressée u32 · longueur claire u32 · BLAKE3 [32]`

### Table des fichiers (variable, chemins triés bytewise, uniques)

`longueur du chemin u16 · chemin UTF-8 (séparateur `/`, relatif) · taille u64 · racine Merkle [32] · nombre de morceaux u32 · indices u32 × n`

### Table de hachage (40 × n, triée par hachage)

`BLAKE3 [32] · indice de morceau u32 · réservé u32` — recherche dichotomique hash → morceau.

### Trailer (128, deux copies identiques en fin de fichier)

| Offset | Taille | Champ |
|---|---|---|
| 0 | 8 | magique `THOLOSTR` |
| 8 | 16 | offset, longueur de l'index des morceaux |
| 24 | 16 | offset, longueur de la table des fichiers |
| 40 | 16 | offset, longueur de la table de hachage |
| 56 | 8 | nombre de morceaux |
| 64 | 8 | nombre de fichiers |
| 72 | 32 | racine d'archive |
| 104 | 8 | longueur de la zone de données |
| 112 | 12 | réservé |
| 124 | 4 | CRC32C des octets 0..124 |

Le lecteur essaie la dernière copie puis l'avant-dernière ; aucune valide → erreur explicite.

## Hachages

- Identifiant de morceau : `BLAKE3(contenu clair)`.
- Racine de fichier : arbre binaire sur les identifiants de morceaux dans l'ordre ; nœud = `BLAKE3(0x01 ‖ gauche ‖ droite)`,
  nœud impair promu tel quel, feuille seule = racine, fichier vide = `BLAKE3(0x00)`.
- Racine d'archive : même arbre sur les feuilles `BLAKE3(0x02 ‖ len(chemin) u16 ‖ chemin ‖ taille u64 ‖ racine de fichier)`,
  dans l'ordre de la table des fichiers.

## Déterminisme

Deux constructions du même arbre de fichiers avec les mêmes options produisent des octets identiques : parcours trié,
aucun horodatage ni attribut hôte, zstd mono-thread à niveau fixe. Test : `deterministic_bytes_l001`.

## Vérification

`verify` décompresse chaque morceau et compare son BLAKE3, recalcule chaque racine de fichier à partir des
identifiants stockés, puis la racine d'archive. La v0 **détecte** la corruption ; la réparation arrive avec la parité.
