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

### I-003 — Une mesure que l'hôte ne peut pas faire ne doit pas disparaître en silence (règle, 2026-11-19)

- **Contexte** : H4 mesure un montage réel ; les postes Windows et la matrice CI n'ont pas tous un
  pilote. Première tentation : ne produire que les métriques possibles et laisser le contrat
  « métrique déclarée non produite = erreur » être contourné par profil.
- **Risque** : un hôte sans pilote aurait affiché H4 `supported` sur la seule mesure du cœur, et la CI
  aurait pu perdre le pilote (image, paquet `fuse3`) sans que rien ne rougisse.
- **Règle** : une métrique déclare la capacité qu'elle exige (`requires`). Capacité absente → métrique
  sautée, raison rapportée, verdict `incomplete`. Capacité présente → la métrique est due comme les
  autres. Métrique produite sans la capacité → erreur. La CI passe `--require-env mount` : l'absence du
  pilote est une erreur de configuration (code 2), pas un rapport incomplet. Pilote présent mais non fonctionnel (`fusermount3` absent) : `MountError::DriverUnavailable`, même code.
- **Prévention vérifiable** : tests `missing_capability_skips_metrics_and_marks_incomplete`,
  `required_metric_produced_without_capability_is_an_error`, `capability_present_requires_the_metrics`,
  `unknown_capability_is_rejected` dans `bench/src/report.rs` ; étape CI `fuse3` + `--require-env`.
- **Leçon générale** : un banc qui conclut doit distinguer « mesuré et conforme » de « non mesurable
  ici », et l'environnement de référence doit exiger la mesure plutôt que la tolérer.

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
- **Suivi 2026-11-17** : dénominateur recalé sur BLAKE3 du même buffer protégé, mesuré au même instant
  (`h5.blake3_s_per_gb` rapporté) : rapport 1,82–2,06 sur 5 exécutions à 4 fils (± 6 %). Seuil **réarmé**
  (`blocking: true`, borne ≤ 3 inchangée) ; dérogation RAIDER (D) de l'ADR-0003 levée.

### B-002 — H4 : débit de lecture à travers le montage FUSE (2026-11-20)
- **Mesure** (première exécution CI avec pilote, `ubuntu-latest`, 4 vCPU) : cœur 0,99 (seuil ≥ 0,9),
  contenu exact, ouverture 0,69 × BLAKE3/MiB (seuil ≤ 1), mais `h4.mount_read_ratio` **0,18** pour un seuil
  ≥ 0,7 : le montage lit 5,5 × plus lentement que le décodeur.
- **Décision** : [ADR-0004](../adr/0004-debit-lecture-montage-fuse.md) — métrique non bloquante avec renvoi
  d'ADR, chantier « lecture séquentielle à travers FUSE » (décodage à vol unique dans `ChunkCache`, mesure
  `--threads 1`, réponse sans copie) pour réarmer le seuil.
- **Leçon** : un cœur mesuré « sans perte » en requêtes de la taille du noyau ne prouve rien sur le
  montage : la concurrence du noyau (lecture anticipée, plusieurs fils) change le profil d'accès. Mesurer
  le cœur **sous la même concurrence** que le pilote avant d'annoncer un ratio ; un cache sans vol unique
  multiplie le travail au lieu de l'économiser dès que deux requêtes manquent le même morceau.
- **Suivi** (2026-11-20, chantier « lecture séquentielle à travers FUSE ») : vol unique posé et mesuré
  (`h4.mount_decode_ratio` = 1,00, borne bloquante ≤ 1,05 sur le cœur concurrent) — la cause présumée
  n'était **pas** la bonne : chaque morceau n'était déjà décodé qu'une fois en pratique. La perte est la
  latence d'un aller-retour FUSE par requête (lecture anticipée plafonnée à 128 KiB par le noyau, puis
  `lookup`/`read`/`getattr` par fichier sur les petits fichiers). `FUSE_NO_OPEN_SUPPORT` gagne 1,5 × sur
  les petits fichiers ; `FOPEN_DIRECT_IO` mesuré et rejeté. Seuil inchangé, toujours non bloquant ;
  métriques par corpus ajoutées pour séparer flux et coût par fichier (détail : ADR-0004, section finale).
  **Leçon ajoutée** : avant d'attribuer une perte à son propre code, mesurer le nombre de requêtes et leur
  taille côté noyau (`h4.mount_req_kib`) et le coût fixe par requête ; une hypothèse plausible chiffrée
  (1/8 ≈ 0,125) n'est pas une mesure.
