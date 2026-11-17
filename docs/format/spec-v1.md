# Format tholosvault — version 1 (brouillon) : parité Reed-Solomon

Statut : **brouillon, non publié**. La v1 étend la v0 (`spec-v0.md`) d'une section de parité ; tout le reste
(en-tête, frames, index, tables, trailer) est inchangé. Le lecteur accepte les versions 0 et 1 ; l'écrivain
produit toujours la version 1 (champ « version de format » = 1), même quand la parité est désactivée.

## Objectif (L-004)

Reconstruire les zones corrompues d'une archive sans source externe, avec un budget de redondance choisi à
l'écriture (5 % par défaut). La parité couvre **les octets tels qu'écrits** — frames zstd adressées, index,
tables, en-tête — et non les fichiers en clair : elle compose donc avec la déduplication et les deltas, et une
métadonnée abîmée se répare comme une donnée.

## Agencement

```text
┌──────────────┬────────────────────────┬────────────────┬───────────────┬───────────────┬─────────────┬─────────┬─────────┐
│ en-tête (64) │ frames zstd, 1/morceau │ index morceaux │ table fichiers│ table hachage │ parité (v1) │ trailer │ trailer │
└──────────────┴────────────────────────┴────────────────┴───────────────┴───────────────┴─────────────┴─────────┴─────────┘
│◀────────────────────────────── zone protégée ──────────────────────────────────────────▶│
```

- **Zone protégée** : octets `[0, P)` où `P = hash_table_off + hash_table_len`. Elle commence à l'en-tête.
- **Section de parité** : octets `[P, P + parity_len)`, en clair. `parity_len = 0` signifie « sans parité ».
- **Trailer** : les octets 112..120, réservés en v0 (toujours nuls), portent `parity_len` (u64). Une archive v0
  est donc lue comme une archive v1 sans parité, sans cas particulier.

## Géométrie

Paramètres d'écriture : `percent` (budget, 0 = désactivé), `shard_size` S (4096 par défaut ; pair, 64 o à 1 MiB),
`group_max` (1024 par défaut ; 1 à 32768).

- La zone protégée est découpée en **N = max(1, ⌈P / S⌉)** shards de données de S octets ; le dernier est complété
  de zéros pour le calcul (son CRC porte sur les octets réels).
- **G = ⌈N / group_max⌉** groupes **entrelacés** : le shard `i` appartient au groupe `i mod G`, en position
  `⌊i / G⌋`. Le groupe `g` contient `k_g = ⌊N / G⌋ + (g < N mod G)` shards. Une perte contiguë de `R` shards ne
  touche chaque groupe qu'au plus `⌈R / G⌉` fois.
- Chaque groupe reçoit **m = ⌈k_max × percent / 100⌉ + 1** shards de parité, `k_max = ⌈N / G⌉`. Le shard
  supplémentaire absorbe le désalignement : une perte contiguë de `percent %` de la zone protégée est toujours
  réparable, quelle que soit sa position.
- Code : Reed-Solomon systématique sur GF(2^16) (crate `reed-solomon-simd` 3.1, algorithme Leopard-RS), appliqué
  groupe par groupe (`k_g` originaux, `m` parités). Toute combinaison de `≤ m` effacements (données ou parité)
  par groupe est reconstructible.
- La géométrie est entièrement dérivée de `(P, percent, S, group_max)` : le lecteur la recalcule et refuse une
  section dont l'en-tête la contredit.

Avec le budget par défaut, la section pèse environ `percent % + (G × S + 176 + tables)` de la zone protégée (≈ 5,6 %
pour 1 Mo).

## Section de parité

Entiers little-endian.

### En-tête (48)

| Offset | Taille | Champ |
|---|---|---|
| 0 | 8 | magique `THOLOSPR` |
| 8 | 4 | S, taille de shard |
| 12 | 4 | `group_max` |
| 16 | 4 | m, parités par groupe |
| 20 | 1 | `percent` |
| 21 | 1 | disposition : 0 = tables juste après l'en-tête ; 1 = copie du trailer (128) intercalée |
| 22 | 2 | réservé (0) |
| 24 | 8 | N, shards de données |
| 32 | 8 | P, longueur protégée |
| 40 | 4 | CRC32C des deux tables de CRC |
| 44 | 4 | CRC32C des octets 0..44 |

### Copie du trailer (128, disposition 1)

Copie octet pour octet du trailer (magique, champs, CRC32C), identique aux deux copies de queue. L'écrivain
produit toujours la disposition 1 ; la disposition 0 (sections antérieures) reste lue.

### Tables de CRC

- `u32 × N` : CRC32C de chaque shard de données (octets réels, sans complément).
- `u32 × (G × m)` : CRC32C de chaque shard de parité, dans l'ordre de stockage.

### Shards de parité (G × m × S)

La parité `j` du groupe `g` est stockée au slot `j × G + g` (entrelacée comme les données).

## Lecture, vérification, réparation

- **Ouverture** : inchangée tant qu'une copie de queue du trailer est valide ; la section n'est alors pas lue
  avant vérification ou réparation.
- **Balayage de secours** : si les deux copies de queue sont illisibles, le lecteur remonte depuis la fin du
  fichier (hors trailers) à la recherche du magique `THOLOSPR` ; un candidat n'est retenu que si son en-tête a
  un CRC valide, porte la disposition 1, et si la copie de trailer embarquée (CRC valide) désigne exactement
  cette position (`P = position`) et cette fin de section (`P + parity_len = longueur − 2 × 128`). Un faux
  magique dans les données est ainsi écarté sans collision CRC. L'archive s'ouvre et se lit normalement ; la
  réparation restaure les deux copies de queue. Coût : proportionnel à la taille de la section.
- **Bilan des dégâts** (`verify`, `scan_file`) : chaque shard de données et de parité est comparé à son CRC ;
  un morceau dont le hachage BLAKE3 est faux mais dont aucun shard n'est signalé voit tous ses shards marqués
  effacés (repli contre une collision CRC). Le bilan compte les effacements par groupe et déclare l'archive
  **réparable** si chaque groupe en a au plus `m`, **non réparable** sinon (premier groupe en dépassement
  rapporté). Sans section de parité, seule l'absence de dégât est « réparable ». Les copies du trailer
  illisibles sont comptées et restaurées depuis la copie valide.
- **Réparation** (`repair_file`, `Archive::repair`, `tholos repair`) : n'exige ni en-tête ni tables lisibles,
  seulement un trailer valide et une section auto-cohérente. Les originaux manquants sont décodés groupe par
  groupe ; les parités effacées sont recalculées. L'archive est copiée dans `<chemin>.tmp`, les octets
  reconstruits y sont écrits, puis le fichier est renommé : jamais de réécriture en place d'un fichier
  potentiellement mappé (I-001). Si rien n'est à réparer, aucune écriture (idempotent). Si les dégâts dépassent
  la parité, erreur explicite et aucune écriture.
- **Déterminisme** (L-001) : la section ne dépend que des octets protégés et des paramètres ; deux
  constructions identiques produisent la même archive, parité comprise.

## Limites connues

- Sans parité (v0 ou budget nul), les deux copies du trailer détruites rendent l'archive illisible.
- Une section de parité dont l'en-tête est abîmé perd aussi sa copie du trailer ; avec les tables de CRC
  abîmées, les dégâts sont détectés mais déclarés non réparables. La réplication de l'en-tête et des tables
  est un travail ultérieur.
- La parité protège un fichier ; la réparation par téléchargement de morceaux depuis une version voisine
  (distribution) reste à venir.
