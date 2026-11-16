# Leçons apprises — VAULT

Mémoire des échecs et quasi-incidents du projet. Chaque entrée : contexte, cause racine, classification,
résolution, prévention vérifiable, leçon généralisée. À consulter avant tout travail proche d'un incident connu.

## Leçons héritées de la recherche (avant tout code)

### L-001 — Le format doit figer la version du codec par morceau
- **Contexte** : le rapport rappelle le détour de Google Play : un même contenu clair compressé par deux versions
  d'un outil produit des octets différents, ce qui détruit la dédup et les deltas entre versions d'archive.
- **Classification** : conception / durabilité.
- **Prévention** : la version de zstd (ou le hash du codec) et l'identifiant du dictionnaire sont enregistrés dans
  l'en-tête et testés par un test de non-régression « même entrée, mêmes octets » entre deux builds.
- **Leçon** : le déterminisme de l'encodage est un contrat du format, pas un détail d'implémentation.

### L-002 — Les métadonnées se lisent sans décompression
- **Contexte** : DwarFS fait tomber l'init FUSE de 61 ms à 2,9 ms avec un index non compressé ; CoFS mesure +73–86 %
  de lookup dès que les métadonnées passent par une couche supplémentaire (H3).
- **Classification** : performance / architecture.
- **Prévention** : index et table hash → offset en clair, lisibles par mmap ; benchmark de montage et de lookup
  dans `bench/` avec seuil bloquant. **En place** : `h3.metadata_plaintext`, `h3.open_scaling`,
  `h3.find_p99_over_blake3_mib`, `h3.chunks_per_read4k.max` (`bench/config.json`, profil `ci` en CI).
- **Leçon** : le surcoût d'un FS user-mode est par opération ; on optimise le nombre d'I/O par lookup, pas les octets.

### L-003 — Ne pas vendre le montage par driver aux joueurs compétitifs
- **Contexte** : aucun ticket primaire ne confirme un blocage WinFsp par EAC / BattlEye / Vanguard, mais tous les
  indices convergent (allowlisting de drivers, refus des volumes non NTFS). Le risque porté par l'utilisateur est un ban.
- **Classification** : produit / sécurité utilisateur.
- **Prévention** : avertissement explicite à l'onboarding desktop ; voie ProjFS + WOF proposée quand un exécutable
  de jeu est détecté ; matrice H7 à publier avant toute communication « jouer depuis l'archive ».
- **Leçon** : un risque non documenté mais non réductible par le produit se traite par le périmètre, pas par un disclaimer.

### L-004 — La parité se calcule sur les morceaux adressés, pas sur les octets du fichier
- **Contexte** : PAR2 (modèle fichier) ne compose ni avec la dédup ni avec les deltas ; une perte contiguë peut
  toucher plusieurs shards d'un même groupe si les stripes ne sont pas entrelacées (H5, E2).
- **Classification** : conception du format.
- **Prévention** : test d'injection de corruptions (aléatoire 1–5 %, contiguë 1–10 %, trailer) dans la suite de tests
  de `vault-core` avant toute publication du format. **En place depuis le format v1** (`tests/parity_v1.rs`) : la
  parité couvre les octets tels qu'écrits (frames, index, tables, en-tête), stripes entrelacées, un shard de parité
  supplémentaire par groupe pour absorber le désalignement d'une perte contiguë ; la réparation écrit une copie
  temporaire puis renomme (I-001).
- **Leçon** : la réparation doit être composable avec la distribution (réparer = télécharger les morceaux manquants d'une version voisine).

## Incidents du projet

### I-001 — Un mmap ouvert verrouille l'archive sous Windows (near miss, 2026-11-16)
- **Contexte** : le test `trailer_replica_survives_last_copy_corruption` ouvrait une archive (`Archive::open`, mmap
  lecture seule) puis réécrivait le fichier sans l'avoir fermée. Vert sous Linux, `PermissionDenied` sous Windows :
  un fichier mappé n'est pas réécrivable tant que la vue existe.
- **Cause racine** : contrat implicite « archive immuable » non formalisé côté outillage ; les tests supposaient la
  sémantique POSIX.
- **Classification** : portabilité / tests.
- **Résolution** : `drop(ar)` explicite avant toute réécriture dans les tests.
- **Prévention** : la CI exécute la suite sur `ubuntu-latest` et `windows-latest` ; toute opération qui remplace une
  archive en place (future `repack`, `compact`) doit écrire dans un fichier temporaire puis renommer, jamais réécrire
  un fichier potentiellement mappé.
- **Leçon** : un format fondé sur mmap doit documenter que l'archive est verrouillée tant qu'un lecteur est ouvert, et
  les tests doivent tourner sur l'OS cible le plus restrictif, pas seulement sur celui du développeur.

### I-002 — Deux corpus de doublons identiques pour deux taux (near miss, 2026-11-16)
- **Contexte** : première exécution du banc H2, les corpus `dup10` et `dup20` avaient le même digest et
  `h2.penalty_pct.d10` = `h2.penalty_pct.d20` = −38 %. Le générateur tirait des copies de binaires de
  1–8 MiB sur une base de 8 MiB : une seule copie éditée dépassait le budget des deux taux, et le taux réel
  de doublons était ≈ 50 %.
- **Cause racine** : le générateur ne vérifiait pas que le corpus produit respectait le paramètre déclaré ;
  la mesure aurait soutenu la thèse sur un corpus qui ne la testait pas.
- **Classification** : validité de mesure / banc.
- **Résolution** : tailles des binaires proportionnelles à la base, règle « ne copier que ce qui tient dans
  le budget restant », au moins une édition par copie éditée.
- **Prévention** : bande de validité dans `h2` (taux réel à ±2 points, copies éditées présentes au taux où
  le rappel est mesuré) → sortie 2 ; test `dup_rate_is_respected` sur deux tailles de base.
- **Leçon** : un banc valide son corpus contre ses paramètres déclarés avant de mesurer ; deux corpus
  censés différer et de même digest doivent faire échouer l'exécution, pas produire une ligne de plus.

## Réfutations consignées par le banc

### B-001 — H5 : coût de construction de la parité (2026-11-16)
- **Mesure** : 1,0–1,3 s/GB mono-thread, 5–10 × BLAKE3, contre ≤ 0,3 s/GB annoncé ; indépendant de la
  géométrie des shards et des groupes. Réparation : 100 % sur les quatre scénarios.
- **Décision** : [ADR-0003](../adr/0003-cout-construction-parite.md) — métrique non bloquante avec renvoi
  d'ADR, chantier « encodage parallèle par groupe » pour réarmer le seuil.
- **Leçon** : un seuil ne se relâche jamais pour faire passer la CI ; une réfutation se consigne (ADR +
  ici) et la métrique garde son seuil déclaré, visible dans chaque rapport.
- **Suivi 2026-11-16** : encodage parallèle par tranches de colonnes livré (ADR-0003, « Suivi ») : 0,33–0,39
  s/GB à ≥ 4 fils (÷ 3,2–3,7), sortie identique. Seuil **non réarmé** : rapport 2,3–3,5 à 4 fils, dominé par
  la variance de la calibration BLAKE3 (0,11–0,18 s/GB sur le même hôte). Leçon ajoutée : un seuil relatif
  n'est pas plus stable que son dénominateur ; calibrer sur le même volume et le même régime mémoire que la
  mesure, sinon le rapport mesure la fréquence turbo.
