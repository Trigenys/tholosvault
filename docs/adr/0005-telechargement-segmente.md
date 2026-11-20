# ADR-0005 — Téléchargement segmenté conscient du format (`vault-fetch`)

- **Statut** : proposé
- **Date** : 2026-11-20
- **Contexte** : le propriétaire veut récupérer une archive `.vault` « en une fois », à la manière de
  Free Download Manager : reprise après coupure, plusieurs connexions HTTP en parallèle (`Range`), file
  d'attente, limitation de débit. Le réseau n'est pas dans le périmètre déclaré du MVP
  (`docs/produit/cadrage-surfaces.md`, §9) : toute entrée passe par un ADR. Ce qui compte :
  - **Le format rend le téléchargeur meilleur qu'un téléchargeur générique.** Chaque morceau a son empreinte
    BLAKE3 dans la table de hachage, les frames zstd sont adressées par l'index, l'arbre de Merkle donne une
    racine qui identifie la version (`docs/format/spec-v1.md`). Un téléchargeur générique vérifie un SHA-256
    du fichier entier à la fin et redemande tout en cas d'échec ; ici un morceau corrompu (de 64 à 256 KiB) est
    détecté à l'arrivée et seul ce morceau est redemandé.
  - **Agencement du fichier** : `en-tête (64) │ frames zstd │ index │ table fichiers │ table hachage │ parité │
    trailer ×2`. Les tables sont en **queue** de fichier : « l'index d'abord » signifie lire le trailer
    (`Range: bytes=-256`), puis les tables, avant le premier octet de contenu.
  - **Lecture par plage existante** : `Archive::read_range` et le `ChunkCache` (PR #9) lisent un fichier par
    morceaux ; le montage FUSE (`vault-mount`) en dépend. Une archive partiellement reçue est donc lisible dès
    que les tables et les morceaux demandés sont là, pourvu que le lecteur sache demander un morceau absent.
  - **Contraintes** : cœur unique `vault-core`, apps = adaptateurs minces ; Rust ; open core Apache-2.0
    (ADR-0001) ; aucun service réseau Trigenys requis pour télécharger depuis une URL quelconque (D4/D5 restent
    « métadonnées seules » en V1) ; RAM modeste (le téléchargeur doit tourner sur le VPS 3,7 Go comme sur un
    poste).

## Options considérées

Reuse-first (recherche GitHub du 2026-11-20, `gh search repos --language rust`) :

| Brique | Ce qu'elle apporte | Ce qui manque | Décision |
|---|---|---|---|
| `rgreinho/trauma` (★180, MIT, lib, `reqwest` 0.13 + `reqwest-retry`) | Reprise si le serveur l'accepte, file de téléchargements, progression | Une connexion par fichier, pas de segments, pas de point d'entrée pour vérifier un segment | **Learn** (découpage API, retries par middleware) |
| `harshsinghsv/swiftdl` (★804, MIT, app Tauri), `FortoGraphy/prism-download-manager` (★973, MIT, Tauri), `anefzaoui/misterdownloadmanager` (★13 968, GPL-3.0, egui) | Moteurs multi-connexions mûrs : jusqu'à 32 connexions, état sauvé chaque seconde, contrôle que le fichier distant n'a pas changé (ETag), limitation de débit, files, planification | Applications, pas bibliothèques ; vérification par fichier entier (SHA-256) ou aucune ; GPL pour la troisième (incompatible avec le cœur Apache-2.0) | **Learn** (ordonnanceur de segments, persistance d'état, détection de changement distant) |
| `aria2/aria2` (★43 571, GPL-2.0, C++, dernière release 1.37.0 du 2023-11-15) en sous-processus via RPC JSON | Tout le téléchargement générique est fait, multi-sources | Binaire externe à distribuer par OS, licence séparée, vérification par fichier entier (`--checksum`), pas de montage partiel, projet au ralenti | **Écarté** (Learn : politique de reprise et de politesse HTTP) |
| `seanmonstar/reqwest` (+ `rustls`, `tokio`) | Client HTTP async déjà dans la stack (API Axum, desktop Tauri), `Range`, flux, timeouts | Aucune logique de segments ni d'état : à construire | **Adopt** comme transport HTTP |

Options d'architecture :

1. **Aucun téléchargeur** : laisser navigateur / `curl` / `aria2c` récupérer le fichier, puis `tholos verify` et
   `tholos repair`. Pour : zéro code. Contre : une archive de 50 Go corrompue d'un octet exige un nouveau
   téléchargement complet ou une parité suffisante ; pas de reprise fiable ; pas de lecture avant la fin ;
   l'argument « on répare sans retélécharger » du produit n'est pas tenu au moment où il compte le plus.
2. **Téléchargeur générique maison** (`reqwest` + `Range`, segments de taille fixe, SHA du fichier à la fin),
   ou adoption de `trauma`. Pour : simple, réutilisable pour n'importe quel fichier. Contre : refait ce que
   FDM, Prism, SwiftDL font déjà mieux ; aucun avantage lié au format ; la vérification reste globale.
3. **Téléchargeur conscient du format** : un cœur `vault-fetch` agnostique du transport qui planifie les
   segments **sur les frontières de morceaux**, vérifie chaque morceau à l'arrivée avec la table de hachage,
   persiste l'état par morceau et expose les morceaux reçus au lecteur `vault-core` pour lecture et montage
   partiels. Pour : différenciateur réel, réutilise index, Merkle, parité et `read_range` ; même cœur pour CLI,
   desktop et console. Contre : ne télécharge que des `.vault` (hors périmètre volontaire) ; demande une
   abstraction de source de morceaux dans `vault-core`.

## Décision

Option 3 : construire un crate **`vault-fetch`** (cœur du téléchargement, agnostique du transport et de
l'interface) avec un adaptateur **`vault-fetch-http`** (`reqwest` + `rustls`), exposé par `tholos fetch` en
MVP et par un écran « Téléchargements » du desktop en V1 ; `vault-fetch-s3` plus tard, derrière le même
trait.

### Positionnement

Le téléchargeur n'est pas un gestionnaire de téléchargements généraliste : il récupère **des archives
tholosvault**, et tire parti du format à chaque étape :

| Étape | Téléchargeur générique | `vault-fetch` |
|---|---|---|
| Démarrage | `HEAD`, taille | `Range: bytes=-256` → trailer (CRC32C, racine, offsets des tables) → tables (index, fichiers, hachage) ; racine comparée à la racine attendue si fournie (`--root <hex>`, lien `/s/<id>`) |
| Découpage | Segments de taille fixe | Segments = suites contiguës de morceaux (cible 4–16 MiB), bornes = frontières de frames connues par l'index |
| Vérification | SHA du fichier à la fin | BLAKE3 de chaque morceau à l'arrivée ; échec → seul ce morceau est redemandé (autre connexion, puis autre source si disponible) ; après réception des tables, Merkle recalculé = racine |
| Reprise | Octets contigus reçus | Bitmap des morceaux vérifiés dans `<archive>.vault.part` ; relance = diff bitmap, zéro octet vérifié retéléchargé |
| Lecture avant la fin | Impossible | `read_range` servi dès que les tables et les morceaux visés sont vérifiés ; un morceau manquant est **demandé en priorité** (ordonnancement à la demande, comme un lecteur vidéo) ; montage FUSE / WinFsp en lecture seule sur l'archive partielle (V1) |
| Version voisine locale | — | Les morceaux dont l'empreinte existe déjà dans une archive locale (version précédente) sont copiés, pas téléchargés (V1, même logique que le delta) |
| Parité | — | La section de parité est téléchargée en dernier et reste facultative : en cours de téléchargement, réparer = redemander ; après, elle couvre la perte de la source |

### Découpage en crates

```
crates/
  vault-fetch/         # cœur : planificateur, état persistant, vérification, limiteur, file d'attente
  vault-fetch-http/    # adaptateur Transport HTTP/HTTPS (reqwest, rustls, Range, ETag, Retry-After)
  vault-fetch-s3/      # plus tard : GetObject + Range (OVH Object Storage, R2), même trait
  vault-cli/           # `tholos fetch` (MVP)
  vault-core/          # ajout (PR séparée) : source de morceaux abstraite pour la lecture partielle
```

- `vault-fetch` ne connaît ni HTTP ni S3 : il reçoit un `Transport` et une destination, et produit des
  événements (progression, morceau vérifié, morceau rejeté, terminé, erreur actionnable). Il réutilise
  `vault-core` pour analyser trailer et tables et pour hacher les morceaux : **aucune logique de format n'est
  dupliquée**.
- `vault-core` reçoit, dans une PR distincte coordonnée avec le chantier « débit FUSE » en cours sur
  `vault-mount`, une abstraction de source de morceaux (le fichier local complet en est la première
  implémentation) ; `vault-fetch` en fournit une seconde : « morceau présent et vérifié ? sinon demander et
  attendre ». Rien n'est modifié dans `vault-mount` par cet ADR.

### API cible (esquisse, identifiants anglais)

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    /// Taille, support de `Range`, identifiant de version distante (ETag / Last-Modified / version S3).
    async fn probe(&self, source: &Source) -> Result<RemoteInfo>;
    /// Flux d'octets pour [start, end) ; l'implémentation gère ses retries de bas niveau.
    async fn get_range(&self, source: &Source, range: Range<u64>) -> Result<ByteStream>;
}

pub struct FetchOptions {
    pub connections: usize,          // défaut 8, borné par le serveur et par `probe`
    pub segment_bytes: u64,          // cible 8 MiB, arrondie aux frontières de morceaux
    pub rate_limit: Option<Bytes/s>, // seau à jetons global, partagé par la file
    pub expected_root: Option<Hash>, // racine Merkle attendue ; mismatch = arrêt avant tout contenu
    pub fetch_parity: bool,          // défaut true, en dernier
    pub resume: bool,                // défaut true : lit `<dest>.part`
}

pub struct Fetcher { /* état persistant, planificateur, limiteur */ }
impl Fetcher {
    pub fn plan(&self) -> Plan;                 // morceaux manquants, segments, octets à transférer (lecture seule)
    pub async fn run(&self, events: Sender<Event>) -> Result<Summary>;
    pub fn pause(&self); pub fn resume(&self);
    pub fn prioritize(&self, chunks: Range<ChunkId>); // demandé par la lecture partielle
}

pub struct Queue { /* N téléchargements, M actifs, limite de débit partagée, persistée */ }
```

Fichier d'état `<dest>.vault.part` : URL / source, taille, identifiant de version distante, racine, bitmap
des morceaux vérifiés, horodatage ; écrit atomiquement (fichier temporaire puis renommage) au plus toutes les
secondes et à chaque pause. Le fichier de destination est créé à sa taille finale (fichier creux) et écrit en
place aux offsets des frames ; à la fin, `.part` est supprimé et `tholos verify` rapide confirme la racine.

Comportements imposés :

- Serveur sans `Range` (`Accept-Ranges: none` ou 200 sur une requête 206) : une connexion, reprise impossible,
  message explicite ; la vérification par morceau reste active.
- Changement distant pendant le téléchargement (ETag / taille / racine différents) : arrêt, état conservé,
  erreur actionnable (« la source a changé, relancer avec `--restart` »).
- `429` / `503` + `Retry-After` : respecté ; backoff exponentiel plafonné sinon ; trois rejets BLAKE3 consécutifs
  d'un même morceau sur une même connexion → connexion fermée et rouverte.
- Limitation de débit : un seau à jetons par file, réparti entre connexions ; `--limit 5M` en CLI.
- Jamais de secret dans les journaux ni dans `.part` (en-têtes d'authentification injectés par l'adaptateur,
  non persistés).

### Priorités

| Niveau | Contenu |
|---|---|
| **MVP** | `tholos fetch <url> [--out] [--root] [--connections N] [--limit] [--no-parity]` : trailer puis tables, segments sur frontières de morceaux, vérification BLAKE3 par morceau, redemande ciblée, reprise par `.part`, limite de débit, repli une connexion ; `tholos fetch --plan` (lecture seule) |
| **V1** | File d'attente persistée (`tholos fetch add/list/pause/resume`), écran « Téléchargements » desktop (Tauri, même cœur), lecture et montage de l'archive partielle avec priorisation à la demande, réutilisation des morceaux d'une version locale voisine, `vault-fetch-s3`, source `/s/<id>` de la console (racine fournie par la page de partage) |
| **Plus tard** | Plusieurs sources pour une même racine (miroirs), planification horaire, intégration « Réparer depuis une URL » en ne demandant que les morceaux abîmés |

### Hors périmètre

BitTorrent et P2P, extension navigateur et capture de liens, sites vidéo et listes de lecture, téléchargement
de fichiers qui ne sont pas des archives tholosvault, envoi (upload) vers une console ou un bucket, et tout
service d'hébergement de morceaux (D5). Un téléchargeur généraliste existe déjà en Rust et en libre
(Prism, SwiftDL) ; tholosvault ne le refera pas.

## Conséquences

- **Impose** : deux nouveaux crates dans le workspace ; une abstraction de source de morceaux dans
  `vault-core` (PR dédiée, après le chantier FUSE) ; un serveur HTTP de test local (Axum, déjà dans la stack)
  qui simule 206 / 200, coupures, `Retry-After`, octets corrompus et changement d'ETag ; `appfactory-impact.json`
  : `fetch` dépend de `core`, déclenche `cli`.
- **Mesures à inscrire au banc** (candidate thèse H9, à proposer dans `docs/recherche/` si retenue) :
  un morceau corrompu coûte au plus 1 morceau + 1 requête supplémentaire (jamais l'archive) ; une reprise
  après coupure retélécharge 0 octet déjà vérifié ; premier `read_range` servi avant la fin du téléchargement ;
  débit agrégé sur N connexions ≥ 0,9 × le débit de `curl` parallèle sur le même serveur local.
- **Interdit** : toute logique de format en dehors de `vault-core` ; un `Transport` qui connaît le format ;
  une vérification « fichier entier à la fin » comme seul contrôle.
- **Vérification** : tests d'intégration contre le serveur local (corruption injectée, coupure à mi-segment,
  serveur sans `Range`, changement distant), test d'idempotence (`run` sur une archive complète = zéro requête
  de contenu), `clippy -D warnings`, banc.
- Mise à jour de `docs/produit/cadrage-surfaces.md` (surface, matrice, D12) et de `docs/produit/stack.md`
  (briques) dans la même PR que cet ADR.

## Dérogation RAIDER

- **A — Agnostic** : `vault-fetch` est volontairement **spécifique au format tholosvault** (il lit trailer,
  tables et empreintes). C'est le différenciateur de la surface, documenté ici ; l'agnosticité est maintenue
  là où elle a un sens : transport (HTTP, S3, fichier) et interface (CLI, desktop, console) derrière des traits.
- Entrée dans le réseau, hors périmètre déclaré du cadrage : autorisée par cet ADR, bornée aux archives
  tholosvault.
