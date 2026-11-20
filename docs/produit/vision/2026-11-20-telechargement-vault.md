# Vision produit — Téléchargement d'archives `.vault`

- **Date** : 2026-11-20
- **Question** : quelle place le téléchargement doit-il prendre dans tholosvault, au-delà du cadrage
  technique de l'ADR-0005 (`vault-fetch`), et dans quel ordre livrer ?
- **Résultat attendu** : pistes classées avec thèse mesurable, contradicteur, prérequis ; décision du fondateur.
- **Cadre** (3 lignes) :
  - Unique : archive compressée dédupliquée, adressée par morceau (BLAKE3, Merkle, index), parité RS, lecture
    par plage (`read_range`) et montage (PR #9). Personne ne combine ces briques avec un téléchargement segmenté.
  - Réfuté : H4 sur le débit séquentiel à travers FUSE (0,18 vs ≥ 0,7, ADR-0004) ; cœur et exactitude soutenus.
  - Hors périmètre déclaré : réseau (cadrage §9) → entrée autorisée par l'ADR-0005, bornée aux `.vault` ;
    P2P, capture de liens, upload, hébergement de morceaux restent exclus.

## 1. Terrain

| Besoin | Solution actuelle | Douleur mesurée | Preuve (date) |
|---|---|---|---|
| Récupérer un gros fichier (jeu, jeu de données, modèle) sans repartir de zéro | Navigateur, `curl`, FDM, Steam | Un morceau corrompu ou une mise à jour en cours de téléchargement → retéléchargement complet ; perte de > 20 Go rapportée | Steam Client Beta, forum « Stop from entire redownload if it picks up corrupt part » (2025) ; `minigalaxy` #630 |
| Payer la sortie de données une seule fois | Hébergement S3/GCS/Azure | 0,09 $/Go (AWS, premiers 10 To), 0,12 $/Go (GCP) ; 10 To/mois ≈ 900–1 100 $ ; R2 = 0 $ egress, B2 gratuit jusqu'à 3× le stock | egresscost.com, computecheck.com (2026) |
| Ne transférer que ce qui change entre versions | Git LFS, fichiers entiers | Xet (Hugging Face) : CDC ~64 Kio, dédup 30–85 % en test, démo 2026 : 62 % dédupliqué (1,5 To évités sur 2,4 To) ; projection hub : 100 To évités à 50 % `[vendeur, pas de mesure post-déploiement publiée]` | HF blog « From files to chunks » (2024-11), « Storage Buckets » (2026) ; ZipLLM arXiv 2505.06252 (coût métadonnées 35 To à l'échelle du hub) |
| Télécharger vite sur un lien lent | Multi-connexions (FDM, Prism 32 conn., SwiftDL) | Médiane fixe Cameroun 17,4 Mbit/s (mars 2026, miroir Ookla) `[incertain : 8,9–17,4 selon la source]` ; Afrique du Sud 48,9 ; 50 Gio à 17,4 Mbit/s = **6,9 h** | Wikipedia/Ookla (2026-03), worldpopulationreview (2025) |
| Mettre à jour par morceaux avec cache CDN | Steam (~1 Mio par morceau, réutilisation locale, rsync-like) | Un seul mauvais morceau peut forcer un redémarrage complet côté client (rapports utilisateurs) | Valve news 5856 (2011), forum Steam Client Beta (2025) `[secondaire]` |
| Monter une archive distante sans tout lire | ratarmount, ratarmount-rs, casync/desync, RAUC | ~9 000 requêtes `Range` par shard de 128 Mio avant coalescence (ratarmount-rs PR #61) | GitHub (2025) |

Taux d'échec publié des gros téléchargements : **aucun trouvé** (forums seulement). C'est la première
incertitude de cette séance : la douleur est réelle mais sa fréquence n'est pas mesurée.

## 2. Goulots chiffrés

| Goulot | Nature | Ordre de grandeur (50 Gio, morceaux de 256 Kio) | Conséquence |
|---|---|---|---|
| Débit du lien | Physique | 17,4 Mbit/s → 6,9 h ; 100 Mbit/s → 1,2 h ; 1 Gbit/s → 7 min | Ne se contourne pas : seul levier = **moins d'octets** (dédup inter-versions, parité facultative, lecture partielle) |
| Coupures sur une longue session | Physique/probabiliste | Si une coupure toutes les 2 h en moyenne, P(≥ 1 coupure en 6,9 h) = 1 − e^(−3,45) ≈ **97 %** `[hypothèse de fréquence]` | La reprise exacte n'est pas une option : c'est le cas nominal sur lien lent |
| Coût d'une corruption | Mathématique | Générique : 1 octet faux → 50 Gio (6,9 h, +4,5 $ d'egress AWS). Par morceau : 256 Kio → **0,12 s** à 17,4 Mbit/s. Rapport ≈ 200 000 | Différenciateur n° 1 ; Merkle par morceau déjà payé par le format |
| Nombre de requêtes | Physique (RTT) | 204 800 morceaux × 50 ms = 2,8 h si une requête par morceau en série ; segments de 8 Mio → 6 400 requêtes → 5,3 min à 1 connexion, 40 s à 8 | Coalescer sur frontières de morceaux (leçon ratarmount) ; sinon le RTT domine |
| Vérification | Mathématique (Amdahl) | BLAKE3 ≈ 1–3 Go/s par cœur vs 2,2 Mo/s de réseau à 17,4 Mbit/s : rapport > 500 | Le hachage n'est jamais le goulot ; la vérification à l'arrivée est gratuite |
| Entropie | Mathématique | Vidéo, zstd, chiffré : taille ≈ entropie, compression nulle | Le téléchargement n'est pas accéléré par la compression ; seule la dédup inter-versions réduit les octets |
| Parité | Mathématique | RS à 5 % = 2,5 Gio de plus ; inutile tant que la source est joignable | Télécharger la parité en dernier, facultative (ADR-0005) |
| Egress | Économique | 50 Gio × 0,09 $ = 4,5 $ par téléchargement AWS ; 0 $ sur R2/B2 (< 3× stock) ; retélécharger double la facture de l'éditeur | Déplacé vers l'éditeur : le téléchargeur protège **son** budget, argument de vente côté éditeur |
| Confiance | Humain | Un SHA-256 global n'est vérifiable qu'à la fin ; une racine Merkle dans le lien est vérifiable avant le premier octet de contenu | Première minute : « racine attendue = racine reçue » ou arrêt |
| Amorçage | Humain/économique | Aucune archive `.vault` publiée aujourd'hui ; un téléchargeur sans fichiers à télécharger n'a pas d'usage | Le premier éditeur doit être Trigenys (jeux de données, livrables clients) |

## 3. Résolution

| Goulot | Méthode existante (source) | Combinaison (invariant) | Extension | Thèse mesurable |
|---|---|---|---|---|
| Corruption / reprise | Vérification par chunk + hash de fichier (xet-core PR #967) ; ETag fort avant reprise (downkyicore #372) | **Le même hachage BLAKE3 par morceau sert la dédup, la vérification du téléchargement et la réparation** | — | **H9a** : un morceau corrompu coûte ≤ 1 morceau + 1 requête ; une reprise retélécharge 0 octet vérifié. Réfuté si > 1 morceau retéléchargé par corruption injectée |
| RTT | Coalescence de plages (ratarmount-rs PR #61), segments ~1 Mio (Steam) | Segments = suites de morceaux contigus bornées par l'index (frontières de frames connues avant le premier octet) | — | **H9b** : débit agrégé sur N connexions ≥ 0,9 × `curl` parallèle sur le même serveur local |
| Débit du lien (octets) | Dédup CDC inter-versions (HF Xet 30–85 %), réutilisation locale (Steam, casync) | Table de hachage de l'archive locale N = bitmap de départ du téléchargement de N+1 | Contradiction « il faut l'archive entière (montage) et pas l'archive entière (lien lent) » levée par **séparation dans le temps** : lire ce qui est là, demander ce qui manque en priorité (ordonnancement à la demande) | **H9c** : version N+1 coûte ≤ octets des morceaux changés + tables + 5 % ; **H9d** : premier `read_range` d'un fichier interne servi en ≤ 2 RTT + 1 segment après les tables, quelle que soit la taille de l'archive |
| Confiance | Lien avec racine (magnet/BitTorrent, OCI digest `@sha256:`) | Racine Merkle dans `/s/<id>` et `--root` ; mismatch = arrêt avant tout contenu | — | **H9e** : 100 % des archives dont la racine diffère sont rejetées avant le premier octet de contenu |

Bornes et pire cas : pire cas de H9a = corruption sur chaque morceau d'un segment → coût = segment + 1 requête
par morceau (borné par la taille de segment, 8 Mio). Pire cas de H9d = fichier interne dont les morceaux
sont dispersés dans l'archive → autant de segments que de morceaux ; l'index le dit avant de demander, donc
le coût est **prévisible** avant la requête (`plan()` en lecture seule).

## 4. Cinq lectures

- **Financière** : MVP `tholos fetch` ≈ 10–15 j (cœur + adaptateur HTTP + serveur de test) ; exploitation 0 $
  (client seul, serveur statique) ; pas de prix direct : vend l'archive, pas le téléchargeur ; cannibalise
  rien (les généralistes restent utiles pour le reste).
- **Économique** : besoin porté par la croissance des fichiers (modèles, jeux de données, jeux > 50 Go) et par
  l'egress payant (0,09 $/Go) ; pourquoi maintenant : CDC + Merkle validés à grande échelle (HF Xet 2025–2026),
  R2/B2 rendent l'hébergement statique gratuit en sortie ; substituts : FDM/Prism + `tholos verify/repair`.
- **Sociétale** : gagnants = utilisateurs sur lien lent ou instable (17 Mbit/s médian, coupures) et éditeurs
  payant l'egress ; souveraineté : aucun service tiers requis, un fichier sur n'importe quel serveur `Range` ;
  détournement : distribution de contenus illicites comme tout téléchargeur, pas de garde-fou propre au format
  au-delà de la racine vérifiable.
- **Scientifique** : reproductible via le banc (`bench/`, serveur Axum local avec corruption, coupure, 200 vs 206,
  ETag changeant) ; démontré = Merkle par morceau détecte la corruption ; supposé = fréquence réelle des
  coupures et corruptions, gain de dédup sur les corpus visés ; publier exigerait un corpus public de versions
  successives et une mesure terrain des coupures.
- **Psychologique** : première minute = la racine affichée et vérifiée avant le contenu, barre par morceaux
  vérifiés ; défaut sûr = refuser une racine différente, ne jamais écrire un morceau non vérifié ; abandon si
  « la source a changé » sans action claire → message avec `--restart` ; charge mentale nulle en MVP (une
  commande).

## 5. Pistes classées (impact × faisabilité × différenciation, 1–5)

| # | Piste | I | F | D | Score | Thèse / seuil | Prérequis | Premier livrable ≤ 2 sem. | Risque principal | Ce qui la tue |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | `tholos fetch` MVP : trailer → tables → segments sur frontières, BLAKE3 à l'arrivée, reprise par bitmap `.part` | 4 | 4 | 5 | **80** | H9a, H9b, H9e | ADR-0005 accepté ; `read_range` existant | Crate `vault-fetch` + serveur de test, H9a en CI | Aucun `.vault` publié → pas d'usage | Si `verify/repair` + FDM couvrent 95 % des cas mesurés |
| 2 | Lecture et montage de l'archive partielle (ordonnancement à la demande) | 5 | 2 | 5 | 50 | H9d | Piste 1 ; source de morceaux abstraite dans `vault-core` ; chantier débit FUSE (ADR-0004) | Prototype `read_range` sur archive partielle en test, sans montage | Dépend de H4 non résolue ; complexité d'ordonnancement | Si le débit FUSE reste à 0,18 : lire à travers le montage sera plus lent que d'attendre la fin |
| 3 | Mise à jour par delta depuis une archive locale | 4 | 3 | 4 | 48 | H9c | Piste 1 ; table de hachage locale lisible | Mesure de dédup sur 3 corpus internes versionnés (préalable à tout code) | Gain réel inconnu sur les corpus visés (HF : 30–85 %) | Dédup < 20 % mesurée sur nos corpus |
| 4 | « Réparer depuis une URL » : ne redemander que les morceaux abîmés | 3 | 4 | 4 | 48 | coût = morceaux abîmés × (1 + ε), ε ≤ 1 requête/morceau | Piste 1 ; `tholos repair` existant | Option `--source <url>` de `repair` | Faible fréquence d'usage | Si la parité RS suffit dans tous les cas mesurés |
| 5 | Lien `/s/<id>` avec racine + file d'attente desktop | 3 | 3 | 2 | 18 | H9e côté console | Console web (V1), piste 1 | Spécification du lien (format, racine, taille) | Refait l'UX des généralistes | Si personne ne partage de `.vault` par lien |

**Ordre recommandé** : 1 → 3 (mesure de dédup, sans code) → 2 (après le chantier FUSE) → 4 → 5.

## Contradicteur (meilleure raison de ne pas faire la piste 1)

La douleur « retélécharger tout pour un morceau faux » n'a **aucune fréquence mesurée** : avec TCP et TLS, la
corruption silencieuse en transit est rare ; les cas rapportés viennent de mises à jour pendant le
téléchargement et de disques défaillants. Un généraliste libre (Prism, SwiftDL) + `tholos verify` + parité RS
couvre déjà la reprise et la réparation locale. Construire un téléchargeur pour un format que **personne ne
publie encore** inverse l'ordre : la valeur vient quand Trigenys distribue ses propres archives. Réponse
proposée : garder la piste 1 au MVP minimal (une commande, H9a/H9b/H9e en CI), **mesurer** sur les premières
archives distribuées par Trigenys le nombre de morceaux rejetés et de reprises par téléchargement, et ne
lancer les pistes 2–5 qu'avec ces chiffres. Décision engageante (positionnement « format + téléchargeur »
vs « format seul ») → `comex` si le fondateur hésite.

## Sources (consultées le 2026-11-20)

- egresscost.com, computecheck.com, ecorpit.com — tarifs egress 2026 (AWS 0,09 $/Go, GCP 0,12, R2 0, B2 3×).
- huggingface.co/blog/from-files-to-chunks (2024-11), huggingface.co/blog/storage-buckets (2026),
  huggingface.co/join/xet — CDC ~64 Kio, dédup 30–85 %, démo 62 %.
- arxiv.org/abs/2505.06252 (ZipLLM, 2025) — coût des métadonnées de dédup par morceau à l'échelle.
- Wikipedia « List of countries by Internet connection speeds » (Ookla, 2026-03) ; worldpopulationreview (2025).
- steamcommunity.com, Steam Client Beta « Stop from entire redownload… » (2025) ; store.steampowered.com/news/5856
  (2011) ; github.com/sharkwouter/minigalaxy/issues/630.
- github.com/hilather/ratarmount-rs (PR #61), huggingface/xet-core (PR #967), downkyicore (#331, #372),
  systemd/casync, folbricht/desync, rauc — voir ADR-0005 pour le détail.
- gh search repos 2026-11-20 : trauma, swiftdl, prism-download-manager, misterdownloadmanager, aria2 (ADR-0005).

## Incertitudes

- Fréquence réelle des corruptions et coupures sur les téléchargements de `.vault` : **non mesurée** ; à
  instrumenter dès la première distribution.
- Débit médian Cameroun : 8,9–17,4 Mbit/s selon la source et l'année ; l'ordre de grandeur suffit au calcul.
- Gain de dédup inter-versions sur les corpus Trigenys : inconnu, préalable à la piste 3.
- Résultats Xet post-déploiement : uniquement des chiffres de l'éditeur.
