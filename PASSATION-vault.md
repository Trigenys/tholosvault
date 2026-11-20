# Passation

## 2026-11-16 12:37 — Cadrage produit VAULT / tholosvault

**Objectif en cours** — Cadrage terminé (surfaces, stack, nom, licence) ; prochaine étape : premier code de `vault-core` et CI.
**Fait**
- `docs/produit/cadrage-surfaces.md` : surfaces (CLI, landing, desktop, web, mobile), parcours, matrice MVP/V1/plus tard, décisions D1–D11 (§8).
- `docs/produit/stack.md` : 2 options par surface, choix : Rust core/CLI/mount/Axum, Tauri 2 + React, Astro, Next.js, Flutter plus tard.
- `docs/produit/nom.md`, `docs/adr/0002-nom-public-tholosvault.md` : nom public **tholosvault**, signature « by Trigenys Group », CLI `tholos`.
- `docs/adr/0001-modele-open-core-apache.md`, `LICENSE` : open core, Apache-2.0 sur format + vault-core/cli/mount.
- Squelette monorepo (crates/, apps/, packages/, bench/ avec READMEs), `.gitignore`, `.editorconfig`, `.gitattributes`, `docs/engineering/lessons-learned.md` (L-001 à L-004).
- Commits sur `main` : 1ceb492, 882dd49, 64d7695, 1157a53, 0d4058f. Aucun remote.
**Décisions**
- Domaine : pas d'achat ; `tholosvault.trigenys.com` (landing) et `console.tholosvault.trigenys.com` (DNS Spaceship, CNAME à créer au déploiement).
- VAULT reste le nom interne (crates `vault-*`) ; renommage éventuel en `tholos-*` par ADR.
- Historique `PASSATION.md` (3 premières entrées + 2 correctifs) appartient à cette même session ; à partir d'ici, journal dans ce fichier.
**Reste à faire**
1. Réserver org GitHub `tholosvault`, crate et paquet npm ; contrôle de marque INPI/EUIPO (utilisateur).
2. Créer le remote GitHub, pousser `main` ; envisager Git LFS pour `docs/media/VAULT-principe-v5.mp4` (38 Mo).
3. Adopter `EagleFox31/appfactory-project-automation` (impact-aware CI, releases) avant tout workflow maison.
4. Démarrer `crates/vault-core` : CDC (fastcdc), zstd seekable, Merkle BLAKE3, index mmap ; protocole bench H1–H8 dans `bench/`.
5. Trancher D1–D5, D8–D10 (§8 du cadrage) au fil de l'eau, chacune par ADR si structurante.
**Pièges / contexte**
- `python3` absent : utiliser `python -I -` avec heredoc ; `python -` sans corps bloque le REPL.
- Vérif. domaines : `https://dns.google/resolve?name=X&type=NS` (Status 3 = libre) ; `nslookup` trompe.
- Pas de mention d'assistant dans commits/docs ; PASSATION en ajout seul ; messages de commit sans trailer.

## 2026-11-16 12:53 — Premier code de vault-core (format v0) et CI

