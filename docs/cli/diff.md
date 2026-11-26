# `tholos diff` — aperçu du delta entre deux archives

```text
tholos diff <ANCIENNE> <NOUVELLE> [--json] [--all]
```

Compare deux archives **à partir de leurs tables seules**, sans décompresser un morceau (L-002) :
fichiers ajoutés, supprimés, modifiés, et morceaux de la nouvelle archive absents de l'ancienne,
c'est-à-dire ce qu'une mise à jour delta devrait transférer (thèse H6, écran « Mise à jour delta »).
La commande ne produit ni n'applique de fichier delta : c'est un aperçu chiffré.

Un morceau est identifié par le BLAKE3 de son contenu **décompressé** : deux archives compressées à
des niveaux zstd différents partagent leurs morceaux. En revanche, des paramètres de découpe
différents (`min / moyen / max`, voir `tholos info`) font diverger les morceaux : la commande
l'annonce sur la sortie d'erreur (`same_chunking: false` en JSON) et un fichier de plusieurs
morceaux peut alors apparaître modifié à contenu égal.

## Sortie texte

```text
+ README.md  (466 o)
~ diff.rs  (7936 → 7945 o, 1 morceau(x) nouveau(x), 7945 o)
- path.rs  (1498 o)
1 ajouté(s), 1 supprimé(s), 1 modifié(s), 11 inchangé(s)
à transférer   2 morceau(x), 2.7 Kio (2793 o) compressés (7.5 % des frames), 8.2 Kio (8411 o) décompressés
réutilisés     11 morceau(x), 33.5 Kio (34315 o)
métadonnées    1.9 Kio (1903 o)
racines        613e27b6…2dee41 → 0ca8e65b…4650bd
```

(Exemple réel : sources de `vault-core`, une ligne ajoutée à un fichier, un fichier supprimé, un
ajouté.) `+` ajouté, `-` supprimé, `~` modifié ; `--all` liste aussi les fichiers inchangés. Pour
un fichier modifié, « morceau(x) nouveau(x) » compte ses références absentes de l'ancienne archive.
« À transférer » porte sur les frames zstd de la nouvelle archive ; s'y ajoutent les métadonnées
(index, table des fichiers, table de hachage), hors parité. Deux archives de même racine donnent
`archives identiques (même racine) : rien à transférer`.

## Sortie JSON (`--json`)

Schéma `tholos-diff/1`. Les clés ne sont jamais renommées ; une évolution ajoute des clés ou change
de schéma.

| Clé | Sens |
|---|---|
| `schema` | `"tholos-diff/1"` |
| `old_root`, `new_root` | racines d'archive (hex) |
| `identical` | mêmes racines |
| `same_chunking` | mêmes paramètres de découpe |
| `counts.{added,removed,modified,unchanged}` | nombre de fichiers par nature de changement |
| `transfer.chunks` | morceaux uniques de la nouvelle archive absents de l'ancienne |
| `transfer.logical_bytes` | leurs octets décompressés |
| `transfer.compressed_bytes` | leurs octets de frames zstd |
| `transfer.metadata_bytes` | tables de la nouvelle archive |
| `transfer.percent` | `compressed_bytes` / frames de la nouvelle archive, en % |
| `reused.{chunks,compressed_bytes}` | morceaux déjà présents dans l'ancienne |
| `new_compressed_bytes` | frames zstd de toute la nouvelle archive |
| `files[]` | `path`, `change` (`added` / `removed` / `modified` / `unchanged`), `old_size`, `new_size` (`null` si absent), `new_chunk_refs`, `new_chunk_bytes` ; inchangés seulement avec `--all` |

## Codes de sortie

`0` si les deux archives s'ouvrent (qu'elles diffèrent ou non) ; `1` sinon, avec le chemin fautif
(« ouverture de … »). Pour l'intégrité des morceaux, voir [`tholos verify`](verify.md).
