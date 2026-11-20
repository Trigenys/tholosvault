# Thèse H9 — Téléchargement conscient du format

Thèse issue de l'ADR-0005 (`docs/adr/0005-telechargement-segmente.md`) et du rapport
`telechargement-histoire-goulots.md`. Mesurée par le banc (`bench/`, module `theses/h9.rs`) contre un
serveur HTTP local qui simule 206 / 200 sans `Range`, coupures à mi-segment, octets corrompus,
`Retry-After` et changement d'ETag.

## Énoncé

Un téléchargeur qui lit d'abord le trailer et les tables de l'archive, puis demande le contenu par
plages alignées sur les morceaux et vérifie chaque morceau par son empreinte BLAKE3, obtient trois
propriétés qu'aucun téléchargeur générique n'a sans coopération du serveur :

- **H9a — localité de la panne** : un morceau corrompu coûte au plus 1 morceau et 1 requête ; une
  coupure suivie d'une reprise ne re-télécharge aucun octet déjà vérifié.
- **H9e — refus avant contenu** : si l'appelant fournit une racine attendue (`--root`) différente de la
  racine lue dans le trailer, aucun octet de contenu n'est demandé et rien n'est écrit sur le disque.
- **Débit** : la vérification par morceau et le découpage en segments ne coûtent pas plus de 10 % par
  rapport à `curl` parallèle sur le même serveur local (N connexions, une plage contiguë chacune).

H9b (premier `read_range` servi avant la fin du téléchargement) et H9d (montage partiel) restent hors
du MVP ; ils seront mesurés quand la lecture partielle existera.

## Métriques

| Métrique | Seuil | Bloquant | Capacité |
|---|---|---|---|
| `h9.corrupt_chunk_cost` (morceaux) | ≤ 1 | oui | — |
| `h9.resume_rebytes` (B) | = 0 | oui | — |
| `h9.root_mismatch_rejected` (ratio) | = 1 | oui | — |
| `h9.aggregate_throughput_ratio` (× `curl`) | ≥ 0,9 | non (ADR-0008) | `curl` |
| `h9.fetch_s_per_gb`, `h9.curl_s_per_gb` | informatif | — | `curl` |

`h9.corrupt_chunk_cost` est le pire des trois comptes : morceaux rejetés, requêtes supplémentaires par
rapport à une passe propre, octets rejetés rapportés à la taille du morceau. `h9.resume_rebytes` somme
les octets servis par le serveur, après la reprise, dans les plages que la première passe avait déjà
vérifiées (le journal du serveur fait foi, pas le client).

## Protocole

1. Corpus binaire pseudo-aléatoire (`fetch_bytes` du profil : 32 Mio en `ci`, 256 Mio en `full`),
   empaqueté au niveau de H1 avec la parité par défaut ; l'archive est servie en mémoire.
2. Passe propre (4 connexions) : référence de requêtes, vérification octet à octet de la destination.
3. Corruption d'un octet du morceau au tiers de l'archive, à sa première livraison.
4. Coupure après 1 000 octets dans le morceau médian, sans relance interne (`max_segment_attempts = 1`),
   puis reprise depuis le `.part`.
5. Racine attendue fausse.
6. Si `curl` est dans le PATH : médiane de 3 passes complètes `tholos fetch` et de 3 passes de 4 `curl`
   parallèles ; le rapport est `curl / fetch`.

## Réfutation

Les trois premières métriques sont des invariants : une valeur hors seuil casse la CI, il n'y a rien à
« ajuster ». Le rapport de débit dépend du bruit du runner et de la boucle serveur locale ; une valeur
sous 0,9 est consignée dans l'ADR-0008 avec la cause mesurée, jamais masquée par un seuil abaissé.
