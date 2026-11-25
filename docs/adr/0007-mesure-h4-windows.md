# ADR-0007 — Mesure de H4 sous Windows (WinFsp) : référence CI avant tout verdict

- **Statut** : accepté
- **Date** : 2026-11-20, révisé le 2026-11-24
- **Contexte** : l'adaptateur WinFsp de `vault-mount` (feature `winfsp`) permet pour la première fois
  de mesurer H4 sous Windows (profil `ci`, `--require-env mount`). Deux hôtes ont parlé, et ils ne
  disent pas la même chose :

  | Métrique | FUSE (CI, ADR-0004) | WinFsp, poste de dév. (16 cœurs, 1 run complet, 3 runs `--only H4`) | WinFsp, `windows-latest` (1 run) | Seuil |
  |---|---|---|---|---|
  | `h4.core_read_ratio` | 0,99 | 1,22 ; puis 0,79 / 0,83 / 1,03 | **0,8966** | ≥ 0,9, bloquant |
  | `h4.mount_content_ok` | 1 | 1 | 1 | = 1 |
  | `h4.mount_open_p99_over_blake3_mib` | 0,69 | **4,79** ; puis 1,34 / 2,11 / 1,04 | **0,8958** | ≤ 1, bloquant |
  | `h4.mount_read_ratio` | 0,18 | 0,05 | 0,07 | ≥ 0,7, non bloquant (ADR-0004) |

  Le poste de développement a d'abord fait croire à une réfutation nette sur la latence d'ouverture
  (4,8 × le hachage d'un MiB) ; l'hôte de référence la contredit (0,90, dans le seuil) et montre à la
  place un cœur juste sous son seuil (0,8966 pour 0,9). Les mesures du poste varient de 0,79 à 1,22
  d'une exécution à l'autre : antivirus en temps réel sur un volume nouveau, charge de fond, hachage
  BLAKE3 rapide (172 µs/MiB) qui rend le dénominateur de la métrique d'ouverture sévère. Rien de tout
  cela n'est une mesure de référence.
- **Ce que l'on peut dire a priori** (sans mesure) : `h4.core_read_ratio` compare une lecture directe
  en requêtes de 1 MiB à la même lecture servie par `VaultFs` en requêtes de 128 KiB, soit huit fois
  plus d'appels de lecture du fichier d'archive. Le coût fixe par appel de lecture est plus élevé sous
  Windows (gestionnaire d'E/S, filtre antivirus, cache NTFS) que sous Linux ; le ratio dépend donc de
  l'hôte, pas seulement du cœur. Le plancher de ce ratio sur un hôte donné est mesurable sans
  `tholos` : lecture `std::fs` de l'archive en 1 MiB ÷ en 128 KiB. Tant que ce plancher n'est pas
  mesuré sur `windows-latest`, aucun seuil Windows ne peut être posé sans être post-hoc.
- **Options considérées** :
  1. Abaisser le seuil pour Windows ou le fixer à partir de la mesure (0,8966 → 0,85) — refusé :
     seuil post-hoc, ne réfute plus rien (ADR-0003, B-001).
  2. Passer `h4.core_read_ratio` en non bloquant partout — refusé : FUSE/Linux est dans le seuil
     (0,99), la CI Linux perdrait un garde-fou vert.
  3. Désactiver H4 sous Windows — refusé : I-003, une mesure que l'hôte peut faire est due.
  4. **Exemption déclarée par système, bornée dans le temps** : `"non_blocking_on": ["windows"]`
     (avec `adr` obligatoire) sur `h4.core_read_ratio` et `h4.mount_open_p99_over_blake3_mib`, seuils
     inchangés, évalués et affichés dans chaque rapport, bloquants sous Linux. L'exemption dure le
     temps de constituer la référence : **trois exécutions** du job `bench (profil ci, windows-latest)`
     sur `main`, artefact `bench-ci-windows`.
- **Décision** : option 4. À l'issue des trois exécutions, une PR qui cite les trois rapports fait
  l'un des deux gestes suivants, pour chaque métrique séparément :
  - les trois mesures sont dans le seuil → l'exemption est retirée (la métrique redevient bloquante
    sous Windows) ;
  - une mesure au moins est hors seuil → la métrique est consignée **réfutée sous Windows**
    (`docs/engineering/lessons-learned.md`, section « Réfutations ») et un nouvel ADR propose, s'il y
    a lieu, un seuil **par système dérivé du plancher mesuré** (lecture `std::fs` 1 MiB ÷ 128 KiB sur
    l'hôte de référence, ou ouverture d'un fichier sur un volume WinFsp « passthrough » pour la
    latence), jamais de la mesure de `tholos` seule.
  Pistes à mesurer si le cœur reste sous le seuil : (a) taille de requête côté `VaultFs` sous
  Windows (le noyau découpe-t-il plus petit que 128 KiB ?) ; (b) coût d'un `ReadFile` froid avec et
  sans exclusion antivirus ; (c) `--threads 1` contre le parallélisme disponible.
- **Conséquences** : `bench/config.json` porte `"non_blocking_on": ["windows"]` et `"adr"` vers ce
  fichier sur les deux métriques ; le banc gagne le champ `non_blocking_on` (contrôle : exemption sans
  ADR refusée au chargement ; test `os_exemption_keeps_threshold_and_blocks_elsewhere`) ; la CI
  `bench (profil ci, windows-latest)` exécute H4 à travers WinFsp et publie la mesure de référence.
  Les mesures du poste de développement ne servent qu'à détecter des régressions grossières, pas à
  consigner un verdict (leçon B-003).
- **Dérogation RAIDER** : D (non-régressif) — deux seuils déclarés ne sont pas appliqués par la CI
  sur un système ; bornée à Windows et à trois exécutions de référence, visible dans chaque rapport.
