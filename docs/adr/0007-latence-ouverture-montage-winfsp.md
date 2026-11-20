# ADR-0007 — Latence d'ouverture à travers le montage WinFsp

- **Statut** : accepté
- **Date** : 2026-11-20
- **Contexte** : l'adaptateur WinFsp de `vault-mount` (feature `winfsp`) permet pour la première fois
  de mesurer H4 sous Windows. Première exécution du banc (profil `ci`, `--require-env mount`) sur un
  poste de développement Windows 11 (16 cœurs logiques, WinFsp 2.2.26323, pilote et banc sur le même
  hôte, BLAKE3 calibré à 172 µs/MiB) :

  | Métrique | FUSE (CI, ADR-0004) | WinFsp (poste) | Seuil | Verdict WinFsp |
  |---|---|---|---|---|
  | `h4.core_read_ratio` | 0,99 | 1,22 | ≥ 0,9 | ✓ |
  | `h4.mount_content_ok` | 1 | 1 | = 1 | ✓ |
  | `h4.mount_open_ns` (p50 / p99, 2 000 ouvertures) | — | 190 µs / 825 µs | — | · |
  | `h4.mount_open_p99_over_blake3_mib` | 0,69 | **4,79** | ≤ 1 | **✗ (bloquante)** |
  | `h4.mount_read_ratio` | 0,18 | **0,05** | ≥ 0,7 | ✗ (non bloquante, ADR-0004) |

  Le cœur et l'exactitude du contenu tiennent ; la bande passante séquentielle est déjà consignée
  réfutée par l'ADR-0004 (la cause présumée, décodage concurrent du même morceau sans vol unique,
  s'applique aussi ici, avec un noyau Windows qui découpe en requêtes plus petites). La **nouveauté**
  est la latence d'ouverture : le p99 d'un `File::open` à travers WinFsp vaut 4,8 × le hachage d'un
  MiB, contre 0,69 sous FUSE.
- **Causes présumées** (à confirmer par mesure) : (1) sous Windows, une ouverture traverse le
  gestionnaire d'E/S, le filtre WinFsp et un aller-retour noyau → processus utilisateur par étape
  (`GetSecurityByName` puis `Open`), chaque étape résolvant le chemin complet dans l'adaptateur ;
  (2) l'adaptateur recherche d'abord le nom exact puis, en repli, parcourt le répertoire pour la
  correspondance insensible à la casse — coûteux si l'Explorateur ou le banc présente une casse
  différente de l'archive ; (3) bruit d'hôte (antivirus Defender inspectant les ouvertures sur un
  volume nouveau, poste de développement non isolé) ; (4) la comparaison au hachage d'un MiB
  pénalise un hôte rapide en BLAKE3 (172 µs/MiB ici contre un runner CI plus lent).
- **Bruit d'hôte observé** : trois exécutions `--only H4` successives sur le même poste donnent
  `h4.mount_open_p99_over_blake3_mib` 1,34 / 2,11 / 1,04 et `h4.core_read_ratio` 0,79 / 0,83 / 1,03
  (seuil ≥ 0,9, bloquant). Le cœur n'est pas en cause (1,22 sur l'exécution complète, 0,99 en CI
  Linux) : la mesure courte est sensible à la charge du poste. La mesure de référence est donc celle
  de la CI, pas celle du poste ; le seuil de `h4.core_read_ratio` n'est pas touché.
- **Options considérées** :
  1. Relever le seuil ou le dénominateur pour Windows — refusé : un seuil ajusté à la mesure ne réfute
     plus rien (ADR-0003, B-001).
  2. Passer la métrique en non bloquante partout — refusé : FUSE est dans le seuil (0,69) et la CI
     Linux perdrait un garde-fou vert.
  3. Exemption **déclarée par système** : `"non_blocking_on": ["windows"]` avec `adr` obligatoire, le
     seuil restant évalué et affiché (✗ visible dans chaque rapport, verdict H4 `refuted` sous Windows
     tant que la mesure est hors seuil), bloquant inchangé sous Linux. Chantier de suivi : mesurer
     sur `windows-latest` (hôte de référence), puis (a) mémoriser la résolution de chemin entre
     `GetSecurityByName` et `Open` pour le même fichier ; (b) index insensible à la casse construit
     à l'ouverture de l'archive (évite le parcours de répertoire) ; (c) comparer `--threads 1` et
     le parallélisme disponible ; (d) si la latence est structurelle à WinFsp, le consigner et
     proposer, par un nouvel ADR, un seuil distinct **par pilote** justifié par une mesure de
     référence (ouverture d'un fichier sur un volume WinFsp « passthrough »), jamais par la mesure
     de `tholos` seule.
  4. Désactiver la mesure H4 sous Windows (`requires: fuse`) — refusé : I-003, une mesure que
     l'hôte peut faire est due.
- **Décision** : option 3. H4 est consignée **réfutée sous Windows sur la latence d'ouverture et le
  débit séquentiel**, **soutenue sur le cœur et l'exactitude du contenu** ; le verdict Linux
  (ADR-0004) est inchangé.
- **Conséquences** : `h4.mount_open_p99_over_blake3_mib` porte `"non_blocking_on": ["windows"]` et
  `"adr"` vers ce fichier ; le banc gagne le champ `non_blocking_on` (contrôle : exemption sans ADR
  refusée au chargement ; test `os_exemption_keeps_threshold_and_blocks_elsewhere`) ; la CI
  `bench (profil ci, windows-latest)` exécute H4 à travers WinFsp et publie la mesure de référence.
  Remettre la métrique en bloquant sous Windows passe par une PR qui cite les mesures.
- **Dérogation RAIDER** : D (non-régressif) — un seuil déclaré n'est pas appliqué par la CI sur un
  système ; bornée à Windows, visible dans chaque rapport, suivie par le chantier ci-dessus.
