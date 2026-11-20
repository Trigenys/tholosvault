# ADR-0004 — Débit de lecture à travers le montage FUSE

- **Statut** : accepté
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
