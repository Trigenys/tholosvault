# ADR-0004 — Débit de lecture à travers le montage FUSE

- **Statut** : accepté ; complété le 2026-11-20 par les mesures du chantier (section finale)
- **Date** : 2026-11-20
- **Contexte** : la thèse H4 (`docs/recherche/`) annonce qu'un montage lit « à la vitesse du décodeur ».
  Le banc (`bench/`, thèse H4, PR #9) la mesure pour la première fois avec un pilote réel, sur
  `ubuntu-latest` (4 vCPU, `fuse3`, `fuser` 0.18 pur Rust, profil `ci`, exécution 44675746318) :

  | Métrique | Mesure | Seuil | Verdict |
  |---|---|---|---|
  | `h4.core_read_ratio` (cœur `vault-mount`, requêtes de 128 KiB ÷ lecture directe 1 MiB) | 0,99 | ≥ 0,9 | ✓ |
  | `h4.mount_content_ok` (BLAKE3 de chaque fichier lu par le noyau) | 1 | = 1 | ✓ |
  | `h4.mount_open_p99_over_blake3_mib` (ouverture par le noyau) | 0,69 | ≤ 1 | ✓ |
  | `h4.mount_read_ratio` (lecture directe ÷ lecture `std::fs` du montage, médiane de 3 passes) | **0,18** | ≥ 0,7 | **✗** |

  Le cœur sert les requêtes de la taille du noyau sans perte (0,99) ; la perte vient du trajet noyau →
  démon FUSE → `vault-core` : le montage lit **5,5 × plus lentement** que le décodeur. Le contenu est exact
  et l'ouverture est dans le seuil : seule la bande passante séquentielle réfute la thèse.
- **Cause présumée** (à confirmer par mesure, pas de pilote sur le poste de développement) : le démon sert
  les requêtes sur `n_threads` = parallélisme disponible et le noyau émet, avec une lecture anticipée de
  1 MiB, plusieurs requêtes de 128 KiB en parallèle pour le même fichier. `ChunkCache` (16 entrées, verrou
  unique) n'a pas de « vol unique » : des requêtes simultanées qui manquent le même morceau le décodent
  chacune en entier, puis se chassent du cache. Pour un morceau de ~1 MiB découpé en 8 requêtes, le travail
  de décodage peut être multiplié jusqu'à 8 (1/8 = 0,125, cohérent avec 0,18). Secondairement : une copie
  supplémentaire par requête (`Vec` alloué dans `read`, puis copie par `reply.data`).
- **Options considérées** :
  1. Relever le seuil à la mesure — refusé : un seuil ajusté à la mesure ne réfute plus rien.
  2. Garder le seuil bloquant — refusé : CI rouge sans action possible dans le chantier `vault-mount` (PR 1),
     qui livre le montage, pas son optimisation.
  3. Garder le seuil déclaré (≥ 0,7), passer `h4.mount_read_ratio` en non bloquante avec renvoi vers cet
     ADR, et ouvrir un chantier « lecture séquentielle à travers FUSE » : (a) décodage à vol unique par
     morceau dans `ChunkCache` (les requêtes concurrentes d'un même morceau attendent le décodeur en cours) ;
     (b) mesurer `--threads 1` contre le parallélisme disponible dans le banc ; (c) servir la réponse sans
     copie intermédiaire (tranche du morceau en cache) ; (d) si nécessaire, cache dimensionné par le
     parallélisme × taille de morceau. Réarmer `blocking: true` quand le banc passe en CI.
  4. Contourner le noyau (`FOPEN_DIRECT_IO`, lecture anticipée désactivée) — à mesurer dans le même
     chantier, mais perd le cache de pages, donc probablement pire pour les relectures.
- **Décision** : option 3. H4 est consignée **réfutée sur le débit séquentiel à travers le noyau**,
  **soutenue sur le cœur, l'exactitude du contenu et la latence d'ouverture**.
- **Conséquences** : `h4.mount_read_ratio` porte `"blocking": false` et `"adr"` vers ce fichier ; le rapport
  continue d'afficher la valeur et le ✗, et le verdict de H4 reste `refuted` tant que la mesure est sous le
  seuil. Remettre la métrique en bloquant ou changer le seuil passe par une PR qui cite les mesures.
- **Dérogation RAIDER** : D (non-régressif) — un seuil déclaré n'est pas appliqué par la CI ; bornée par le
  chantier de suivi et visible dans chaque rapport.

## Mesures du chantier « lecture séquentielle à travers FUSE » (2026-11-20)

Le chantier a appliqué les pistes (a) à (d) et mesuré la (e). La cause présumée est **infirmée** : le
cache multipliait bien le travail en théorie, mais ce n'est pas ce qui fait 0,18.

- **(a) Vol unique dans `ChunkCache`** : implémenté (`get_or_decode`, compteurs `hits`/`decodes`/`joined`,
  test de concurrence). Nouvelle métrique `h4.mount_decode_ratio` (décodages ÷ morceaux de l'archive) :
  **1,00** à travers le noyau, borne bloquante `h4.core_concurrent_decode_ratio` ≤ 1,05 (**1,00**). Chaque
  morceau est décodé une seule fois : le décodage n'est pas le goulot.
- **(b) `--threads 1` contre parallélisme** : `h4.mount_read_ratio_1thread` ≈ `h4.mount_read_ratio`
  (laboratoire : 0,09 contre 0,06 ; sonde : 0,45–0,67 contre 0,46–0,56 sur gros fichiers). Le noyau
  plafonne la lecture anticipée FUSE à 128 KiB (`VM_READAHEAD_PAGES`) : pour un lecteur séquentiel, les
  requêtes de 128 KiB sont **émises l'une après l'autre**, le parallélisme du démon ne sert à rien.
  `h4.mount_req_kib` (taille moyenne d'une requête du noyau) le confirme : 26 KiB en moyenne (fichiers
  `code` de 1 à 64 KiB), 128 KiB au plus.
- **(c) Réponse sans allocation** : tampon réutilisé par fil de service (`READ_BUF`). Gain marginal.
- **(d) Cache dimensionné** : 64 morceaux (16 MiB au plus). Sans effet mesurable une fois (a) en place.
- **(e) `FOPEN_DIRECT_IO` + `FUSE_ASYNC_DIO`** : mesuré par sonde en laboratoire puis **rejeté**. Gros
  fichiers : 0,46–0,61 (80 requêtes de 1 MiB au lieu de 256 de 128 KiB, gain nul) ; petits fichiers :
  0,04–0,05 au lieu de 0,08–0,10 (le mode direct rétablit `open`/`release` et ajoute une lecture de fin
  de fichier par fichier). Le code de l'option a été retiré.
- **Ajout : `FUSE_NO_OPEN_SUPPORT`** : quand le noyau l'annonce, `open` répond `ENOSYS` et le noyau cesse
  d'émettre `open`/`release` (cache de pages conservé). Sur petits fichiers : ~300–430 µs/fichier →
  ~200 µs/fichier (1,5 ×). Capacité `FUSE_PARALLEL_DIROPS` activée.

**Où est la perte.** Sonde en laboratoire (WSL2, Docker, noyau 6.18, 6 cœurs ; `vault-core` lit le même
contenu en 15 µs par morceau) :

| Corpus | Lecture directe ÷ montage | Coût dominant |
|---|---|---|
| 16 fichiers de 2 MiB (flux) | 0,45–0,67 | 256 allers-retours de 128 KiB à ~110 µs (plafond de lecture anticipée) |
| 2167 fichiers de 1 à 64 KiB | 0,04–0,10 | `lookup` + `read` + `getattr` par fichier, ~200 µs, pour ~15 µs de décodage |

Le banc CI mélange les deux (corpus `code` : 1107 fichiers de 1 à 64 KiB ; corpus `binary` : 4 KiB à
2 MiB) ; le corpus `code` domine l'agrégat. Les nouvelles métriques informatives `h4.mount_read_ratio_code`
et `h4.mount_read_ratio_binary` séparent les deux régimes. Mesures CI `ubuntu-latest` (4 vCPU, exécution
44726058404, PR #12), après le chantier :

| Métrique | Avant (PR #9) | Après (PR #12) | Seuil |
|---|---|---|---|
| `h4.core_read_ratio` | 0,99 | 0,96 | ≥ 0,9 ✓ |
| `h4.core_concurrent_decode_ratio` (nouvelle) | — | 1,00 | ≤ 1,05 ✓ (bloquante) |
| `h4.mount_decode_ratio` (nouvelle) | — | 1,00 | — |
| `h4.mount_open_p99_over_blake3_mib` | 0,69 | 0,62 | ≤ 1 ✓ |
| `h4.mount_read_ratio` | 0,18 | **0,21** | ≥ 0,7 ✗ (non bloquante) |
| `h4.mount_read_ratio_1thread` (nouvelle) | — | 0,23 | — |
| `h4.mount_read_ratio_code` (nouvelle) | — | 0,19 | — |
| `h4.mount_read_ratio_binary` (nouvelle) | — | 0,28 | — |
| `h4.mount_req_kib` (nouvelle) | — | 25,8 KiB | — |

Un seul fil de service fait aussi bien que quatre (0,23 contre 0,21) : les requêtes sont bien émises en
série. Le corpus `binary` lui-même (4 KiB à 2 MiB) reste à 0,28 : sa moitié de petits fichiers pèse autant
que ses gros. Le plancher est la **latence d'un aller-retour FUSE par requête**, une
propriété du trajet noyau ↔ espace utilisateur, pas du format ni du décodeur.

**Conséquence sur la décision.** Le seuil ≥ 0,7 sur l'agrégat n'est pas atteignable sans changer ce qui
est mesuré ; il reste déclaré et **non bloquant** (option 3, inchangée), le verdict H4 reste `refuted`
sur le débit à travers le noyau. Deux issues possibles, qui passent par une nouvelle PR citant les
mesures CI : (i) scinder la métrique — débit en flux sur `binary` (seuil à fixer après mesure CI) et
coût par fichier sur `code` (µs/fichier au-dessus du décodeur, seuil propre) ; (ii) garder H4 réfutée et
reformuler la thèse (« à la vitesse du décodeur » n'est vraie que pour le cœur, pas à travers FUSE).
Relever ou abaisser le seuil à la mesure reste refusé.
