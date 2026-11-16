# ADR-0002 — Nom public « tholosvault », éditeur Trigenys Group

- **Statut** : accepté
- **Date** : 2026-11-16
- **Contexte** : « VAULT » seul est générique (HashiCorp Vault, « vault » Obsidian) ; le binaire `vault` est déjà celui
  de HashiCorp. Le propriétaire souhaite une racine grecque ou latine ajoutée à « vault », et un format open core qui
  doit paraître neutre pour être adopté (ADR-0001).
- **Options considérées** : noms composés anglais (chunkvault, vaultfs : sans collision forte mais `.com` pris) ;
  racines gréco-latines (tholosvault, arxvault, custosvault, aerariumvault…) ; nom préfixé par l'éditeur (trigenys-vault).
  Vérifications : dépôts GitHub homonymes, organisation GitHub, crates.io, npm, DNS `.com/.io/.dev/.app` (voir `docs/produit/nom.md`).
- **Décision** : nom public **tholosvault** (θόλος : édifice à voûte, trésor de Delphes), sans préfixe éditeur ;
  signature « by Trigenys Group » sur la landing, les stores, le README et les en-têtes de licence. Commande CLI : `tholos`.
  « VAULT » reste le nom de travail interne (dépôt, crates `vault-*`).
- **Conséquences** : pas d'achat de domaine, le produit est servi en sous-domaine de `trigenys.com`
  (`tholosvault.trigenys.com`, console sur `console.tholosvault.trigenys.com`) ; réserver org GitHub, crate et paquet npm avant toute
  communication ; contrôle de marque INPI / EUIPO ; le renommage des crates en `tholos-*` est possible plus tard par un ADR.
- **Dérogation RAIDER** : aucune.
