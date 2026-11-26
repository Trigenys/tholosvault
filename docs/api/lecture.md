# API en lecture seule (`tholos-api`, crate `vault-api`)

```text
tholos-api <RÉPERTOIRE> [--listen 127.0.0.1:7070] [--verify-slots 2]
```

Sert en HTTP les archives `.vault` d'un répertoire (non récursif), **sans jamais écrire** : catalogue,
bilan, liste des fichiers, contenu d'un fichier par plage, vérification. Toute la logique de format
est celle de `vault-core` ; l'API n'est qu'un adaptateur (Axum). Première brique de la console web
et de la page de partage `/s/<id>` (cadrage §5), utilisable dès maintenant pour servir des
archives sur un réseau local ou derrière un proxy inverse.

**Sécurité.** Aucune authentification : l'écoute est locale par défaut. Pour exposer le service,
le placer derrière un proxy inverse qui authentifie et chiffre (Caddy, décision D4 du cadrage). Un
nom d'archive est un simple nom de fichier `*.vault` du répertoire servi : séparateur, `..`,
fichier caché ou autre extension donnent 400. Les vérifications (coûteuses en CPU) sont limitées
à `--verify-slots` simultanées ; au-delà, 503 avec `Retry-After`.

**Mise à jour d'une archive servie.** `tholos pack` écrit un temporaire puis renomme (I-007). Le
service ouvre l'archive à chaque requête (en-tête, trailer et tables, sans décompression : environ
8 ms pour 20 000 fichiers, démarrage de processus compris) et la referme avec la réponse. Il n'y a
pas de cache pouvant servir une version périmée : la nouvelle version est servie dès la requête
suivante, et une réponse en cours continue sur l'ancienne. Ne jamais réécrire une archive servie en place.

## Routes

Les réponses JSON portent `"schema": "tholos-api/1"` ; les clés ne sont jamais renommées, une
évolution ajoute des clés ou change de schéma. Une erreur répond `{"error": "<message actionnable>"}`.

| Méthode | Chemin | Réponse |
|---|---|---|
| GET | `/health` | `{"status": "ok"}` |
| GET | `/v1/archives` | `archives[]` : bilan de chaque archive ; une archive illisible y figure avec `error` sans masquer les autres |
| GET | `/v1/archives/{nom}` | bilan, sans décompression |
| GET | `/v1/archives/{nom}/files` | `files[]` : `path`, `size`, `root` (racine de Merkle du fichier, hex) |
| GET, HEAD | `/v1/archives/{nom}/files/{chemin}` | contenu du fichier (chemin tel que `tholos list` l'affiche, encodé en pourcentage) |
| GET | `/v1/archives/{nom}/verify` | vérification complète ; `?quick=true&sample=N` pour la vérification rapide |

Bilan d'une archive : `name`, `format_version`, `files`, `logical_bytes`, `unique_chunks`,
`dedup_saved_bytes`, `compressed_bytes`, `parity_bytes`, `archive_bytes`, `ratio`, `archive_root`
(mêmes définitions que [`tholos info`](../cli/info.md)).

Vérification : `mode` (`full` / `quick`), `ok` (contenu intègre), `needs_repair`, `repairable`,
`chunks_checked`, `chunks_total`, `bad_chunks[]`, `bad_files[]`, `archive_root_ok`,
`bad_data_shards`, `bad_parity_shards`, `bad_trailer_copies`, `archive_root` (mêmes définitions
que [`tholos verify`](../cli/verify.md)). La réparation reste `tholos repair`, hors API.

## Contenu d'un fichier

- `Range: bytes=a-b`, `bytes=a-`, `bytes=-n` : 206 avec `Content-Range`. Plage hors du fichier :
  416 avec `Content-Range: bytes */<taille>`. Plusieurs plages ou plage mal formée : ignorées
  (RFC 9110), fichier entier en 200.
- `ETag` : racine de Merkle du fichier (validateur fort par construction) ;
  `X-Tholos-Archive-Root` : racine de l'archive.
- Seuls les morceaux couvrant la plage sont décompressés, chacun vérifié par BLAKE3, par blocs de
  1 Mio (mémoire bornée). Un morceau corrompu en tête de plage donne 422 avec la marche à suivre ;
  plus loin, l'en-tête est déjà parti et la réponse est coupée (le client voit une longueur
  incomplète et ne doit pas utiliser le contenu).

## Exemples

```sh
tholos-api ./archives &
curl -s localhost:7070/v1/archives
curl -s localhost:7070/v1/archives/jeux.vault/files
curl -s -r 0-1023 localhost:7070/v1/archives/jeux.vault/files/sous%20dossier/save.dat -o debut.bin
curl -s 'localhost:7070/v1/archives/jeux.vault/verify?quick=true'
```
