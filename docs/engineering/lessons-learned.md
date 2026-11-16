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
  dans `bench/` avec seuil bloquant.
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
