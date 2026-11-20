# VAULT — Cadrage des surfaces et des pages

Version 0.1 · 2026-11-16 · Statut : proposition, à trancher (voir §8).

Référence technique : rapport *Archives montables et compression prospective* (dossier `docs/recherche/`),
en particulier « Périmètre du MVP et protocole d'expérimentation », les thèses H1–H8 et les innovations E1–E5.
Référence message : script de la vidéo « VAULT, le principe » (8 scènes), repris en §1.3.

## 1. Produit

### 1.1 Promesse

VAULT est un format d'archive **montable** : on branche l'archive comme un disque, on ne l'extrait pas.
Sous le capot : découpe par contenu (CDC 64–256 KiB), compression zstd par morceau avec dictionnaires,
arbre de Merkle BLAKE3 (racine d'archive, empreinte par morceau), 3–5 % de parité Reed-Solomon sur
stripes entrelacées, index non compressé lisible par mmap. Résultat : lire un morceau en ~0,1 ms,
localiser un octet abîmé, réparer sans retélécharger, mettre à jour en ne transférant que les morceaux changés.

### 1.2 Cibles (ordre de priorité, issu du rapport)

1. **Bibliothèques de jeux solo / indé / rétro, mods, émulateurs** (ratio 1,5–2× sur 2 To ; aucun anti-cheat noyau).
2. **Artefacts CI, arbres de sources, images de build** (dédup massive : 465 GiB → 28,6 GiB chez DwarFS).
3. **Archives documentaires et photothèques** (recompression Deflate/JPEG réversible, vérification d'intégrité).

Hors cible explicite : jeux compétitifs protégés par Vanguard / EAC / BattlEye montés par driver (risque de ban, H7).

### 1.3 Message produit (repris du script vidéo, à réutiliser tel quel sur la landing)

| Scène | Idée | Accroche |
|---|---|---|
| intro | Le problème | Pour lire un seul fichier, il faut tout sortir. Un quart d'heure, un disque plein. |
| mount | La promesse | VAULT : on branche l'archive, on ne l'ouvre pas. Prêt en moins d'une seconde. |
| cdc | Découpe par contenu | On coupe là où le contenu le décide. Un ajout au début : un seul morceau change. |
| zstd | Compression par morceau | Chaque morceau est compressé seul. Lire un morceau : 0,1 ms au lieu de 122. |
| merkle | Intégrité | Une empreinte par morceau, résumées en une racine. Un octet abîmé : on sait exactement où. |
| parity | Réparation | 3 à 5 % de parité : on répare sans retélécharger. |
| delta | Mise à jour | Nouvelle version : seuls les morceaux changés voyagent. |
| outro | Philosophie | Une seule découpe sert à tout. L'intelligence à l'encodage. Un lecteur simple, rapide, prévisible. |

### 1.4 Vocabulaire commun à toutes les surfaces

| Terme UI | Définition | Terme interne |
|---|---|---|
| Archive | Fichier `.vault` (un ou plusieurs volumes) | archive |
| Morceau | Unité de découpe CDC, adressée par BLAKE3 | chunk |
| Racine | Hash racine de l'arbre de Merkle, identité de la version | merkle_root |
| Parité | Shards Reed-Solomon, exprimée en % | parity budget |
| Monter / Démonter | Exposer l'archive comme lecteur ou dossier | mount / unmount |
| Vérifier | Recalculer et comparer les empreintes | verify |
| Réparer | Reconstruire les morceaux abîmés par la parité (ou depuis une version voisine) | repair |
| Mise à jour delta | Produire / appliquer la liste des morceaux changés entre deux racines | delta |
| Profil d'accès | Trace de lecture servant au layout chaud/froid (E1) | access profile |

## 2. Surfaces et priorités

Niveaux : **MVP** (prouve la thèse, premiers utilisateurs techniques), **V1** (produit grand public desktop + site),
**Plus tard** (après validation H1–H8 et traction).

| Surface | Rôle | Priorité |
|---|---|---|
| CLI `vault` | Référence du cœur, scripts, CI, support | MVP |
| App desktop | Créer, monter, vérifier, réparer, mettre à jour ; FUSE / WinFsp / ProjFS | MVP (minimal) → V1 |
| Landing page | Expliquer, convaincre, télécharger, capter des e-mails | MVP (une page) → V1 |
| App web (console/compte) | Compte, licences, archives publiées, partage, télémétrie opt-in | V1 (compte + licences) → Plus tard (console complète) |
| App mobile | Consulter une archive, extraire un fichier, vérifier l'intégrité | Plus tard |
| Téléchargeur (`tholos fetch`, écran « Téléchargements ») | Récupérer une archive par segments parallèles avec reprise, vérification BLAKE3 par morceau, file d'attente, limite de débit, lecture avant la fin (ADR-0005) | MVP (CLI) → V1 (desktop, file, montage partiel) |

Le CLI n'est pas demandé dans le cadrage initial mais il est la surface la moins chère pour prouver le format ;
toutes les apps l'enveloppent (même cœur Rust, voir `docs/produit/stack.md`).

## 3. Landing page (marketing)

Objectif global : un visiteur technique comprend le principe en 60 s, télécharge l'app ou laisse son e-mail.

| Page / section | Objectif | Composants clés | Données affichées | Priorité |
|---|---|---|---|---|
| `/` Hero | Promesse en une phrase + vidéo 65 s | Titre, sous-titre, vidéo (MP4 + poster), CTA « Télécharger » / « Rejoindre la liste » | OS détecté, version courante | MVP |
| `/` Le problème → la solution | Scènes intro → mount | Comparatif « extraire » vs « monter » (deux colonnes, chiffres) | 15 min vs < 1 s ; disque plein vs 0 octet écrit | MVP |
| `/` Comment ça marche | Scènes cdc, zstd, merkle, parity, delta | 5 cartes illustrées (reprise des stills de la vidéo), schéma animé léger | 0,1 ms vs 122 ms ; 3–5 % parité | MVP |
| `/` Pour qui | Trois cibles §1.2 | Trois cartes, un chiffre chacune | Ratios mesurés (sourcés) | MVP |
| `/` Comparatif | Pourquoi pas 7-Zip, squashfs, EROFS, DwarFS | Tableau de fonctionnalités (montable, parité, Merkle, delta, Windows/macOS natif, dédup inter-versions) | Matrice booléenne sourcée au rapport | V1 |
| `/` Preuves | Benchmarks du protocole | Graphiques statiques générés depuis `bench/` | Ratio, latence p50/p99, temps de réparation | V1 (dès premières mesures) |
| `/` FAQ | Anti-cheat, antivirus, sécurité, licences | Accordéon | — | MVP (3 questions) → V1 |
| `/download` | Télécharger selon l'OS | Détection OS, 3 boutons, checksums, notes de version | Version, SHA-256, date | V1 (MVP : lien GitHub Releases) |
| `/docs` | Documentation utilisateur et format | Astro content collections, recherche | Spécification du format, guides CLI | V1 |
| `/pricing` | Offre | Grille gratuit / pro / équipe | Prix, limites | Plus tard (dépend de la décision modèle économique) |
| `/blog` | Publication des résultats H1–H8 | Articles Markdown | — | V1 |
| `/legal`, `/privacy` | Mentions, RGPD | Pages statiques | — | MVP |

Parcours : arrivée (lien vidéo / réseau) → hero → vidéo → « comment ça marche » → CTA → téléchargement ou e-mail.
Mesure : taux de clic CTA, téléchargements par OS, inscriptions. Analytics sans cookie (Plausible ou Umami auto-hébergé).

## 4. App desktop (Windows, macOS, Linux)

Objectif global : la personne crée, monte, vérifie, répare et met à jour une archive sans ligne de commande.
Toute action passe par le cœur Rust ; l'UI n'implémente aucune logique de format.

### 4.1 Écrans

| Écran | Objectif | Composants clés | Données affichées | Priorité |
|---|---|---|---|---|
| Onboarding (1re ouverture) | Installer le pilote de montage, expliquer les limites | Détection OS ; étape WinFsp / macFUSE-FSKit / FUSE ; avertissement anti-cheat ; choix du dossier de montage par défaut | État du pilote, version | MVP |
| Bibliothèque | Voir toutes les archives connues | Liste/grille, recherche, tri, statut (montée, saine, à réparer, mise à jour dispo), actions rapides | Nom, taille archive vs taille logique, ratio, racine abrégée, dernière vérification | MVP |
| Créer une archive (assistant) | Produire un `.vault` depuis des dossiers | Sélecteur de sources, préréglages (jeux / sources / photos / personnalisé), niveau zstd, budget parité (3/5/10 %), dictionnaires, recompression (preflate/JXL, opt-in), découpage en volumes, chiffrement | Estimation taille/temps, progression par étape (scan, découpe, compression, parité, index), journal | MVP |
| Détail d'une archive | Tableau de bord d'une archive | Onglets : Contenu, Intégrité, Parité, Versions, Partage, Profil d'accès | Arborescence navigable, racine Merkle, % parité, historique de versions | MVP (Contenu, Intégrité) → V1 (autres onglets) |
| Monter / Démonter | Exposer l'archive | Bouton, choix lettre/point de montage, mode (lecture seule ; ProjFS+WOF « installation paresseuse » sur Windows), montage au démarrage | Point de montage, latence p50 observée, cache RAM utilisé | MVP |
| Vérifier | Contrôler l'intégrité | Vérification rapide (index + échantillon) ou complète ; carte des morceaux (sain / abîmé / manquant) | Nombre de morceaux, abîmés, temps | MVP |
| Réparer | Reconstruire | Choix de la source : parité locale, autre volume, version voisine, URL ; progression ; rapport | Morceaux réparés / irréparables | MVP (parité locale) → V1 (version voisine, URL) |
| Mise à jour delta | Passer d'une racine à une autre | Comparer deux archives ou source + archive ; aperçu des morceaux changés ; produire un fichier delta ou appliquer | Octets à transférer vs taille totale, liste des fichiers changés | V1 |
| Partager | Publier ou exporter | Export de volumes, génération d'un lien (via compte web), QR code pour mobile | Lien, racine, taille | V1 (export) → Plus tard (lien compte) |
| Téléchargements (ADR-0005) | Récupérer des archives distantes | File d'attente, pause / reprise, connexions et limite de débit, carte des morceaux reçus / vérifiés / rejetés, « Ouvrir » ou « Monter » avant la fin | Débit, octets vérifiés / restants, morceaux redemandés, racine attendue vs reçue | V1 |
| Profil d'accès (E1) | Optimiser le layout chaud/froid | Activer l'enregistrement, voir la trace, lancer `optimize` | Fichiers chauds détectés, gain estimé | Plus tard |
| Paramètres | Pilote, cache, télémétrie opt-in, mises à jour de l'app, compte | Formulaires | — | MVP |
| Journal / Diagnostics | Support | Logs structurés, export d'un bundle de diagnostic (sans secrets) | — | MVP |

### 4.2 Parcours

- **Onboarding** : installation → détection du pilote → installation guidée (WinFsp / FSKit-macFUSE / fuse3) → avertissement anti-cheat
  (case à cocher « j'ai compris ») → bibliothèque vide avec deux CTA : « Créer » / « Ouvrir ».
- **Créer** : sources → préréglage → options avancées (repliées) → résumé (estimation) → exécution avec progression par étape →
  écran de détail avec la racine Merkle et la proposition « Monter maintenant ».
- **Ouvrir / monter** : double-clic sur un `.vault` (association de fichier) ou « Ouvrir » → détail → « Monter » → le lecteur s'ouvre dans l'explorateur.
- **Vérifier / réparer** : depuis le détail ou l'alerte de la bibliothèque → vérification → si morceaux abîmés, bouton « Réparer » →
  choix de la source → rapport. Si parité insuffisante : message actionnable (« 12 morceaux irréparables ; fournir une autre version ou le volume 3 »).
- **Partager** : détail → Partager → export des volumes vers un dossier / lien compte.
- **Mise à jour delta** : détail → Versions → « Mettre à jour vers… » → aperçu du delta → appliquer ; ou « Produire un delta » pour distribution.

### 4.3 Contraintes

- Lecture seule au montage dans le MVP (pas d'overlay d'écriture, conformément au rapport).
- Toute opération longue est annulable et reprend (idempotence du build : morceaux adressés par hash, reprise depuis l'index partiel).
- Aucune télémétrie sans opt-in explicite ; les traces d'accès (E1) restent locales.

## 5. App web (console / compte)

Objectif global : gérer son compte et ses licences, publier et partager des archives, consulter leur santé à distance.
Elle n'héberge pas les données des archives dans le MVP : elle stocke des **métadonnées** (racine, index signé, liste de morceaux)
et, plus tard, des morceaux pour la distribution.

| Page | Objectif | Composants clés | Données affichées | Priorité |
|---|---|---|---|---|
| Connexion / inscription | Accéder au compte | Magic link e-mail + passkeys (WebAuthn), OAuth GitHub en option | — | V1 |
| Tableau de bord | Vue d'ensemble | Cartes : archives publiées, espace, dernières vérifications, appareils liés | Compteurs, alertes d'intégrité remontées par les clients | V1 |
| Archives | Lister les archives publiées / référencées | Table, filtres, statut, racine | Nom, racine, taille, versions, dernier contrôle | V1 |
| Détail d'une archive | Versions et partage | Historique des racines, graphe des deltas, liens de partage (expiration, mot de passe), QR | Racines, deltas entre versions (octets), accès | V1 |
| Partage public `/s/<id>` | Permettre à un tiers de récupérer | Page publique : nom, racine, taille, bouton « Ouvrir dans VAULT » (deep link) / télécharger volumes, vérification en ligne de la racine | Racine, taille, version | V1 |
| Appareils | Appareils connectés au compte | Liste, révocation | Nom, OS, dernière activité | V1 |
| Licences / facturation | Gérer l'offre | Portail de paiement (Stripe ou Lemon Squeezy), factures | Plan, échéance | Plus tard (dépend décision §8) |
| Équipe / organisation | Multi-utilisateurs | Membres, rôles, archives partagées | — | Plus tard |
| Paramètres compte | Profil, sécurité, suppression (RGPD) | Formulaires, export de données | — | V1 |
| Admin interne | Support | Recherche compte, état des services | — | Plus tard |

Parcours **partager** (bout en bout) : desktop → Partager → connexion compte si besoin → publication des métadonnées (+ volumes si hébergés) →
lien `/s/<id>` → destinataire ouvre la page → « Ouvrir dans VAULT » → l'app desktop ou mobile récupère et vérifie la racine.

## 6. App mobile (Android, iOS)

Objectif global : consulter une archive, en extraire un fichier, en vérifier l'intégrité ; pas de montage (impossible sans jailbreak / root).
Cas d'usage : vérifier une sauvegarde sur un disque externe ou un partage, récupérer un document précis, scanner un QR de partage.

| Écran | Objectif | Composants clés | Données affichées | Priorité |
|---|---|---|---|---|
| Accueil / archives récentes | Retrouver une archive | Liste, « Ouvrir un fichier », « Scanner un QR », compte | Nom, racine, taille, dernier contrôle | Plus tard |
| Ouvrir une archive | Depuis fichiers locaux, stockage externe (USB-C, SMB) ou lien de partage | Sélecteur de fichiers système, lecteur par plages (range requests) pour les liens | — | Plus tard |
| Explorer | Naviguer dans l'arborescence sans extraire | Arbre, recherche, aperçu (images, texte, PDF) lu morceau par morceau | Taille, type, empreinte | Plus tard |
| Extraire un fichier | Récupérer un seul fichier | Choix de destination (Fichiers / Partager), progression | Octets lus vs taille totale de l'archive | Plus tard |
| Vérifier | Intégrité rapide ou complète | Carte des morceaux, rapport, option « signaler au compte » | Morceaux abîmés | Plus tard |
| Détail / versions | Voir la racine et les versions connues | Racine, QR de la racine, versions | — | Plus tard |
| Paramètres | Compte, cache, télémétrie | — | — | Plus tard |

Pas de réparation sur mobile dans un premier temps (lecture seule du média ; reporter à l'app desktop).

## 7. Matrice surfaces × fonctionnalités

Légende : ● MVP · ◐ V1 · ○ Plus tard · — non prévu.

| Fonctionnalité | CLI | Desktop | Landing | Web (console) | Mobile |
|---|---|---|---|---|---|
| Créer une archive (CDC + zstd + Merkle + parité) | ● | ● | — | — | — |
| Monter / démonter (FUSE / WinFsp) | ● | ● | — | — | — |
| Mode « installation paresseuse » ProjFS + WOF (E5) | ◐ | ◐ | — | — | — |
| Explorer le contenu sans extraire | ● | ● | — | ○ (aperçu index) | ○ |
| Extraire un fichier | ● | ● | — | — | ○ |
| Vérifier l'intégrité (Merkle) | ● | ● | — | ◐ (racine en ligne) | ○ |
| Réparer par parité locale | ● | ● | — | — | — |
| Réparer depuis une version voisine / URL | ◐ | ◐ | — | ◐ (source) | — |
| Télécharger une archive par segments, reprise, vérification par morceau (ADR-0005) | ● | ◐ | — | ◐ (source `/s/<id>`) | — |
| File d'attente et limite de débit des téléchargements | ◐ | ◐ | — | — | — |
| Lire / monter une archive en cours de téléchargement | ◐ | ◐ | — | — | — |
| Produire / appliquer un delta | ◐ | ◐ | — | ◐ (graphe) | — |
| Dictionnaires zstd par catégorie | ● | ● (préréglages) | — | — | — |
| Recompression réversible Deflate / JPEG (E3) | ◐ | ◐ | — | — | — |
| Chiffrement (BLAKE3 keyed, AES) | ◐ | ◐ | — | — | ○ |
| Profil d'accès et `optimize` (E1) | ○ | ○ | — | — | — |
| GPU au build (E4) | ○ | ○ | — | — | — |
| Partager (export volumes) | ● | ◐ | — | — | — |
| Partager par lien / QR | — | ○ | — | ◐ | ○ |
| Compte, appareils, licences | — | ◐ | — | ◐ | ○ |
| Télécharger l'app | — | — | ● | — | — |
| Explication produit, vidéo, FAQ | — | — | ● | — | — |
| Comparatif, benchmarks publiés | — | — | ◐ | — | — |
| Documentation format et CLI | — | — | ◐ | — | — |
| Télémétrie opt-in, diagnostics | ● | ● | — | ◐ | ○ |

## 8. Décisions à trancher

| # | Décision | Options | Recommandation | Échéance |
|---|---|---|---|---|
| D1 | Stack par surface | voir `docs/produit/stack.md` | Rust core + Tauri + Astro + Flutter | avant le premier commit de code |
| D2 | Authentification du compte web | Auth déléguée (Clerk / Auth0 / Supabase Auth) vs auto-hébergée (Better Auth / Lucia sur la stack web) | Auto-hébergée (Better Auth) : magic link + passkeys, zéro dépendance SaaS, RGPD simple | V1 |
| D3 | Hébergement landing | Pages statiques (Cloudflare Pages / GitHub Pages) vs VPS OVH existant derrière Caddy | Statique sur Cloudflare Pages (gratuit, CDN) ; repli VPS Caddy déjà en place | MVP |
| D4 | Hébergement API / console | VPS OVH (Docker + Caddy, RAM limitée 3,7 Go) vs PaaS (Fly.io / Railway) vs AWS eu-west-3 | VPS OVH pour la V1 (coût nul, infra connue), limites mémoire déclarées ; migrer si trafic | V1 |
| D5 | Stockage des morceaux hébergés | Aucun (métadonnées seules) vs S3 compatible (OVH Object Storage / R2) | Métadonnées seules en V1 ; S3 compatible quand le partage par lien exige l'hébergement | V1 → Plus tard |
| D6 | Modèle économique | Open core vs tout libre + services vs propriétaire | **Tranché 2026-11-16 (ADR-0001)** : open core, format + `vault-core` + CLI libres, apps et console payantes | — |
| D7 | Licence du cœur | MIT / Apache-2.0 vs MPL-2.0 vs AGPL | **Tranché 2026-11-16 (ADR-0001)** : Apache-2.0, fichier `LICENSE` à la racine | — |
| D8 | Voie Windows par défaut | WinFsp (montage) vs ProjFS + WOF (hydratation) | WinFsp par défaut, ProjFS + WOF proposé dès qu'un exécutable de jeu est détecté ; trancher sur H7 | après matrice H7 |
| D9 | Signature et distribution desktop | Certificat EV Windows + notarisation Apple vs non signé (MVP) | Non signé en MVP (utilisateurs techniques), signé en V1 | V1 |
| D10 | Télémétrie | Aucune vs opt-in auto-hébergée (Umami / OpenTelemetry) | Opt-in, auto-hébergée, agrégée ; jamais de chemin de fichier | MVP |
| D11 | Nom et domaine | « VAULT » est très générique (HashiCorp Vault, Obsidian « vault ») | **Tranché 2026-11-16 (ADR-0002)** : nom public **tholosvault**, éditeur Trigenys Group, CLI `tholos` ; VAULT reste le nom de travail interne. Détail : `docs/produit/nom.md` | — |
| D12 | Téléchargement d'archives distantes | Aucun (navigateur / `curl` puis `verify`) vs téléchargeur générique (`trauma`, `aria2c`) vs téléchargeur conscient du format | **Proposé 2026-11-20 (ADR-0005)** : crate `vault-fetch` (cœur agnostique du transport) + `vault-fetch-http` ; segments sur frontières de morceaux, vérification BLAKE3 par morceau, reprise, lecture partielle ; `tholos fetch` en MVP | MVP (CLI) → V1 |

## 9. Hors périmètre déclaré

Écriture en place (overlay), montage sur mobile, décompression GPU, compression neuronale, adaptateur EROFS/composefs,
dédup inter-archives distante : hors MVP et hors V1, conformément au rapport. Toute entrée dans ces sujets passe par un ADR.
Réseau : le téléchargement d'archives tholosvault est entré au périmètre par l'ADR-0005 (proposé) ; restent hors
périmètre BitTorrent / P2P, extension navigateur, sites vidéo, téléchargement de fichiers non tholosvault, upload.
