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