**Objectif en cours** — Format v0 lisible/écrivable, CLI `tholos` minimal, CI change-scoped en place ; prochaine étape : remote GitHub, parité RS + injection de corruptions (L-004), bench H1–H8.
**Fait**
- Workspace Cargo (`Cargo.toml`, `rust-toolchain.toml`), crates `vault-core` et `vault-cli` (binaire `tholos`).
- `vault-core` : `chunk` (fastcdc 2020, 64/128/256 KiB), `codec` (zstd bulk, niveau 15, version zstd dans l'en-tête),
  `merkle` (BLAKE3, domaines 0x00/0x01/0x02), `format` (en-tête 64 o, trailer 128 o ×2 avec CRC32C), `writer`
  (`pack_dir`, dédup par hash, déterministe), `reader` (`Archive` par mmap, `read_file`, `verify`, lookup par hash).
- Spéc : `docs/format/spec-v0.md`. Tests : 13 unitaires + 7 d'intégration (`tests/format_v0.rs`) : aller-retour,
  déterminisme L-001, dédup, corruption détectée avec métadonnées lisibles L-002, trailer répliqué, dossier vide, rejet.
- CLI : `tholos pack|list|verify|extract` ; fumée OK (pack → list → verify → extract → `diff -r` identique).
- CI : `.github/appfactory-impact.json` + `.github/workflows/ci.yml` (reusable-impact-analysis épinglé SHA
  `3fbf1825…`, main d'appfactory ; le tag v1.1 ne contient pas ce workflow). Job `rust` gardé par le gate `rust`,
  matrice ubuntu + windows : fmt, clippy -D warnings, test, build release. Non exécutée (pas de remote).
- `docs/engineering/lessons-learned.md` : incident I-001 (mmap verrouille le fichier sous Windows).
- Reuse-first : Adopt fastcdc 5, blake3 1.8, zstd 0.14, memmap2 0.9, crc32c 0.6, thiserror 2, clap 4 ; Build pour
  l'agencement binaire (aucun format existant ne combine dédup + index mmap + trailer répliqué).
**Décisions**
- v0 = détection seule ; parité RS, dictionnaires (`dict_id` réservé), chiffrement, tier froid → versions suivantes.
- Trailer essayé de la fin vers l'avant ; en-tête porte codec, niveau, version zstd, paramètres CDC.
- Fichiers réguliers seulement (liens, permissions, horodatages ignorés) : déterminisme avant fidélité.
**Reste à faire**
1. Créer le remote GitHub et pousser `main` (déclenche la première CI) ; LFS pour la vidéo 38 Mo.
2. Parité Reed-Solomon (`reed-solomon-simd` 3.1) + tests d'injection de corruptions (L-004) → format v1.
3. Bench H1–H8 dans `bench/` (corpus, protocole, seuils bloquants L-002).
4. Lecture par plage (`read_range`) et `vault-mount` (fuser / winfsp-rs).
5. Réservations org/crate/npm et marque (utilisateur).
**Pièges / contexte**
- MSVC trouvé par le crate `cc` sans `cl.exe` dans le PATH : `cargo build` passe tel quel.
- fastcdc 5 prend des `usize` ; l'en-tête stocke des `u32` (conversion explicite dans `chunk.rs`).
- Sous Windows, fermer l'`Archive` (drop) avant de réécrire le fichier.
- Heredocs Bash avec apostrophes françaises peu fiables ici : passer par l'outil d'écriture de fichiers.

## 2026-11-16 14:50 — Format v1 : parité Reed-Solomon et injection de corruptions (L-004)

**Fait**
- `FORMAT_VERSION = 1`, `MIN_FORMAT_VERSION = 0` : le lecteur accepte v0 et v1 ; le trailer porte `parity_len`
  (octets 112..120, nuls en v0). Spéc : `docs/format/spec-v1.md` (+ renvoi dans `spec-v0.md`).
- `vault-core/src/parity.rs` : `ParityOptions { percent (5 %), shard_size (4 KiB), group_max (1024) }`,
  `Geometry` (N shards, G groupes entrelacés, m = ⌈k·pct/100⌉ + 1 parités), `build_section` (en-tête `THOLOSPR`
  + CRC32C par shard + parités, slot `j·G+g`), `ParitySection::parse`, `scan` → `Damage` (réparable / non),
  `reconstruct` → `Patch`. Reuse-first : Adopt `reed-solomon-simd` 3.1 (Leopard-RS, GF(2^16)).
- Parité calculée sur les octets tels qu'écrits (en-tête + frames + index + tables), relus par mmap du fichier
  en cours d'écriture (ouvert lecture + écriture). `PackOptions.parity`, `PackSummary.parity_bytes`.
- `reader.rs` : `VerifyReport.damage` + `repairable()` (`is_ok()` inchangé) ; `scan_file`, `repair_file`
  (copie `<chemin>.tmp` → patches → `rename`, I-001 ; no-op si rien à réparer ; `Unrepairable` / `NoParity` /
  `NoValidTrailer` sans écriture), `Archive::repair(self)`, `has_parity()`, `trailer()`.
- CLI : `tholos pack --parity <PCT>` (0 désactive), `tholos repair`, `tholos verify` affiche « réparable /
  non réparable » et retombe sur `scan_file` si l'ouverture échoue. Fumée : 2 % contigus + trailer → réparé → OK.
- Tests : `tests/common/mod.rs` partagé ; `tests/parity_v1.rs` (12) : archive saine, déterminisme, v0 relu,
  aléatoire 1 % / 2 % / 5 % (budget 15 %), contigu 1-3-5 % / 10 % (budget 15 %), en-tête + index détruits,
  trailer (1 copie restaurée, 2 → `NoValidTrailer`), au-delà du budget sans écriture, sans parité → `NoParity`.
  Les 7 tests v0 restent verts (dont un en options v0 pures). fmt, clippy -D warnings, test workspace : OK.
**Décisions**
- Section en clair auto-vérifiée (CRC en-tête + CRC tables) ; géométrie recalculée depuis le trailer et refusée
  si l'en-tête la contredit. Section illisible = dégâts non réparables (pas de devinette).
- Repli hachage → shards : un morceau au BLAKE3 faux sans shard CRC-faux voit tous ses shards marqués effacés.
**Reste à faire**
1. Balayage de secours du trailer quand les deux copies sont détruites (non réparable aujourd'hui).
2. Bench du coût de `build_section` sur gros corpus (`bench/`) ; parité par flux pour éviter le mmap d'écriture.
3. Items 1, 3-5 de l'entrée précédente (remote GitHub, bench H1–H8, `read_range`, réservations).
**Pièges**
- `reed-solomon-simd` exige des shards pairs et non vides ; `decode` ne renvoie que les originaux manquants.
- clippy `single_range_in_vec_init` : construire un `Vec<Range>` d'un élément via `iter::once(..).collect()`.

## 2026-11-16 16:07 — Dépôt GitHub Trigenys/tholosvault, première CI, enregistrement au registre

**Objectif en cours** — Le dépôt public existe et la CI est verte ; prochaine étape : bench H1–H8, `read_range`, `vault-mount`, et décision sur la gouvernance / le board.
**Fait**
- `Trigenys/tholosvault` créé par l'utilisateur (public, description « Mountable, verifiable and deduplicated archive format with the tholos CLI, by Trigenys Group »). Remote `origin` ajouté, `main` poussé (3 commits : 20db4bb v0, 61fe1aa URL manifeste, 0423ca3 parité RS v1).
- Première CI (run 43891855725) : `impact / analyze`, `rust (ubuntu-latest)`, `rust (windows-latest)` tous verts sur 0423ca3.
- Registre : PR https://github.com/EagleFox31/project-registry/pull/72 (branche `registre/tholosvault`) ajoute l'entrée `tholosvault` (prototype, draft, portfolio, dépôt `application-principale` exposé). Tests du registre identiques avant/après (2 échecs préexistants sans lien). À merger par l'utilisateur.
- Gouvernance appfactory : `plan` exécuté en lecture seule, en local, avec le cœur `src/governance` d'appfactory-project-automation et le token gh (le workflow exige un secret `APPFACTORY_GOVERNANCE_TOKEN` ou une GitHub App, non créés). Résultat : CREATE ruleset « AppFactory default branch governance », preset `solo` (pas de suppression ni force-push, PR obligatoire, fils de revue résolus, 0 approbation requise). **Non appliqué** : la PR obligatoire bloquerait les commits directs sur `main` pratiqués par les sessions actuelles ; décision utilisateur.
- Pas de Git LFS : la vidéo de 38 Mo passe sous la limite de 100 Mo.
**Décisions**
- Description GitHub en anglais, une phrase, « by Trigenys Group », alignée sur les dépôts publics de l'org.
- Board GitHub Project et releases (Release Please) : non posés, en attente d'accord explicite.
**Reste à faire**
1. Utilisateur : merger la PR 72 du registre ; trancher gouvernance (`apply` = passer au flux PR) et board/releases.
2. Bench H1–H8 dans `bench/` (corpus, protocole, seuils bloquants L-002).
3. Lecture par plage (`read_range`) et `vault-mount` (fuser / winfsp-rs).
4. Réservations crate/npm et marque (utilisateur).
**Pièges / contexte**
- `gh repo create` est refusé par le filtre de permissions du mode auto : la création de dépôt revient à l'utilisateur.
- Le clone du registre dans le scratchpad n'a pas d'identité git : poser `user.name`/`user.email` locaux avant de commiter.
- `gh pr create` depuis un clone shallow exige `--head <branche>`.
- Autre session (parité RS) a partagé ce checkout : vérifier `git status` avant tout commit, ne commiter que ses propres fichiers.

## 2026-11-16 16:13 — README public, gouvernance de branche appliquée

**Objectif en cours** — Dépôt public présentable et protégé ; prochaine étape : bench H1–H8, `read_range`, `vault-mount`.
**Fait**
- README racine réécrit (promesse, tableau pourquoi, ce qui marche, démarrage, format, feuille de route, docs, licence), commit 5a99825.
- Gouvernance appfactory : `.github/project-config.json` (preset `solo`) et `.github/workflows/repository-governance.yml` (manuel, action épinglée SHA 3fbf1825…), commit 6299760. Ruleset « AppFactory default branch governance » (id 27383387) **appliqué** en local via le cœur `src/governance` et le token gh, sur accord explicite ; second `plan` = NO-OP.
- Conséquence : `main` n'accepte plus de push direct. Toute modification passe par une branche et une PR (0 approbation requise, fils de revue résolus, pas de force-push ni suppression).
**Décisions**
- Le workflow GitHub de gouvernance reste inactif tant que le secret `APPFACTORY_GOVERNANCE_TOKEN` (PAT fine-grained, Administration RW sur ce seul dépôt) ou la GitHub App n'est pas posé : à faire par l'utilisateur ; la réconciliation locale suffit en attendant.
**Reste à faire**
1. Utilisateur : merger la PR 72 du registre ; créer le secret de gouvernance si l'on veut le workflow et la réconciliation continue ; décider board / releases.
2. Bench H1–H8 dans `bench/` ; `read_range` ; `vault-mount`.
**Pièges / contexte**
- Depuis l'application du ruleset, travailler sur une branche : `git switch -c <type>/<sujet>` puis `gh pr create`. Les sessions partageant ce checkout doivent faire de même.
- `cancel-in-progress` de la CI annule le run d'un push précédent quand un second suit de près (5a99825 annulé par 6299760) : normal.

## 2026-11-16 16:38 — Gouvernance branchée sur la GitHub App AppFactory (zéro PAT)

**Objectif en cours** — Réconciliation continue du ruleset sans créer de secret ; prochaine étape : bench H1–H8, `read_range`, `vault-mount`.
**Fait**
- `.github/workflows/repository-governance.yml` réécrit sur le workflow réutilisable `reusable-repository-governance.yml` (SHA 3fbf1825…, même épingle que la CI) en `authentication: github-app` : `vars.APPFACTORY_APP_CLIENT_ID` + `secrets.APPFACTORY_APP_PRIVATE_KEY`. Déclencheurs : manuel (plan/apply), push sur la config de gouvernance depuis la branche par défaut, cron quotidien 03:17 UTC (apply).
- Inventaire préalable : aucun secret/variable sur le dépôt ; org Trigenys illisible sans scope `admin:org` ; aucun autre dépôt Trigenys ne fait tourner la gouvernance ; `broker-user` ne couvre que les Projects. Seule voie zéro-PAT : la GitHub App AppFactory déjà existante (README du broker).
**Décisions**
- Règle permanente : ne jamais proposer un nouveau PAT/secret avant d'avoir vérifié l'existant (org, App, broker, autres dépôts). `APPFACTORY_GOVERNANCE_TOKEN` abandonné.
**Reste à faire**
1. Utilisateur : exposer `APPFACTORY_APP_CLIENT_ID` (variable org) et `APPFACTORY_APP_PRIVATE_KEY` (secret org) à `Trigenys/tholosvault`, installer l'App sur le dépôt, puis lancer le workflow en `plan` pour valider (attendu : NO-OP). Merger la PR 72 du registre ; décider board / releases.
2. Bench H1–H8 ; `read_range` ; `vault-mount`.
**Pièges / contexte**
- Sans ces valeurs, le job échoue dès l'étape « Validate governance authentication » avec une erreur explicite : comportement attendu, pas une régression.
- Commit construit par plumbing git (`GIT_INDEX_FILE` temporaire, `commit-tree`, `update-ref`) car `git worktree add` échoue sur Windows (« Filename too long », vignettes `apps/landing/video/.thumbnails/`) et le checkout est partagé avec d'autres sessions.

## 2026-11-16 16:52 — GitHub App appfactory-bot : variable posée, clé privée à poser par l'utilisateur

**Objectif en cours** — Activer le workflow de gouvernance (PR 2) en zéro PAT.
**Fait**
- Scope `admin:org` accordé au token `gh` par l'utilisateur (device flow).
- Inventaire org Trigenys : aucune variable ni secret Actions ; App **appfactory-bot** (propriétaire Trigenys, client ID public `Iv23lisXtkmMFl0u3hs5`) déjà installée sur **tous** les dépôts de l'org avec Administration en écriture : rien à installer.
- Variable d'org `APPFACTORY_APP_CLIENT_ID` créée, visibilité `selected` limitée à `tholosvault` (valeur publique, pas un secret).
**Reste à faire**
1. Utilisateur : poser le secret d'org `APPFACTORY_APP_PRIVATE_KEY` (PEM de l'App, à générer dans les réglages de l'App si aucune clé n'est disponible) avec `gh secret set APPFACTORY_APP_PRIVATE_KEY --org Trigenys --visibility selected --repos tholosvault < cle.pem`. Puis merger la PR 2 et lancer le workflow en `plan` (attendu : NO-OP).
2. Bench H1–H8 ; `read_range` ; `vault-mount`.
**Pièges / contexte**
- Lire ou poser une clé privée est hors de mon périmètre : seule la commande est fournie. Ne jamais mettre le PEM dans une variable ni dans un fichier du dépôt.
- `gh api orgs/<org>/installations` liste les Apps installées et leurs permissions : vérifier là avant d'installer quoi que ce soit.

## 2026-11-16 17:45 — Gouvernance continue active via la GitHub App (zéro PAT)

**Objectif en cours** — Gouvernance opérationnelle ; prochaine étape : bench H1–H8, `read_range`, `vault-mount`.
**Fait**
- Secret d'org `APPFACTORY_APP_PRIVATE_KEY` posé par l'utilisateur (clé générée dans Credentials → Key pairs de l'App appfactory-bot), visibilité limitée à `tholosvault` ; variable d'org `APPFACTORY_APP_CLIENT_ID` idem. Deux anciennes paires de clés orphelines (fichiers introuvables sur la machine) à supprimer côté App.
- PR 2 mergée (b603bc8) : workflow en `authentication: github-app`. Premier run annulé : « deadlock detected for concurrency group » car le workflow appelant et le réutilisable déclaraient le même groupe. PR 3 mergée (0121e83) : bloc `concurrency` retiré côté appelant, comme l'exemple appfactory.
- Runs verts : push (apply) 43910565525 et dispatch (plan) 43910629996 ; jeton d'App révoqué en fin de job. Ruleset 27383387 inchangé.
**Reste à faire**
1. Utilisateur : merger la PR 72 du registre ; décider board / releases ; supprimer les 2 clés orphelines de l'App.
2. Bench H1–H8 ; `read_range` ; `vault-mount`.
**Pièges / contexte**
- Ne jamais ajouter de `concurrency` dans un workflow qui appelle `reusable-repository-governance.yml` : le réutilisable le porte déjà.
- PowerShell refuse `<` : poser un secret depuis un fichier avec `Get-Content -Raw f | gh secret set …`.
- Clés privées d'App : non retéléchargeables ; vérifier `Credentials → Key pairs` et le disque avant d'en générer.

## 2026-11-16 21:48 — Banc public H1–H8 (`bench/`) et `read_range` dans vault-core

**Objectif en cours** — Chantier banc livré (PR `feat/bench-h1-h8`) ; prochains : `vault-mount` (H4), delta (H6), encodage parallèle de la parité pour réarmer le seuil H5.
**Fait**
- `vault-core` : `Archive::chunk_span`, `read_range_into` (lecture d'une plage sans extraire), tests `crates/vault-core/tests/read_range.rs` ; CLI `tholos cat <archive> <chemin> --offset --len`.
- Crate `vault-bench` (`bench/`, binaire `tholos-bench`) : corpus synthétique déterministe (code, binaire, petits fichiers, doublons exacts/édités à taux contrôlé, parité), profils `ci`/`full` dans `bench/config.json`, contrat déclaré ↔ produit, bandes de validité des corpus (sortie 2), rapport JSON/CSV/markdown, exit 0/1/2.
- Verdicts profil `ci` : H1, H2, H3 soutenues ; H5 réparation 100 % sur 4 scénarios, **réfutée sur le coût de construction** (1,0–1,3 s/GB mono-thread, 5–10 × BLAKE3) → ADR-0003, `h5.build_vs_blake3` non bloquante avec champ `adr` (contrôle au chargement). H4/H6/H7/H8 `not_measured` avec prérequis.
- CI : surface `bench` dans `appfactory-impact.json`, job `bench` (ubuntu, profil ci, résumé de job + artefact `bench-ci`), non requis par le ruleset.
- Leçons : I-002 (corpus de doublons identiques → bande de validité), B-001 (réfutation H5), pointeur L-002 → métriques H3.
**Reste à faire**
1. Encodage parallèle par groupe de la parité, comparer à un encodeur GF(2⁸), puis remettre `h5.build_vs_blake3` en bloquant.
2. `vault-mount` (H4), delta (H6) ; `bench/results/reference-full.*` sur un hôte stable.
3. Ignorer/détracker `apps/landing/video/.thumbnails/` (noms trop longs, `git worktree add` échoue sous Windows).
**Pièges / contexte**
- Ne jamais relâcher un seuil : réfutation = ADR + lessons-learned + `blocking:false` + `adr`.
- Exécution complète du profil `ci` ≈ 2 min, plus si les corpus se régénèrent (I/O) ; lancer en arrière-plan.
- Les binaires `target/release/tholos*` ne sont pas rebuilds par `cargo test` : `cargo build -p vault-cli --release` avant un smoke test.

## 2026-11-16 22:48 — Parité : encodage parallèle par tranches de colonnes (vault-core)

**Objectif en cours** — Chantier « parité parallèle » livré (PR `feat/parite-parallele`) ; seuil H5 `h5.build_vs_blake3` **non réarmé** (marge insuffisante, voir ADR-0003 « Suivi »). Prochains : calibration BLAKE3 du banc sur le même volume, `vault-mount` (H4), delta (H6).
**Fait**
- `vault-core` : fonctionnalité `parallel` (rayon, activée par défaut) ; `build_section` découpe chaque groupe en tranches de colonnes de 512 o (ensemble de travail ≤ 1 MiB), tâches groupes × tranches sur le pool, un `ReedSolomonEncoder` réutilisé par travailleur, CRC en parallèle, écriture en place. Sortie identique octet pour octet (test `slicing_is_exact`, roundtrip/réparation inchangés) avec et sans la fonctionnalité.
- Mesures (32 MiB, 10 %) : 1,23 s/GB → 0,84 (1 fil), 0,33–0,38 (4 fils), 0,33–0,39 (16 fils). Rapport BLAKE3 : 6,2 / 2,3–3,5 / 2,2–2,5.
- Docs : ADR-0003 section « Suivi » (option GF(2⁸) écartée : k + m ≤ 256), leçon B-001 complétée, `bench/README.md`, README (`--no-default-features`). CI : `cargo test -p vault-core --no-default-features` sur ubuntu.
**Reste à faire**
1. Banc : calibrer BLAKE3 sur 32 MiB en mémoire (même régime que la construction) — à consigner dans l'ADR avant d'appliquer ; puis ≥ 5 exécutions `--only H5` à 4 fils, réarmer `blocking: true` si ≤ 2,5 partout.
2. Cœur : écrire les shards de secours directement dans la section ; paralléliser `reconstruct`.
3. `vault-mount` (H4), delta (H6) ; `bench/results/reference-full.*` ; détracker `apps/landing/video/.thumbnails/`.
**Pièges / contexte**
- La calibration BLAKE3 du banc varie de 0,11 à 0,18 s/GB d'une exécution à l'autre : le rapport `h5.build_vs_blake3` bouge de ±30 % sans changement de code. Juger la tendance sur `h5.build_s_per_gb`.
- `RAYON_NUM_THREADS=n` pour simuler un hôte à n cœurs ; `--only H5` ≈ 50 s.
- Encodeurs GF(2⁸) incompatibles avec les groupes de 1 024 shards sans changement de format.

## 2026-11-17 00:23 — Banc H5 : calibration BLAKE3 sur le volume protégé (en cours)

**Objectif en cours** — Réarmer `h5.build_vs_blake3` (≤ 3, non bloquant depuis ADR-0003) en rendant son dénominateur stable.
**Fait (non commité, dossier partagé)**
- `bench/src/theses/h5.rs` : BLAKE3 mono-fil du buffer protégé mesuré juste après chaque construction ; rapport = médiane build / médiane hash ; nouvelle métrique rapportée `h5.blake3_s_per_gb` (déclarée dans `bench/config.json`). La calibration 1 MiB de l'hôte reste pour H3.
- `bench/README.md` : paragraphe calibration mis à jour.
**Reste à faire**
1. Lire `scratchpad/h5runs/summary.txt` (5 × 4 fils, 1 × 16, 1 × 1) ; réarmer `blocking: true` seulement si ≤ 2,5 partout à 4 fils.
2. ADR-0003 : section « Suivi (2026-11-17 00:23) — calibration » avec le tableau ; lesson B-001 ; régénérer `bench/results/ci.*` (run complet ≈ 3,5 min, en arrière-plan) ; PR via plumbing (main protégée), CI, fusion, branche supprimée.
**Pièges** : binaire = `target/release/tholos-bench.exe` (pas vault-bench) ; `RAYON_NUM_THREADS=n` ; les seuils ne se relâchent jamais.

## 2026-11-17 00:31 — Banc H5 : seuil réarmé (PR en cours)

- Mesures 5 × 4 fils : rapport 1,82–2,06 (± 6 %) ; 16 fils 2,03 ; 1 fil 3,76. Run `ci` complet : sortie 0, `h5.build_vs_blake3` = 2,13 **bloquant**.
- `bench/config.json` : `blocking: true` (seuil ≤ 3 inchangé, champ `adr` conservé). ADR-0003 « Suivi (2026-11-17) », B-001, `bench/README.md`, `bench/results/ci.*` à jour.
- Reste : PR `bench/calibration-h5` → CI → fusion ; puis chantiers `vault-mount` (H4), delta (H6), shards de secours écrits en place + `reconstruct` parallèle.

## 2026-11-17 00:37 — Chantier vault-mount (H4) démarré

- Choisi par le commanditaire après la fusion de la PR 7 (b9d00e1). Rien de modifié encore ; cadrage en cours (crate `vault-mount` absent du workspace : seul `crates/vault-mount/README.md` existe ; H4 `not_measured` dans `bench/config.json`, surface `mount` déjà déclarée dans `.github/appfactory-impact.json`).
- Plan : cœur OS-agnostique testable sans pilote (table d inodes, lookup/readdir/read sur `Archive::read_range_into`), adaptateurs `fuser` (unix) / WinFsp (windows) derrière des fonctionnalités Cargo, `tholos mount`, thèse H4 mesurée en CI ubuntu (fuse3), ignorée proprement sans pilote.

## 2026-11-17 09:28 — Balayage de secours du trailer (section de parité, disposition 1)

- La section de parité embarque une copie du trailer (128 o) entre son en-tête et les tables de CRC ;
  octet 21 de l'en-tête = disposition (1 = copie présente, 0 = sections antérieures, toujours lues).
- Lecteur : si les deux copies de queue sont illisibles, `locate_trailer` remonte la fin du fichier à la
  recherche de `THOLOSPR`, valide en-tête + copie, et exige `P = position` et `P + parity_len = fin − 256`.
  L'archive s'ouvre et se lit ; `repair` restaure les deux copies de queue.
- Writer : le trailer est encodé avant la section (sa longueur ne dépend que de la géométrie) ;
  `build_section` prend `Option<&[u8; 128]>` (le banc H5 passe `None`).
- Tests : copies détruites → lisible et réparé ; + en-tête de section abîmé → `NoValidTrailer` sans écriture ;
  sans parité → `NoValidTrailer` ; faux magique dans les données ignoré. Le test v0 de la réplique utilise
  explicitement les options sans parité.
- Reste : répliquer l'en-tête et les tables de CRC de la section (une section à en-tête abîmé perd sa parité
  et sa copie de trailer).

## 2026-11-17 09:31 — PR #8 ouverte (balayage trailer)

- `main` est protégée (PR obligatoire) : commit `f19f375` poussé sur `feat/trailer-balayage-parite`,
  PR https://github.com/Trigenys/tholosvault/pull/8. Le `main` local est en avance d'un commit sur
  `origin/main` tant que la PR n'est pas fusionnée (ne pas reset ; après fusion, `git pull --rebase`
  ou réalignement manuel si squash).
- Prochaine étape candidate : répliquer l'en-tête et les tables de CRC de la section de parité.

## 2026-11-19 23:17 — Chantier vault-mount (H4) : banc en cours, rien commité

- Fait (non commité, dossier de travail) : crate `crates/vault-mount` (cœur VaultFs testé sans pilote,
  adaptateur FUSE derrière la feature `fuse`, sonde `probe()`), `tholos mount` dans vault-cli,
  `bench/config.json` H4 mesurée avec champ `requires: mount`, `bench/src/report.rs` (capacités d'hôte,
  métriques sautées → verdict `incomplete`), `bench/src/theses/h4.rs`, `main.rs` (`--require-env`).
- Reste : compiler/tester le banc (Windows : H4 `incomplete`, code 0) ; CI ubuntu avec fuse3 et
  `--require-env mount` ; impact json ; docs (README vault-mount, bench/README, lessons-learned,
  stack.md) ; preuve Linux via WSL Ubuntu (`wsl -d Ubuntu -e bash -lc 'source ~/.cargo/env; ...'`,
  `CARGO_TARGET_DIR=/tmp/vault-target`) ; branche `feat/vault-mount` par plumbing, PR, fusion.
- Règle : jamais assouplir un seuil ; une réfutation en CI passe par ADR.

## 2026-11-19 23:40 — vault-mount (H4) : vérifié sous Windows, preuve Linux en cours

- Windows : fmt, clippy workspace -D warnings, tests workspace, build release : verts. Banc H4 seul :
  `h4.core_read_ratio` 0,95 (seuil 0,9), montage sauté → verdict `incomplete`, code 0.
- CI : fuse3 installé sur ubuntu, tests `vault-mount` (sans feature + `fuse_mount --ignored`), banc avec
  `--require-env mount`. Impact : `cli`/`bench` dépendent de `mount`, `mount` déclenche `bench`.
- Docs faites : `crates/vault-mount/README.md`, `bench/README.md` (H4, `requires`), lessons I-003,
  stack.md. Reste : lire `scratchpad/wsl2.log` (clippy, tests, montage réel, banc H4 avec montage dans
  WSL Ubuntu ; script `wsl_check.sh`, lancer via `wsl -d Ubuntu -e bash -c 'bash /mnt/c/...'`), puis
  branche `feat/vault-mount` par plumbing (index temporaire), PR avec RAIDER review, CI, fusion.
- Piège : `/mnt/c/Users/lawry/.cargo/config.toml` impose `sccache` à cargo WSL → `CARGO_BUILD_RUSTC_WRAPPER=`.

## 2026-11-20 05:32 — vault-mount (H4) : PR #8 fusionnée, ouverture de la PR vault-mount

- PR #8 (balayage du trailer) fusionnée dans `main` (`ce73451`) ; HEAD local réaligné, arbre identique.
- Preuve Linux locale impossible : WSL Ubuntu sans `libc6-dev` (sudo à mot de passe). La CI ubuntu de la
  PR fournit la première compilation de `fuse.rs`, le montage réel et la mesure H4 avec pilote.
- Étape en cours : branche `feat/vault-mount` par plumbing (index temporaire), push, PR avec le corps
  `scratchpad/pr_body.md` (RAIDER review), surveillance CI, fusion. Si H4 est réfutée en CI : ADR, jamais
  de seuil assoupli en silence.

## 2026-11-20 05:55 — PR #9 vault-mount ouverte, correctif Linux poussé

- PR #9 `feat/vault-mount` : https://github.com/Trigenys/tholosvault/pull/9 (corps = RAIDER review).
  Premier passage CI : windows vert, ubuntu rouge sur E0597 dans `probe.rs` (recherche de `fusermount`
  par référence dans l'expression finale). Corrigé (`into_iter`), commit `38c268e`, CI relancée.
- Vérification Linux locale impossible : WSL sans libc6-dev (même les build scripts ne lient pas) ;
  cible croisée depuis Windows bloquée par `zstd-sys` (pas de gcc croisé). La CI ubuntu reste la preuve.
- Prochaine étape : lire les checks de la PR #9 ; si verts → `gh pr merge 9 --merge --delete-branch`,
  `git fetch`, `git read-tree origin/main`, `git update-ref HEAD <new> <old>`, rapport final RAIDER.
  Si H4 réfutée en CI → ADR, pas de seuil assoupli.

## 2026-11-20 05:47 — ADR-0004 : réfutation H4 consignée, commit 631dee6 poussé sur la PR #9

- CI ubuntu (run 44675746318) : `dead_code` sur `not_compiled` (corrigé par `cfg`) et **H4 réfutée** :
  `h4.mount_read_ratio` 0,18 pour un seuil ≥ 0,7 (cœur 0,99, contenu exact, ouverture 0,69 ✓).
- Décision : seuil inchangé, métrique non bloquante + `adr` → `docs/adr/0004-debit-lecture-montage-fuse.md`
  (cause présumée : `ChunkCache` sans vol unique sous requêtes concurrentes du noyau). B-002 dans
  lessons-learned, README du banc, corps de PR mis à jour.
- Local Windows : fmt, tests `vault-bench` (14), clippy verts. Banc H4 local bruité (cœur 0,85 / 0,83 /
  0,53 contre 0,95 plus tôt, charge machine ~27 %) : `h4.core_read_ratio` est sensible à la charge car
  référence et cœur sont mesurés dans des boucles séparées ; la CI ubuntu (0,99) reste la référence.
  À surveiller dans le chantier de suivi (mesurer en alternance).
- Suite : CI de la PR #9 en cours (`gh pr checks 9 --watch`) ; si verte → merge, réalignement HEAD.

## 2026-11-20 05:50 — PR #9 vault-mount fusionnée (30d7a38), chantier vault-mount PR 1 clos

- CI run 44676728561 verte (impact, rust ubuntu+windows, bench). H4 reproduite : cœur 0,99, ouverture
  0,66, `h4.mount_read_ratio` **0,186** (non bloquante, ADR-0004). HEAD local réaligné sur origin/main
  sans toucher au work tree ; branche `feat/vault-mount` supprimée (locale et distante).
- Reste : `PASSATION-vault.md` modifié non commité ; `.claude/` non suivi.
- Chantiers suivants : (1) « lecture séquentielle à travers FUSE » (vol unique dans `ChunkCache`,
  `--threads 1`, réponse sans copie, mesure alternée référence/cœur) pour réarmer `blocking: true` ;
  (2) adaptateur WinFsp (feature `winfsp`, PR 2).

## 2026-11-20 08:52 — ADR-0005 : téléchargement segmenté conscient du format (cadrage, pas de code)

- Nouveau : `docs/adr/0005-telechargement-segmente.md` (statut proposé) : crate `vault-fetch` (cœur agnostique
  du transport) + `vault-fetch-http` (reqwest), `vault-fetch-s3` plus tard ; segments sur frontières de
  morceaux, BLAKE3 par morceau à l'arrivée (seul le morceau corrompu est redemandé), reprise par bitmap
  `.part`, trailer puis tables avant tout contenu (`Range: bytes=-256`), lecture/montage partiel avec
  priorisation à la demande (V1), racine attendue `--root`. Hors périmètre : torrent, navigateur, upload.
- Reuse-first : `trauma` / `swiftdl` / `prism` Learn, `reqwest` Adopt, `aria2` écarté (détail dans l'ADR et
  `docs/produit/stack.md`). Cadrage : surface, écran desktop, matrice §7, D12, §9 mis à jour.
- Prérequis code (PR séparée, après le chantier débit FUSE) : abstraction de source de morceaux dans
  `vault-core` ; rien ne touche `vault-mount` ici.
- Branche `docs/adr-0005-telechargement` créée par plumbing (HEAD et work tree intacts).

## 2026-11-20 09:08 — Recherche : mécanique, histoire et goulots du téléchargement

- Ajout de `docs/recherche/telechargement-histoire-goulots.md` (BLUF, mécanique TCP/HTTP/QUIC/CDN,
  chronologie FTP → Xet, goulots, bornes, tendances 2025–2026, implications pour `vault-fetch`, 61 sources).
- L'ADR-0005 renvoie à ce rapport dans sa section « Veille en ligne ».
- Commit 6d63056 poussé sur `docs/adr-0005-telechargement` (PR #10), par-dessus la séance de vision produit (b9def6a).
- Reste : relecture et fusion de la PR #10 par le propriétaire.
## 2026-11-20 09:12 — vault-mount (H4) : vol unique du cache, banc H4 alterné, réarmement en cours

- Worktree `happy-almeida-9933ff`, branche `claude/happy-almeida-9933ff` depuis `main` (`30d7a38`) ; rien
  n'est encore committé. Objectif : `h4.mount_read_ratio` ≥ 0,7 en CI (0,18 mesuré, ADR-0004), réarmer
  `blocking: true` sans toucher au seuil.
- Fait : `ChunkCache::get_or_decode` (vol unique par morceau, `CacheStats`, 7 tests dont 8 lecteurs
  concurrents → 1 décodage) ; `read_range_into_cached` l'utilise ; `vault-mount` : `CACHE_CHUNKS` 16 → 64,
  `MountOptions.cache_chunks`, `ReadStats` via `MountHandle::fs()`, tampon de réponse par fil dans
  `fuse.rs::read` ; `tholos mount --cache-chunks` ; banc H4 : référence mesurée en alternance, cœur
  sous concurrence (`h4.core_concurrent_*`, `h4.threads`), montage à 1 fil (`h4.mount_read_ratio_1thread`),
  `h4.mount_req_kib`, `h4.mount_decode_ratio` ; config et test `report.rs` à jour ; fmt/clippy/tests
  Windows verts.
- Labo Linux : `scratchpad/lab.sh` (Docker `rust:1-bookworm`, `/dev/fuse`, volumes `tholos-cargo-*`).
  Docker Desktop (exe dans `%LOCALAPPDATA%\Programs\DockerDesktop`) est mort deux fois pendant la
  compilation ; relancé avec `--memory=8g --cpus=6`, résultat dans `scratchpad/lab3.log`.
- Reste : lire lab3, pousser la branche + PR (RAIDER review), lire la mesure CI ; si ≥ 0,7 : `blocking: true`,
  ADR-0004 → clos avec avant/après, B-002 suivi, `bench/README.md`, passation, fusion, rapport au parent.

## 2026-11-20 10:23 — vault-mount (H4) : diagnostic du trajet FUSE, no_open, direct_io

- **Diagnostic (labo Docker `tholos-lab`, WSL2 6.18, 6 cœurs, sonde temporaire
  `crates/vault-mount/examples/h4probe.rs`, à supprimer avant commit)** : la perte de
  `h4.mount_read_ratio` ne vient pas du décodage (vol unique OK, `decode_ratio` = 1) mais du
  trajet noyau : gros fichiers (16 × 2 MiB) ratio ≈ 0,5 (le noyau découpe en requêtes de 128 KiB,
  séquentielles, ~110 µs chacune, parallélisme sans effet) ; petits fichiers (2167 × 1–64 KiB)
  ratio ≈ 0,05 (≈ 300 µs d'allers-retours par fichier : lookup + open + read + release, contre
  ≈ 15 µs de décodage). Le corpus `code` du banc domine l'agrégat → 0,09–0,18.
- **Fait** : `core_concurrent_ns` du banc réécrit en pool de fils persistant (le motif
  « fils relancés par fenêtre » était un artefact du banc, 0,19) ; `fuse.rs` : `open` répond
  `ENOSYS` quand le noyau annonce `FUSE_NO_OPEN_SUPPORT` (plus d'open/release par fichier),
  capacités `FUSE_PARALLEL_DIROPS` + `FUSE_ASYNC_DIO` ; option `MountOptions.direct_io`
  (`FOPEN_DIRECT_IO`, requêtes de 1 MiB) à mesurer (pas encore exposée dans la CLI).
- **En cours** : sonde v3 (default / 1thread / direct_io) dans le labo
  (`bash scratchpad/lab.sh "cargo run -q --release -p vault-mount --example h4probe"`).
- **Décision à prendre (parent/utilisateur)** : la métrique agrégée ne peut pas atteindre 0,7 sur le
  corpus `code` (plancher = latence d'un aller-retour FUSE, pas tholosvault). Options : scinder la
  métrique (débit en flux sur `binary`, latence par fichier sur `code` avec seuil propre) ou garder
  H4 réfutée. Ne pas relâcher le seuil.
- Non commité : `.claude/sessions.json` (local), sonde. Branche de travail : `claude/happy-almeida-9933ff`.

## 2026-11-20 12:10 — vault-mount (H4) : chantier « lecture séquentielle à travers FUSE », PR ouverte
- Fait : vol unique `ChunkCache::get_or_decode` (+ compteurs, test concurrent), tampon de réponse par fil,
  cache 64 morceaux, `FUSE_NO_OPEN_SUPPORT` (open → ENOSYS, 1,5 × sur petits fichiers), `FUSE_PARALLEL_DIROPS`,
  `core_concurrent_ns` en pool persistant, métriques `h4.mount_decode_ratio`, `h4.mount_req_kib`,
  `h4.mount_read_ratio_{1thread,code,binary}`, `h4.core_concurrent_{read,decode}_ratio` (décodage ≤ 1,05 bloquant).
- Mesuré et rejeté : `FOPEN_DIRECT_IO`/`FUSE_ASYNC_DIO` (code retiré) ; la sonde `examples/h4probe.rs` est supprimée.
- Verdict : `h4.mount_read_ratio` ne peut pas atteindre 0,7 (plancher de latence FUSE par requête ; corpus `code`
  dominant). Seuil inchangé, toujours `blocking: false`. Détail et deux issues possibles : ADR-0004, section finale.
- Preuves : Windows fmt/clippy/tests banc verts ; labo Linux (WSL2) tests vault-core/vault-mount + test FUSE ignoré
  verts, banc H4 exécuté (labo bruité : `core_read_ratio` 0,74, `open_p99` 4,3 — CI ubuntu fait foi).
- Suite : lire les chiffres CI de la PR, les reporter dans l'ADR, puis décision (scinder la métrique ou garder H4
  réfutée) à prendre avec la session mère.

## 2026-11-20 12:18 — vault-mount (H4) : PR #12 ouverte, CI verte, chiffres reportés dans l'ADR
- PR https://github.com/Trigenys/tholosvault/pull/12 (`feat/fuse-vol-unique`, main fusionné). CI ubuntu/windows/bench vertes.
- CI ubuntu (run 44726058404) : `mount_read_ratio` 0,21 (avant 0,18), `_code` 0,19, `_binary` 0,28, `_1thread` 0,23,
  `decode_ratio` 1,00, `core_read_ratio` 0,96, `open_p99` 0,62. Tableau avant/après ajouté à l'ADR-0004.
- Verrous : seuil ≥ 0,7 inchangé et non bloquant ; aucune décision prise sur la scission de la métrique.
- Prochaine étape : réponse de la session mère (scinder la métrique ou garder H4 réfutée), puis revue/merge de la PR.

## 2026-11-20 12:22 — vault-mount (H4) : option (ii) retenue, thèse H4 reformulée dans la PR #12
- Décision de la session mère : pas de scission de métrique (seuil tiré de la mesure = option 1 refusée). H4 reformulée
  (titre `bench/config.json`, `bench/README.md`, en-tête `h4.rs`) : « le cœur lit à la vitesse du décodeur ; à travers
  le noyau, débit borné par la latence par requête ». Métriques, seuils, verdict inchangés ; `decode_ratio` ≤ 1,05 reste bloquant.
- Piste ouverte, PR séparée : thèse H4bis (coût par fichier FUSE, seuil a priori contre un passthrough nul, non bloquante 3 runs CI).
- Prochaine étape : vert CI de la PR #12, puis revue/merge. Rien d'autre attendu de cette session.


## 2026-11-20 12:28 — vault-fetch MVP : `tholos fetch` (ADR-0005, thèses H9a/H9e)

- Branche `claude/musing-bose-8702db` (worktree `.claude/worktrees/musing-bose-8702db`), PR à ouvrir vers `main`.
- Écrit : `crates/vault-fetch` (plan par morceaux, `.part` atomique, BLAKE3 par morceau via `vault-core`,
  seau à jetons, événements ; 7 tests unitaires verts), `crates/vault-fetch-http` (reqwest + rustls/`ring`,
  sonde HEAD/Range, ETag, Retry-After, repli ; serveur de test axum sous la feature `test-server`),
  `tholos fetch` (`--out --root --connections --limit --no-parity --plan --restart`), 10 tests d'intégration
  `crates/vault-fetch-http/tests/h9.rs`, thèse H9 au banc (`bench/src/theses/h9.rs`, `config.json`, README),
  `docs/recherche/these-h9-telechargement.md`, ADR-0006, `appfactory-impact.json` (surface `fetch`).
- Reuse-first : `trauma` Learn (découpage API/relances), `reqwest` Adopt (fournisseur `ring` : pas de cmake/NASM
  sur l'hôte), `axum` Adopt (serveur de test), `aria2`/`winfsp` écartés.
- Verrou : disque C: plein (≈ 200 Mo libres) ; `vault-fetch-http`, CLI, tests et banc ne sont **pas encore
  compilés**. Candidats à libérer : `target/` du dépôt principal (2,3 Go), `target/` des worktrees voisins,
  `AppData/Local/Temp` (5,8 Go) — décision du propriétaire.
- Prochaine étape : libérer l'espace, `cargo test -p vault-fetch-http`, ajuster les assertions, `--only H9`
  en local pour compléter l'ADR-0006, `fmt`/`clippy -D warnings`/`test --workspace`, commit, push, PR avec
  revue RAIDER.

## 2026-11-20 12:35 — vault-fetch MVP : état après type-check

- `cargo check` offline vert sur `vault-fetch-http` (feature `test-server`), `vault-cli`, `vault-bench` (H9 câblée).
- Disque libéré en supprimant `~/.cargo/registry/src` (cache d'extraction, régénéré par cargo) ; il reste ≈ 300 Mo,
  insuffisant pour l'édition de liens des tests du workspace. Décision du propriétaire attendue sur `target/`
  du dépôt principal (2,3 Go) / worktrees voisins / `AppData/Local/Temp` (5,8 Go).
- Test `source_without_ranges_falls_back_to_single_stream` corrigé : `bytes_received == len` (la sonde 200 ne
  lit pas le corps).
- Prochaine étape : `cargo test -p vault-fetch-http --features test-server` (en cours), puis workspace,
  clippy, banc `--only H9`, commit, push, PR.


## 2026-11-20 12:52 — vault-fetch MVP : tests verts, cause racine de la coupure masquée

- Disque libéré par le propriétaire (≈ 8,7 Go) : `cargo test --workspace` offline vert (94 tests, dont 10
  d'intégration `crates/vault-fetch-http/tests/h9.rs`), `cargo fmt --check` vert, clippy relancé après une
  correction `clone_on_copy` (`fetcher.rs`).
- Cause racine des 3 tests rouges : hyper (serveur) lit le flux avant d'écrire l'en-tête, donc une coupure
  `[Ok, Err]` fermait la connexion sans réponse, et reqwest 0.13 réémettait la requête en silence (couche de
  relance par défaut) → `cuts: 0`. Correctifs : `Client::builder().retry(reqwest::retry::never())` dans
  l'adaptateur ; `sleep` 20 ms entre octets servis et réinitialisation dans le serveur de test ; borne
  `content_gets` = début des tables (`trailer().chunk_index_off`) dans le test et le banc. Leçon I-005.
- ADR-0005 passé à « accepté » (MVP livré). Corps de PR prêt (RAIDER review, reuse-first).
- Prochaine étape : résultat clippy, `cargo run -p vault-bench --release -- --only H9` pour compléter
  l'ADR-0006, commit (sans attribution), push `claude/musing-bose-8702db`, PR vers `main`.

## 2026-11-20 13:02 — Renumérotation ADR et mesures H9

- L'ADR sur la métrique de débit non bloquante est **ADR-0008** (`docs/adr/0008-debit-agrege-telechargement.md`),
  pas 0006 : la PR #11 (WinFsp) occupe déjà 0006 et 0007. Références mises à jour (`bench/config.json`,
  ADR-0005, thèse H9). Les entrées précédentes qui citent « ADR-0006 » désignent ce fichier.
- Banc `--only H9` (release, profil `ci`, 32 MiB) : `corrupt_chunk_cost` 1, `resume_rebytes` 0,
  `root_mismatch_rejected` 1, `aggregate_throughput_ratio` 0,31 (non bloquant, consigné dans l'ADR-0008 avec
  les causes à mesurer ; seuil 0,9 inchangé). Sortie du banc : succès.
- Suite : commit, push, PR vers `main`.

## 2026-11-20 13:04 — PR #13 ouverte

- Commit `325d963` poussé sur `claude/musing-bose-8702db`, PR https://github.com/Trigenys/tholosvault/pull/13
  vers `main` (sections Objet / Changements / Reuse-first / RAIDER review / Vérification locale).
- CI impact-aware déclenchée (gates `rust` ubuntu + windows, `bench` profil ci) : en cours à l'ouverture,
  résultat à relever dans la PR.
- Hors périmètre, à ouvrir ensuite : lecture / montage d'une archive incomplète (H9b, H9d) ; verdict de thèse
  du banc qui marque « refuted » une thèse dont seule une métrique non bloquante est hors cible.

## 2026-11-20 13:08 — Branche renommée, PR #14 remplace la #13

- Branche `feat/vault-fetch-mvp` (l'ancienne `claude/…` supprimée du dépôt distant : son nom aurait été repris
  dans le commit de fusion). PR https://github.com/Trigenys/tholosvault/pull/14 vers `main`, même corps ;
  PR #13 fermée en la référençant.
- Prochaine étape : relever la CI de la #14, fusionner ; puis H9b/H9d dans une PR séparée.

## 2026-11-23 17:02 — PR #14 réalignée sur main (après fusion de la PR #12)

- `origin/main` (24f173a, vol unique FUSE) fusionné dans `feat/vault-fetch-mvp` : conflits résolus sur
  `PASSATION-vault.md` (les deux séries d'entrées, ordre chronologique) et `crates/vault-cli/src/main.rs`
  (`Mount` avec `cache_chunks` + `Fetch`) ; `bench/config.json`, `report.rs`, `vault-core`, `lessons-learned.md`
  fusionnés automatiquement, aucun seuil modifié.
- Vérification locale après fusion : fmt OK, clippy `-D warnings` OK, `cargo test --workspace` 99 tests, 0 échec.
- Prochaine étape : fusion de la #14 par le propriétaire une fois CLEAN et CI verte.

## 2026-11-20 11:07 — vault-mount : adaptateur WinFsp (feature `winfsp`)
- Worktree `.claude/worktrees/winfsp`, branche `feat/vault-mount-winfsp` (base 30d7a38). Rien de commité.
- Fait : `crates/vault-mount/src/winfsp.rs` (FileSystemContext → VaultFs, FsError → NTSTATUS, volume RO
  insensible à la casse, racine sans `.`/`..`), `probe.rs` (registre `HKLM\SOFTWARE\WinFsp` → LoadLibraryW
  → `winfsp_init`), `build.rs` (delayload), `lib.rs` (branche WinFsp de `mount`/`MountHandle`),
  `tests/winfsp_mount.rs` (`#[ignore]`), vault-cli `default = ["fuse","winfsp"]`, bench (feature + capacité
  `winfsp` manquante), CI windows (choco winfsp 2.2.26323, test ignoré, bench matrice ubuntu/windows,
  artefact `bench-ci-<os>`), README crate / bench, stack.md.
- Verrou : compilation `--features winfsp` exige libclang (bindgen) ; installation LLVM + WinFsp via winget
  acceptée par l'utilisateur, en cours.
- Licence : winfsp-rs GPL-3.0 vs workspace Apache-2.0 → à consigner dans la RAIDER review de la PR.
- Prochaine étape : `cargo test -p vault-mount --features winfsp` (+ `-- --ignored`), clippy `-D warnings`,
  fmt, commit, rebase main, PR, CI verte, fusion ; si H4 réfutée sous Windows : ADR, pas de seuil relâché.

## 2026-11-20 11:20 — vault-mount : adaptateur WinFsp, état vérifié

- Correction de l'entrée précédente : LLVM n'a **pas** été installé sur le poste. `libclang`
  (nécessaire à `winfsp-sys`/bindgen) vient d'une roue PyPI `libclang` 18.1.1 décompressée dans le
  scratchpad de session, exposée par `LIBCLANG_PATH` ; à refaire (ou installer LLVM) pour compiler la
  feature `winfsp` dans une session neuve. WinFsp 2.2.26323 est installé (winget).
- Vérifié en local : `cargo test -p vault-mount --features winfsp` vert ; test réel
  `winfsp_mount -- --ignored` vert (montage noyau, lecture, insensibilité à la casse, écriture refusée,
  démontage) ; clippy `-D warnings` workspace et `vault-mount` sans feature verts ; `cargo fmt` propre.
- Correctif découvert par le test réel : WinFsp refuse un dossier de montage existant
  (`STATUS_OBJECT_NAME_COLLISION`) ; `mount_target` supprime le dossier vide avant montage.
- Licence : `winfsp-rs` est GPL-3.0 → ADR-0006 (proposé, options sans décision), leçon I-005 ;
  licence du workspace inchangée.
- Reste : bench H4 profil `ci` sous Windows, commit, rebase sur `main`, PR, CI.

## 2026-11-20 11:33 — vault-mount : WinFsp, liaison différée, H4 sous Windows

- Régression trouvée par le banc : `tholos.exe` et `tholos-bench.exe` importaient `winfsp-x64.dll`
  en dur (`cargo:rustc-link-arg` ne se propage pas depuis `vault-mount`). Correctif : `build.rs` par
  binaire (`vault-cli`, `bench`) appelant `winfsp::build::winfsp_link_delayload()`, dépendance de
  build `winfsp` (`delayload`) sous Windows ; vérifié sur la table d'imports différés et par
  `tholos --version`. Garde-fou CI : `tholos --version` **avant** l'installation de WinFsp. Leçon B-003.
- H4 sous Windows (poste, WinFsp 2.2.26323) : cœur et contenu ✓, `mount_open_p99_over_blake3_mib`
  4,8 ✗, `mount_read_ratio` 0,05 ✗. ADR-0007 (accepté) : exemption déclarée `"non_blocking_on":
  ["windows"]` avec `adr` obligatoire, seuil inchangé et toujours bloquant sous Linux. Banc : champ
  `non_blocking_on`, `MetricSpec::blocking_on(os)`, `Environment.os`, `validate()` ; 16 tests verts.
- Bruit d'hôte : `h4.core_read_ratio` varie 0,79–1,22 selon l'exécution sur le poste ; la référence
  est la CI (`bench-ci-windows`). Ne pas assouplir.
- Vérifié : `cargo fmt --check`, clippy `-D warnings` workspace et `vault-mount` sans feature,
  `cargo test --workspace` verts (LIBCLANG_PATH vers la libclang portable du scratchpad).
- Suite : commit, rebase sur `main` (ADR-0005 fusionnée entre-temps), PR, CI.

