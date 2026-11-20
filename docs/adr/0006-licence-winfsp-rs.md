# ADR-0006 — Licence de l'adaptateur WinFsp (`winfsp-rs`, GPL-3.0)

- **Statut** : proposé (décision du fondateur attendue)
- **Date** : 2026-11-20
- **Contexte** : `vault-mount` expose le montage Windows derrière la feature `winfsp`
  (`crates/vault-mount/src/winfsp.rs`), en adoptant le crate `winfsp-rs` 0.13 (SnowflakePowered,
  retenu dans `docs/produit/stack.md`). Ce crate et son `winfsp-sys` sont publiés sous **GPL-3.0**,
  alors que le cœur, la CLI et `vault-mount` sont sous **Apache-2.0** (ADR-0001) et que les apps
  (desktop, mobile, console) sont propriétaires. Le pilote WinFsp lui-même est sous **GPLv3 avec
  exception FLOSS** : lier dynamiquement `winfsp-x64.dll` depuis un programme sous licence libre
  approuvée (Apache-2.0 en fait partie) est explicitement permis ; l'exception couvre la DLL, pas le
  crate Rust `winfsp-rs`, qui est une œuvre tierce sous GPL pleine. Lié statiquement dans un binaire
  `tholos` ou dans l'app desktop, `winfsp-rs` impose la GPL-3.0 à ce binaire.
  L'adaptateur est pur adaptateur : toute la logique de format reste dans `vault-core`, et le module
  WinFsp (~450 lignes) ne fait que traduire `VaultFs` vers les rappels de WinFsp.
- **Options considérées** :
  1. **Feature optionnelle, désactivée dans les binaires publiés.** `winfsp` reste compilable pour
     le développement, les tests et le banc (CI `windows-latest`), mais les binaires distribués
     (`tholos`, desktop) sont construits sans. Pour : aucun changement de licence, décision
     réversible, le code livré par cette PR reste utile. Contre : pas de montage sur Windows pour
     les utilisateurs finaux tant que l'option 2 ou 3 n'est pas prise ; H4 n'est mesurée sous
     Windows que par la CI.
  2. **Liaison directe à la DLL WinFsp sans `winfsp-rs`.** Réécrire le module sur les symboles
     de `winfsp-x64.dll` (déclarations `extern "C"` à la main ou `bindgen` sur les en-têtes du
     SDK, structures `FSP_FSCTL_VOLUME_PARAMS`, `FSP_FILE_SYSTEM_INTERFACE`), sous l'exception
     FLOSS du pilote. Pour : le binaire reste Apache-2.0 ; plus de dépendance à `libclang` à la
     compilation si les déclarations sont écrites à la main. Contre : réécriture d'environ 600 à
     900 lignes `unsafe` (table de rappels, descripteurs de sécurité, `delayload`), maintenance
     des structures à chaque version de WinFsp, perte des garde-fous de `winfsp-rs`. Le code
     actuel (traduction `VaultFs` → sémantique WinFsp) se transpose presque tel quel, seule la
     couche d'appel change.
  3. **Accepter la GPL-3.0 pour le binaire Windows.** Publier `tholos` Windows (et l'app desktop
     Windows si elle embarque le montage) sous GPL-3.0, le reste du dépôt restant Apache-2.0 et les
     apps propriétaires ailleurs. Pour : rien à réécrire, montage Windows livré immédiatement.
     Contre : incompatible avec une app desktop Windows propriétaire embarquant `vault-mount`
     (ADR-0001) ; licence par plateforme, à expliquer aux utilisateurs et aux contributeurs.
  4. **Processus séparé.** Un exécutable auxiliaire GPL (`tholos-mount-win.exe`) piloté par IPC
     depuis l'app propriétaire. Pour : isole la GPL. Contre : la frontière « programme séparé »
     est discutable quand l'auxiliaire n'a de sens qu'avec l'app ; complexité d'exploitation.
- **Décision** : **non prise** ; cet ADR pose les options pour le fondateur. En attendant, l'état du
  dépôt correspond à l'option 1 par construction : `winfsp` n'est pas une feature par défaut de
  `vault-mount` ; elle est activée dans `vault-cli` et `bench` pour les tests et le banc, et aucun
  binaire n'est encore publié. **Avant la première publication de binaires Windows**, cette décision
  doit être prise et consignée ici (statut `accepté`, option retenue).
- **Conséquences** : la licence du workspace (`Cargo.toml`, `LICENSE`) n'est pas modifiée par
  l'adaptateur ; la PR qui l'introduit cite cet ADR dans sa revue RAIDER ; le README de
  `vault-mount` signale la licence de `winfsp-rs`. Si l'option 2 est retenue, le travail est un
  chantier `vault-mount` dédié (remplacement de la couche d'appel, même test `winfsp_mount`).
- **Dérogation RAIDER** : aucune à ce stade (décision en attente, explicite et visible). Une
  publication de binaires Windows sans décision serait une dérogation à E (contrats explicites).
