# `tholos info` — résumer une archive

```text
tholos info <ARCHIVE> [--json]
```

Lit l'en-tête, le trailer et les tables, **sans décompresser un seul morceau** (L-002) : la commande
répond en temps constant vis-à-vis du contenu, quelle que soit la taille de l'archive. Elle ne
vérifie rien : pour l'intégrité, voir [`tholos verify`](verify.md).

## Sortie texte

```text
format          v1
codec           zstd 1.5.7 (niveau 15)
découpage       64.0 Kio (65536 o) / 128.0 Kio (131072 o) / 256.0 Kio (262144 o) (min / moyen / max)
fichiers        52
taille logique  38.3 Mio (40111707 o)
morceaux        266 uniques pour 278 références
déduplication   106.0 Kio (108568 o) économisés
frames zstd     37.7 Mio (39536324 o)
métadonnées     28.0 Kio (28622 o)
parité          2.0 Mio (2088816 o) (5.0 % de l'archive)
archive         39.7 Mio (41654082 o) (ratio 0.96×)
racine          a49dffb1654121d13dfec988ccf1195b50db1cbfe31a4536d0636ab9851386c9
```

(Exemple réel : sources de `vault-core` en double et `docs/`, dont des médias déjà compressés ; un
ratio inférieur à 1 signale un contenu incompressible, où seule la déduplication et la parité
pèsent.)

## Sortie JSON (`--json`)

Schéma `tholos-info/1`, destiné aux scripts, à la CI et à l'écran « Bibliothèque » du desktop. Les
clés ne sont jamais renommées ; une évolution ajoute des clés ou change de schéma.

| Clé | Sens |
|---|---|
| `schema` | `"tholos-info/1"` |
| `format_version` | version du format de l'archive |
| `codec.name`, `codec.level`, `codec.version` | codec, niveau et version de zstd à l'encodage |
| `chunking.min`, `.avg`, `.max` | paramètres du découpage CDC (octets) |
| `files` | nombre de fichiers |
| `logical_bytes` | somme des tailles de fichier |
| `chunk_refs`, `unique_chunks` | références de morceaux, morceaux uniques |
| `unique_bytes` | octets décompressés des morceaux uniques |
| `dedup_saved_bytes` | `logical_bytes - unique_bytes` |
| `compressed_bytes` | octets des frames zstd |
| `metadata_bytes` | index des morceaux, table des fichiers, table de hachage |
| `parity_bytes`, `parity_percent` | section de parité, et sa part de l'archive (%) |
| `archive_bytes` | taille du fichier `.vault` |
| `ratio` | `logical_bytes / archive_bytes` (2 décimales), `null` pour une archive vide |
| `archive_root` | racine de Merkle (hex), identité de la version |

Le calcul vit dans `vault-core` (`Archive::summary`, `ArchiveSummary`) ; le CLI ne fait que le
mettre en forme.
