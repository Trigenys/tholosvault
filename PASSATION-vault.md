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
