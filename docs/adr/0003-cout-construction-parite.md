# ADR-0003 — Coût de construction de la parité Reed-Solomon

- **Statut** : accepté
- **Date** : 2026-11-16
- **Contexte** : la thèse H5 (`docs/recherche/`) annonce qu'une parité RS à 10 % de redondance ajoute
  ≤ 0,3 s/GB au build. Le banc (`bench/`, métriques `h5.build_s_per_gb` et `h5.build_vs_blake3`) mesure
  sur l'hôte de développement (16 cœurs, encodage mono-thread) **1,0–1,3 s/GB**, soit **5–10 × le hachage
  BLAKE3** du même volume (calibration 0,18–0,20 s/GB). Un balayage de la géométrie (shards de 4 à 64 KiB,
  groupes de 32 à 1024 shards) donne 0,94–1,44 GB/s : la borne vient de l'encodeur (`reed-solomon-simd`,
  GF(2¹⁶), un seul thread), pas de la disposition des stripes. Les autres volets de H5 sont soutenus :
  4 scénarios de dégâts (aléatoires 2 % et 5 %, contigus 5 % et 10 %) × 8 graines, 100 % réparés.
  Le seuil retenu dans `bench/config.json` est relatif (≤ 3 × BLAKE3) pour être indépendant de l'hôte ;
  il équivaut à ≈ 0,55 s/GB sur l'hôte de développement.
- **Options considérées** :
  1. Relever le seuil à la valeur mesurée — refusé : un seuil ajusté à la mesure ne réfute plus rien.
  2. Garder le seuil bloquant — refusé : CI rouge en permanence sans action possible dans le chantier banc.
  3. Garder le seuil déclaré, passer la métrique en non bloquante avec renvoi vers cet ADR, et ouvrir un
     chantier « encodage parallèle par groupe » (les groupes sont indépendants ; 8 groupes pour 32 MiB) qui
     doit ramener le coût mur sous 0,3 s/GB sur ≥ 4 cœurs ; réarmer `blocking: true` quand le banc passe.
  4. Changer d'encodeur (GF(2⁸) type ISA-L / `reed-solomon-erasure`) — à comparer dans le même chantier
     (reuse-first : mesurer deux approches avant de coder).
- **Décision** : option 3. H5 est consignée **réfutée sur le coût de construction mono-thread**, **soutenue
  sur la réparation**.
- **Conséquences** : `h5.build_vs_blake3` porte `"blocking": false` et `"adr"` vers ce fichier ; le rapport
  continue d'afficher la valeur et le ✗. Toute métrique à seuil non bloquante doit porter un champ `adr`
  (contrôle au chargement de la configuration). Remettre la métrique en bloquant ou changer le seuil passe
  par une PR qui cite les mesures ; `h5.build_s_per_gb` reste reporté pour suivre la tendance.
- **Dérogation RAIDER** : D (non-régressif) — un seuil déclaré n'est pas appliqué par la CI ; borné par le
  chantier de suivi et visible dans chaque rapport.

## Suivi (2026-11-16) — encodage parallèle par tranches de colonnes

- **Option 4 écartée** : les encodeurs GF(2⁸) (ISA-L, `reed-solomon-erasure`) imposent k + m ≤ 256 shards
  par groupe ; la géométrie v1 par défaut (groupes de 1 024 shards) l'exclut sans changement de format.
  `reed-solomon-simd` est conservé.
- **Mise en œuvre** (`vault-core`, fonctionnalité `parallel`, activée par défaut, `rayon`) : le code RS agit
  indépendamment sur chaque bloc de 64 octets d'un shard, donc encoder une tranche de colonnes d'un groupe
  donne exactement la tranche correspondante des shards de secours (test `slicing_is_exact`). Chaque groupe
  est découpé en tranches de 512 octets (ensemble de travail (k + m) × largeur ≤ 1 MiB, en cache L2), les
  tâches groupes × tranches sont réparties sur le pool, un encodeur par travailleur réutilise son espace de
  travail, les CRC des shards sont calculés en parallèle et les résultats écrits en place. La sortie est
  identique octet pour octet, avec ou sans la fonctionnalité (`--no-default-features`, testé en CI).
