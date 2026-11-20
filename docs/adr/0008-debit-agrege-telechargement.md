# ADR-0008 — Débit agrégé du téléchargement : métrique non bloquante

- **Statut** : accepté
- **Date** : 2026-11-20
- **Contexte** : la thèse H9 (`docs/recherche/these-h9-telechargement.md`) compare le débit de
  `tholos fetch` sur 4 connexions à celui de 4 `curl` parallèles contre le même serveur local
  (`h9.aggregate_throughput_ratio`, cible ≥ 0,9). Les trois autres métriques de H9 sont des invariants
  (coût d'une corruption, octets re-téléchargés, refus avant contenu) et restent bloquantes.
- **Cause** : la comparaison porte sur un processus externe (`curl`) ordonnancé par l'OS à côté du
  banc, sur un runner partagé dont la charge varie ; le serveur de test tourne dans le même processus
  que le client, si bien que le temps mesuré mélange service et réception. Un rapport absolu entre deux
  durées dans ces conditions n'est pas reproductible à 10 % près, contrairement aux rapports internes du
  banc (H4, H5) qui mesurent deux chemins du même binaire au même instant.
- **Options** :
  1. Rendre la métrique bloquante avec le seuil de l'ADR-0005 — refusée : des échecs dus au bruit
     feraient relâcher le seuil ou ignorer le banc.
  2. Supprimer la mesure — refusée : le coût de la vérification par morceau doit rester visible.
  3. Mesurer, publier, ne pas bloquer ; exiger `curl` comme capacité pour ne pas conclure sans
     référence — **retenue**.
- **Décision** : `h9.aggregate_throughput_ratio` est `blocking: false`, `requires: "curl"`, et renvoie à
  cet ADR. Les durées brutes (`h9.fetch_s_per_gb`, `h9.curl_s_per_gb`) sont publiées à côté. Le seuil
  0,9 n'est pas modifié.
- **Conséquences** :
  - une valeur sous 0,9 apparaît dans le résumé du banc comme métrique informative hors cible ; elle
    doit être expliquée ici (cause mesurée) avant toute optimisation ;
  - la métrique devient bloquante le jour où le banc dispose d'un serveur de référence hors processus
    et d'un runner dédié (mise à jour de cet ADR) ;
  - mesures locales (2026-11-20, Windows x86_64, 16 cœurs, profil `ci`, archive de 32 MiB, serveur de
    test en processus, 4 connexions, médiane de 3 passes) : `h9.fetch_s_per_gb` 57,5 s/GB,
    `h9.curl_s_per_gb` 17,6 s/GB, `h9.aggregate_throughput_ratio` **0,31** (cible 0,9, non atteinte) ;
    les trois invariants bloquants sont tenus (coût d'une corruption 1 morceau, 0 octet re-téléchargé à
    la reprise, refus avant contenu). Causes à mesurer avant toute optimisation : archive trop petite pour
    amortir sonde + trailer + tables + écriture atomique du `.part` (coûts fixes dominants à 32 MiB),
    vérification BLAKE3 et écriture positionnée dans le même processus que le serveur, `curl` écrivant
    4 fichiers séparés sans aucune vérification. Le seuil reste 0,9. Le verdict de thèse du banc
    (`report.rs`) compte toute métrique hors cible, bloquante ou non : H9 s'affiche « refuted » tant que
    ce rapport est sous 0,9, bien que le banc sorte en succès ; l'exécution de la CI n'a pas pu être
    observée (facturation du dépôt privé).
