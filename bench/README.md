# bench — banc des thèses H1–H8

Banc public et reproductible des thèses du rapport de recherche (`docs/recherche/`). Il génère un corpus
synthétique déterministe, empaquette avec `vault-core`, mesure, puis confronte chaque métrique au contrat
déclaré dans `config.json`. Un seuil **bloquant** non atteint fait échouer l'exécution (et la CI).

## Lancer

```bash
cargo run -p vault-bench --release -- --profile ci              # ≈ 2–4 min, profil joué en CI
cargo run -p vault-bench --release -- --profile full            # longue durée, hôte de référence
cargo run -p vault-bench --release -- --profile ci --only H1,H3 # sous-ensemble
cargo run -p vault-bench --release -- --profile ci --summary-md rapport.md
```

Options : `--config` (défaut `bench/config.json`), `--out` (défaut `bench/results/`), `--corpus-root`
(défaut `bench/corpus/`, ignoré par git, régénéré à l'identique si le digest diffère).

Codes de sortie : **0** toutes les métriques bloquantes passent · **1** au moins une échoue · **2**
configuration invalide, corpus hors bande de validité ou contrat métrique non respecté (métrique déclarée
non produite, ou produite non déclarée).

## Profils

| Profil | code | binaire | petits fichiers | base doublons | parité | graines | niveau 19 / LDM |
|---|---|---|---|---|---|---|---|
| `ci` | 16 MiB | 16 MiB | 2 000 / 20 000 | 32 MiB × {0,10,20,40} % | 32 MiB | 8 | non |
| `full` | 256 MiB | 256 MiB | 10 000 / 100 000 | 128 MiB × idem | 256 MiB | 100 | oui |

Le corpus est tiré d'un PRNG xorshift à graine fixe par catégorie : code (texte source synthétique),
binaire (lignes de 32 octets faiblement corrélées), petits fichiers, doublons (copies exactes et copies
éditées à taux contrôlé), parité (aléatoire). Chaque corpus porte un digest BLAKE3 dans le rapport ; deux
exécutions au même commit produisent le même digest.

## Thèses et métriques

| Thèse | Statut | Métriques à seuil (bloquantes sauf mention) |
|---|---|---|
| H1 découpage vs zstd solide | mesurée | `h1.loss_pct.{code,binary}.block1m` ≤ 20 / 10 % ; `h1.loss_pct.{code,binary}.cdc` ≤ 25 % |
| H2 déduplication CDC | mesurée | `h2.dedup_recall.exact` ≥ 0,99 ; `h2.edit_cost_kib` ≤ 512 KiB ; `h2.penalty_pct.d20`, `.d40` ≤ 0 % |
| H3 montage et accès aléatoire | mesurée | `h3.metadata_plaintext` = 1 ; `h3.open_scaling` ≤ 20× (10× plus de fichiers) ; `h3.find_p99_over_blake3_mib` ≤ 0,05 ; `h3.chunks_per_read4k.max` ≤ 2 |
| H4 le cœur d'un montage lit à la vitesse du décodeur ; à travers le noyau, débit borné par la latence par requête (reformulée, ADR-0004) | mesurée | `h4.core_read_ratio` ≥ 0,9 (cœur `vault-mount` en requêtes de 128 KiB ÷ lecture directe 1 MiB) ; avec pilote (`requires: mount`) : `h4.mount_content_ok` = 1, `h4.mount_read_ratio` ≥ 0,7 (0,18 mesuré en CI, non bloquante : plancher de latence FUSE par requête, voir [ADR-0004](../docs/adr/0004-debit-lecture-montage-fuse.md) ; informatives : `h4.mount_read_ratio_code`, `h4.mount_read_ratio_binary`, `h4.mount_decode_ratio`, `h4.mount_req_kib`), `h4.core_concurrent_decode_ratio` ≤ 1,05 (vol unique), `h4.mount_open_p99_over_blake3_mib` ≤ 1 |
| H5 parité Reed-Solomon | mesurée | `h5.repair_rate.*` = 1 (4 scénarios) ; `h5.build_vs_blake3` ≤ 3× (1,8–2,1× à ≥ 4 fils, 3,8× mono-fil ; BLAKE3 du même volume, voir [ADR-0003](../docs/adr/0003-cout-construction-parite.md)) |
| H6 delta entre archives | non mesurée | prérequis : commande delta |
| H7 dictionnaire zstd | non mesurée | prérequis : dictionnaire dans le format |
| H8 décodage vs disque | non mesurée | prérequis : hôte et disque de référence |

Une métrique peut exiger une capacité de l'hôte (`requires` : `mount`, `fuse`, `winfsp`). Absente,
la métrique est **sautée** (valeur `—`, raison dans la colonne OK) et la thèse a le verdict
`incomplete`, jamais `supported` : le banc ne conclut pas sur un montage qu'il n'a pas fait. Produire
une telle métrique sans la capacité est une erreur (code 2). En CI, `--require-env mount` transforme
l'absence du pilote en erreur de configuration (code 2) au lieu d'un rapport incomplet : la mesure
exigée ne peut pas être sautée en silence. Sous Linux il faut `fuse3` (`fusermount3`) ; le champ
`host.capabilities` du JSON dit ce qui a été détecté.

Les métriques sans seuil (ratios, ns, s/GB, surcoût d'index, rappel édité, pénalité à 0 et 10 %) sont
**rapportées** pour suivre la tendance. Les métriques de temps sont rapportées à BLAKE3, ce qui rend les
seuils indépendants de l'hôte : latence par MiB mesurée au début de l'exécution (1 MiB en cache) pour les
métriques de latence (H3), et BLAKE3 mono-fil du même volume, mesuré au même instant, pour les métriques de
débit (H5, `h5.blake3_s_per_gb` rapporté à côté de `h5.build_s_per_gb`).

## Résultats

`results/<profil>.json` (schéma `tholos-bench/1` : outil, hôte, corpus avec digests et validité, thèses,
métriques avec p50/p99 quand il y a des échantillons, verdict `pass`), `results/<profil>.csv` (une ligne
par métrique) et `results/<profil>.samples.raw` (échantillons bruts, ignorés par git). Le résumé markdown
est imprimé sur la sortie standard et, en CI, ajouté au résumé du job ; les JSON/CSV sont publiés en
artefact `bench-ci`.

Les temps mesurés sur un runner partagé sont bruités : les seuils bloquants portent sur des rapports
(à BLAKE3, à zstd solide, entre deux tailles) ou des invariants, jamais sur une durée absolue.

## Réfuter sans tricher

- Un seuil ne se relâche pas pour faire passer la CI. Une thèse réfutée se consigne : ADR dans
  `docs/adr/`, entrée dans `docs/engineering/lessons-learned.md`, métrique passée en `"blocking": false`
  avec un champ `"adr"` (contrôlé au chargement : un seuil non bloquant sans ADR est refusé).
- Seuls les générateurs de corpus peuvent être recalibrés, dans leurs bandes de validité (taux de doublons
  réel à ±2 points, etc.) ; un corpus hors bande fait échouer l'exécution (code 2).
- Ajouter une métrique : la déclarer dans `config.json` **et** la produire dans `src/theses/` ; le contrat
  déclaré ↔ produit est vérifié à chaque exécution.
