# ADR-0001 — Modèle open core et licence Apache-2.0

- **Statut** : accepté
- **Date** : 2026-11-16
- **Contexte** : un format d'archive n'est adopté que s'il est lisible sans dépendre d'un éditeur (pérennité des
  données, intégration par des tiers, audit de la parité et du Merkle). Le rapport de recherche note qu'une fenêtre
  pour qu'un format « FastCDC + BLAKE3 + RS + mount » s'impose se referme vers 2028. La valeur commerciale est dans
  l'expérience (apps desktop/mobile, console, partage, hébergement), pas dans le format lui-même.
- **Options considérées** :
  - Open core : format, `vault-core`, `vault-cli` et `vault-mount` libres ; apps et console payantes. Pour : adoption
    du format, confiance, contributions. Contre : un tiers peut construire une app concurrente sur le cœur.
  - Tout libre + services : revenu sur hébergement et support seulement. Pour : simplicité. Contre : revenu plus incertain
    tant que le partage hébergé n'existe pas (V1 → plus tard).
  - Propriétaire : contrôle total. Contre : un format fermé n'a pas de raison d'être préféré à squashfs/DwarFS/EROFS.
  - Licences : Apache-2.0 (permissive, clause brevets, standard de l'écosystème Rust et de zstd) ; MPL-2.0 (copyleft
    par fichier) ; AGPL-3.0 (copyleft réseau, freine l'intégration).
- **Décision** : open core ; la spécification du format et les crates `vault-core`, `vault-cli`, `vault-mount` sont
  publiés sous **Apache-2.0**. Les apps (`apps/desktop`, `apps/mobile`) et la console (`apps/web`, `crates/vault-api`)
  restent propriétaires jusqu'à décision contraire par un nouvel ADR.
- **Conséquences** : fichier `LICENSE` Apache-2.0 à la racine ; chaque crate libre déclare `license = "Apache-2.0"` ;
  les dossiers propriétaires portent leur propre `LICENSE` au moment où ils reçoivent du code ; la spécification du
  format est un document public versionné ; les contributions externes au cœur exigent un DCO ou un CLA (à trancher
  lors de l'ouverture du dépôt).
- **Dérogation RAIDER** : aucune.