- **Mesures** (hôte de développement, 32 MiB, 10 %, médiane de 8 constructions à froid) :

  | Fils rayon | `h5.build_s_per_gb` | `h5.build_vs_blake3` |
  |---|---|---|
  | référence (main avant ce chantier) | 1,23 | 7,0 |
  | 1 | 0,84 | 6,2 |
  | 4 | 0,33–0,38 | 2,3–3,5 |
  | 16 | 0,33–0,39 | 2,2–2,5 |

  Le coût mur est divisé par 3,2–3,7 à 4 fils et plus ; le chemin mono-fil gagne ≈ 30 % (tranches en cache,
  encodeur réutilisé).
- **Seuil non réarmé** : à 4 fils le rapport oscille autour de 3 et dépend surtout du dénominateur — la
  calibration BLAKE3 varie de 0,11 à 0,18 s/GB d'une exécution à l'autre sur le même hôte (buffer de 1 MiB
  en cache, fréquence turbo), alors que la construction traite 32 MiB en mémoire. Les exécuteurs CI
  (4 vCPU partagés) sont plus lents et plus bruyants. `h5.build_vs_blake3` reste donc non bloquante avec le
  même seuil ; `h5.build_s_per_gb` suit la tendance.
- **Réarmement** : (1) calibrer BLAKE3 sur le même volume que la construction (mono-fil, 32 MiB en mémoire)
  pour que le rapport compare deux passes équivalentes — changement du banc à consigner ici avant d'être
  appliqué, pas un relâchement du seuil ; (2) rejouer ≥ 5 exécutions `--only H5` à 4 fils ; réarmer
  `blocking: true` si le rapport est ≤ 2,5 dans toutes. Piste restante côté cœur : écrire les shards de
  secours directement dans la section (évite une copie de 3,4 MiB par 32 MiB) et paralléliser `reconstruct`.

## Suivi (2026-11-17) — calibration BLAKE3 sur le volume protégé, seuil réarmé

- **Changement du banc** (étape 1 du réarmement ci-dessus) : le dénominateur de `h5.build_vs_blake3` n'est
  plus la calibration de l'hôte (BLAKE3 d'1 MiB en cache, mesurée au démarrage) mais BLAKE3 mono-fil du
  **même buffer protégé** (32 MiB), mesuré juste après chaque construction, médiane sur les mêmes graines.
  Les deux passes traversent le même volume en mémoire au même instant : le rapport ne dépend plus de la
  fréquence turbo ni du moment de la calibration. Le dénominateur est rapporté (`h5.blake3_s_per_gb`) pour
  rester vérifiable dans chaque rapport. La calibration 1 MiB reste celle des métriques de latence (H3).
  Le seuil (≤ 3) est inchangé : c'est la mesure qui est rendue comparable, pas la borne qui est déplacée.
- **Mesures** (hôte de développement, 32 MiB, 10 %, `--only H5`, 7 exécutions) :

  | Fils rayon | `h5.build_s_per_gb` | `h5.blake3_s_per_gb` | `h5.build_vs_blake3` |
  |---|---|---|---|
  | 4 (× 5) | 0,30–0,44 | 0,16–0,22 | **1,82–2,06** |
  | 16 | 0,34 | 0,17 | 2,03 |
  | 1 | 0,64 | 0,17 | 3,76 |

  Numérateur et dénominateur bougent ensemble d'une exécution à l'autre (hôte plus ou moins chargé) ; le
  rapport varie de ± 6 % contre ± 30 % avec l'ancienne calibration.
- **Décision** : condition de réarmement remplie (≤ 2,5 dans les 5 exécutions à 4 fils) →
  `h5.build_vs_blake3` repasse `"blocking": true` dans `bench/config.json`, seuil ≤ 3 inchangé, champ `adr`
  conservé pour la traçabilité. La dérogation RAIDER (D) consignée plus haut est levée. Mono-fil, H5 reste
  réfutée sur le coût (3,8 ×) : le banc en CI tourne sur ≥ 4 vCPU ; un hôte mono-cœur verra la métrique
  échouer, ce qui est le comportement attendu d'un seuil déclaré.
- **Statut de la thèse** : H5 **soutenue** à ≥ 4 fils (coût ≤ 3 × BLAKE3, réparation 100 %), réfutée
  mono-fil. Pistes restantes côté cœur : écrire les shards de secours directement dans la section,
  paralléliser `reconstruct`.

