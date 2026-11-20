# Téléchargement de fichiers : mécanique, innovations, goulots d'étranglement

Rapport documentaire au service de l'ADR-0005 (téléchargeur segmenté conscient du format). Sources
consultées le 2026-11-20 ; les numéros entre crochets renvoient à la liste finale.

**BLUF.** Le débit d'un téléchargement est borné par `min(bande passante du goulot, fenêtre / RTT)` ; les
connexions parallèles n'aident que pour sommer des fenêtres TCP encore en rampe ou contourner un plafond par
connexion, et deviennent nuisibles une fois le lien, le disque ou le CPU saturé. Trois innovations se sont
imposées en cinquante ans : la **reprise** (ZMODEM 1986, HTTP `Range` 1997), la **vérification par morceau**
(ed2k, BitTorrent 2003, Metalink 2010, Bao/BLAKE3 2020, Xet 2024–2026) et le **découpage par contenu**
(rsync 1996, FastCDC 2016, casync 2017). Les goulots qui restent sont le coût par requête (d'où la
coalescence des plages), la politesse serveur (429, `Retry-After`) et la reprise fragile sur ETag faible. Un
téléchargeur qui connaît l'index et les empreintes de l'archive résout ces trois points sans dépendre du
serveur : c'est la thèse de l'ADR-0005.

## 1. Mécanique d'un téléchargement aujourd'hui

**Résolution et connexion.** Un téléchargement HTTP commence par une résolution DNS, puis une poignée de main
TCP (SYN, SYN-ACK, ACK : un aller-retour complet avant le premier octet utile), puis une négociation TLS (un
aller-retour en TLS 1.3, zéro en reprise 0-RTT). Chaque étape coûte au moins un RTT ; sur un lien
transatlantique à 100 ms, un simple GET paie donc 200 à 300 ms avant la première donnée.

**Slow start et fenêtre.** TCP n'envoie pas tout de suite au débit du lien : il démarre avec une fenêtre
initiale qui, depuis RFC 6928 (avril 2013, expérimental), est bornée à `min(10·MSS, max(2·MSS, 14600))`
octets, soit ~14,6 Ko, contre 2 à 4 segments auparavant (RFC 3390) [1]. La fenêtre double ensuite à chaque
RTT jusqu'à la perte ou au plafond. Le plafond pertinent est le **produit délai × bande passante (BDP)** : à
100 Mbit/s et 100 ms de RTT, il faut 1,25 Mo « en vol » pour remplir le lien [2]. La fenêtre TCP d'origine
est codée sur 16 bits (65 535 octets) ; RFC 7323 constate que lorsque le BDP dépasse 64 Kio, la fenêtre de
réception limite le débit maximal, d'où l'option de mise à l'échelle (facteur de décalage 0–14, fenêtre
jusqu'à ~1 Go) [3]. Exemple souvent cité : 64 Ko de fenêtre sur 100 ms de RTT plafonnent à ~5 Mbit/s quelle
que soit la capacité [4].

**Contrôle de congestion.** Reno (RFC 5681) augmente la fenêtre d'au plus un MSS par RTT en évitement de
congestion ; CUBIC (RFC 9438, août 2023, remplace RFC 8312) la fait croître selon une fonction cubique et est
le défaut de Linux, Windows et Apple [5]. BBR (Google, ACM Queue sept.-oct. 2016) ne réagit plus à la perte
mais modélise le goulot (bande passante et RTT de propagation) ; BBRv3 est en cours de normalisation à l'IETF
(brouillon mars 2026) [6][7].

**HTTP/1.1 et les plages.** La requête partielle existe depuis HTTP/1.1 (RFC 2068, janvier 1997) : en-tête
`Range: bytes=a-b`, réponse `206 Partial Content`, `Accept-Ranges: bytes` côté serveur [8]. La reprise sûre
repose sur `If-Range` : d'après RFC 9110 §13.1.5, il porte une date ou un ETag, et un ETag faible ne
correspond jamais ; seul un validateur fort garantit l'équivalence binaire, sinon le serveur renvoie 200 avec
la représentation entière [9][10]. Les navigateurs ouvrent typiquement 6 connexions par origine en HTTP/1.1,
alors que RFC 2616 en recommandait 2 [11][12].

**HTTP/2.** RFC 7540 (mai 2015) multiplexe plusieurs flux sur une seule connexion TCP, mais hérite du
blocage en tête de file (HoL) de TCP : un segment perdu bloque tous les flux jusqu'à sa retransmission [13].

**HTTP/3 et QUIC.** QUIC (RFC 9000–9002, mai 2021) et HTTP/3 (RFC 9114, juin 2022) passent sur UDP, offrent
un multiplexage sans HoL entre flux et une reprise 0-RTT (données rejouables, sans confidentialité
persistante) [14][15]. Chrome l'a activé par défaut en version 93 [14].

**CDN et caches.** Un CDN sert les plages depuis son cache et ne va à l'origine que pour les octets
manquants. Cloudflare décrit (octobre 2026) le dilemme des tranches : trop petites, une requête client
devient une rafale de petites requêtes vers l'origine qui bloquent chacune de la mémoire ; trop grandes
(64 Mio pour R2), une lecture de quelques Mo qui chevauche une frontière coûte 128 Mio de transfert
inter-datacenters. Leur refonte aligne les plages sur les blocs déjà stockés ; le p99 du temps au premier
octet a été divisé par deux environ [16].

**Écriture disque côté client.** Un téléchargeur multi-connexions écrit à des offsets disjoints ; il doit
préallouer (`posix_fallocate`, quasi instantané sur ext4/XFS/Btrfs) ou tronquer (`ftruncate`, fichier
creux). aria2 expose ces modes (`none`, `prealloc`, `falloc`, `trunc`) et avertit que la préallocation bloque
le processus sur les systèmes de fichiers anciens [17][18]. Les fichiers creux écrits dans le désordre
fragmentent [19].

## 2. Chronologie des innovations

- **1971** — RFC 114 (16 avril), première ébauche du *File Transfer Protocol* ; **1985** — RFC 959 (octobre),
  FTP devient standard Internet [20][21].
- **1977** — XMODEM (Ward Christensen) : paquets de 128 octets avec somme de contrôle [22].
- **1986** — ZMODEM (Chuck Forsberg), description publiée sur comp.sources.unix en mai 1986 ; la documentation
  DSZ décrit le « Crash Recovery » pour achever un transfert interrompu, mais aucune source datée de 1986 ne
  confirme que la fonction était présente dès la première version [23][24].
- **1996** — rsync (Tridgell et Mackerras), annoncé le 19 juin 1996 ; rapport TR-CS-96-05 sur la somme de
  contrôle glissante [25][26].
- **1997** — RFC 2068 (janvier), HTTP/1.1 avec `Range` et 206 [8]. GetRight 1.0 (février 1997) : la reprise
  était la fonctionnalité d'origine ; le téléchargement segmenté depuis plusieurs serveurs exige un fichier
  ≥ 100 Ko [27][28]. Go!Zilla, FlashGet, Download Accelerator Plus et IDM suivent à la fin des années 1990 ;
  aucune source primaire datée n'a été trouvée pour leurs premières versions [29].
- **~2001–2003** — BitTorrent (Bram Cohen) : le papier du 22 mai 2003 décrit les pièces hachées en SHA-1, un
  pair n'annonçant une pièce qu'après vérification, et la stratégie *rarest first*. La date 2001 est
  couramment citée mais non confirmée par les sources consultées [30][31].
- **eDonkey/ed2k** — hachage par blocs de 9 728 000 octets en MD4, puis MD4 de la liste des hachés ; les
  implémentations divergent sur le cas d'un fichier multiple exact du bloc [32][33].
- **2005** — zsync (Colin Phipps) : le client calcule les blocs manquants localement et ne demande que ceux-là
  en `Range` ; papier daté du 30 janvier 2005 [34]. Free Download Manager : date 2004 non confirmée par une
  source primaire.
- **2010** — Metalink, RFC 5854 (juin) : description XML multi-sources avec hachés par morceau ; RFC 6249
  (juin 2011) transporte miroirs et hachés dans les en-têtes HTTP ; aria2 le supporte depuis 1.12.1 [35][36][37].
- **2013** — RFC 6928, fenêtre initiale TCP à 10 segments [1].
- **2015** — HTTP/2, RFC 7540 (mai) [13]. Git LFS : pointeur `oid sha256:` et stockage adressé par contenu [38].
- **2016** — FastCDC (USENIX ATC, juin) : hachage Gear simplifié, saut sous la taille minimale, découpage
  normalisé ; ~10× plus rapide que Rabin [39]. BBR (ACM Queue, sept.-oct.) [6].
- **2017** — casync (Poettering, 20 juin) : images découpées par contenu, frontières de fichiers supprimées
  avant découpage ; desync (Go) le réimplémente avec découpage parallèle [40][41][42].
- **2020** — BLAKE3, annoncé début janvier 2020 (v1.0 de l'implémentation de référence en 2021) : arbre de
  Merkle sur morceaux de 1 Kio, parallélisable [43][44]. Bao implémente la vérification en flux (§6.4 de la
  spécification) : décodage d'une tranche vérifiable contre la racine ; utilisé par iroh-blobs [45][46].
- **2021–2022** — QUIC RFC 9000 (mai 2021), HTTP/3 RFC 9114 (juin 2022) [14][15].
- **2023** — CUBIC RFC 9438 (août) [5].
- **2024–2026** — Hugging Face Xet : « From Files to Chunks » (20 novembre 2024), « From Chunks to Blocks »
  (12 février 2025) ; morceaux CDC de ~64 Ko identifiés par haché cryptographique, agrégés en blocs de 64 Mo ;
  migration du Hub depuis Git LFS. Le brouillon IETF `draft-denis-xet` (déc. 2025, rév. 04 juin 2026) fixe
  Gearhash + BLAKE3 comme suite par défaut [47][48][49][50][51].

## 3. Goulots d'étranglement

**Limitation de débit par connexion (serveur/CDN).** Certains hébergeurs plafonnent chaque connexion ; quatre
connexions donnent quatre fois le plafond. Lawrence (nov. 2024) rappelle qu'en théorie le parallélisme ne
devrait jamais être plus rapide (poignées de main, slow start, TLS en plus) et n'aide que dans deux cas :
plafond par connexion, et HoL TCP sur réseau instable. Edge 148 active le parallélisme par défaut avec
3 requêtes sur un fichier de 24 Go [52]. Parade : N connexions, borné (§4) et poli (ci-dessous).

**Latence et coût par requête.** Chaque `Range` paie un RTT plus les en-têtes ; un lecteur naïf qui suit
l'index d'une archive émet une requête par petit bloc (ratarmount-rs : ~9 000 requêtes `Range` pour 128 Mio,
soit ~14 Kio par requête, avant coalescence). Cloudflare documente le même phénomène côté CDN [16]. Parade :
**coalescer** les plages adjacentes ou proches (zsync-rs et zsync2 fusionnent sous un trou de 256 Kio ;
oxigdal-streaming utilise un seuil d'écart configurable) et viser des requêtes de l'ordre du Mio [53][54].

**Slow start et BDP sur lien long.** Avec 14,6 Ko initiaux et un doublement par RTT, atteindre 1,25 Mo de
fenêtre demande ~7 RTT, soit 0,7 s de rampe par connexion neuve à 100 ms ; sans *window scaling*, le
plafond est ~5 Mbit/s [1][2][3][4]. Parade : réutiliser les connexions (keep-alive, HTTP/2, QUIC 0-RTT), ou
plusieurs connexions pour sommer les fenêtres.

**Head-of-line blocking.** HTTP/2 sur TCP : une perte bloque tous les flux [13]. Parade : HTTP/3, ou
connexions TCP distinctes.

**Dernier kilomètre et asymétrie.** Les accès résidentiels ont un débit montant inférieur au descendant ;
ACK et requêtes partent sur la voie étroite. Le lien d'accès est souvent le vrai plafond [52].

**Plafonds de données.** Sur mobile ou lien facturé, chaque octet refetché compte ; Cloudflare chiffre le
gaspillage d'un recouvrement à 128 Mio pour quelques Mo utiles [16].

**Écriture disque, fichiers creux, fsync.** Écriture désordonnée sur fichier creux → fragmentation ;
préallocation `fallocate` quasi gratuite sur ext4/XFS/Btrfs mais bloquante sur ext3 [17][18][19]. Un `fsync`
par morceau est coûteux ; parade : journal d'état séparé (bitmap des morceaux vérifiés) synchronisé
périodiquement.

**CPU : hachage et déchiffrement.** Mesures 2025 (AMD EPYC 4245P, Zen 5, entrées de 1 Mo) : BLAKE3
~13,2 Go/s contre SHA-256 ~2,4 Go/s par cœur [55]. Sur Apple M1 (16 Mio, un fil), SHA-256 accéléré
matériellement fait 2,0–2,3 Go/s contre 1,3–1,8 Go/s pour BLAKE3 mono-fil (4,1–6,3 Go/s sur 8 cœurs) [56] :
**les sources divergent selon la présence d'extensions SHA**. Dans tous les cas le hachage par cœur dépasse le
débit d'un lien gigabit (125 Mo/s) d'un ordre de grandeur ; il ne devient goulot qu'au-delà de 10–25 Gbit/s
ou si le déchiffrement s'ajoute.

**Mémoire tampon.** N connexions × fenêtre de réception + morceaux en attente de vérification ; un morceau
n'est écrit qu'une fois complet et vérifié, donc la mémoire croît avec N × taille de segment. Parade : borner
N et la taille de plage.

**Équité et politesse.** RFC 6585 (avril 2012) définit `429 Too Many Requests` et recommande `Retry-After`
(RFC 9110 §10.2.3) [57][58]. Trente-deux connexions par client multiplient descripteurs, mémoire et requêtes
vers l'origine [16] ; les serveurs refusent alors les `Range`, rejettent les connexions supplémentaires ou
répartissent le plafond entre elles [52]. Parade : 4–8 connexions, repli exponentiel sur 429/503, honorer
`Retry-After`.

**Corruption silencieuse.** CERN (2007) : un fichier de 2 Go écrit toutes les deux heures sur > 3 000 nœuds
pendant cinq semaines a produit 500 erreurs sur 100 nœuds, fortement corrélées à la mémoire défectueuse ; les
taux observés dépassaient largement les 10⁻¹⁴ spécifiés pour les disques [59][60]. Vérifier le fichier entier
oblige à tout relire et, en cas d'échec, à tout retélécharger ; la vérification par morceau (BitTorrent,
Metalink, Bao) localise et ne refetch que le morceau fautif [30][35][45].

**Reprise cassée.** Un ETag faible (`W/`) est refusé par `If-Range` ; si le fichier a changé, le serveur
renvoie 200 entier et le client doit détecter ce basculement sous peine de concaténer deux versions [9][10].
Parade : exiger un ETag fort ou, mieux, vérifier chaque morceau contre le Merkle attendu : la racine BLAKE3 de
l'archive rend l'ETag secondaire.

## 4. Bornes physiques et mathématiques

Débit effectif = min(bande passante du goulot, fenêtre / RTT) [2][3]. Shannon fixe la capacité d'un canal à
`C = B·log₂(1 + S/N)` : au-delà, aucune ingéniosité protocolaire n'ajoute de bits. Ajouter des connexions
somme les fenêtres (utile tant que `N × fenêtre / RTT < bande passante`) et contourne les plafonds par
connexion ; une fois le lien, le disque ou le CPU de hachage saturé, le gain est nul et le surcoût (poignées
de main, slow start, mémoire, charge serveur) devient net [52]. En pratique, N optimal ≈ BDP / fenêtre
atteignable par connexion, borné par la politesse (§3).

## 5. Non résolu, tendances 2025–2026

- **Vérification intégrée au transport.** HTTP ne transporte pas de hachés par morceau ; Metalink/HTTP
  (RFC 6249) reste peu déployé [36]. Bao/BLAKE3 (iroh-blobs) et Xet (brouillon IETF 2025–2026) poussent la
  vérification au niveau du morceau, indépendamment du serveur [45][46][51].
- **Téléchargement partiel exploitable.** Bao décode des tranches vérifiables sans le fichier entier [45] ;
  casync/desync et Xet reconstruisent à partir de morceaux adressés par contenu [40][47]. Le montage « avant
  la fin » suppose que l'index et les morceaux demandés arrivent et se vérifient en premier ; ratarmount
  montre que l'ordonnancement et la coalescence restent à concevoir [16][53].
- **Stores adressés par contenu sur stockage statique.** casync, desync et Xet servent des morceaux immuables
  depuis des objets statiques (HTTP/S3) ; Hugging Face a migré tout son Hub vers Xet avec des blocs de 64 Mo
  pour amortir le coût par requête [47][48][50].
- **HTTP/3 et CDN.** QUIC supprime le HoL, mais les CDN alignent encore leurs tranches de cache sur des blocs
  fixes (Cloudflare, 2026) ; un téléchargeur conscient du format a intérêt à aligner ses plages sur ces
  blocs pour éviter les recouvrements [14][16].
- **Ouvert.** N adaptatif (mesurer BDP et 429 plutôt que fixer 8 ou 32) ; politesse normalisée (`RateLimit-*`
  toujours en brouillon [61]) ; équilibre taille de morceau CDC (dédup) vs taille de requête HTTP (latence),
  que Xet résout par une couche de blocs intermédiaire [48].

## 6. Implications pour `vault-fetch` (ADR-0005)

| Constat | Conséquence de conception |
|---|---|
| Coût par requête, rampe TCP (§1, §3) | Segments de 4–16 Mio couvrant plusieurs morceaux contigus ; jamais une requête par morceau ; connexions persistantes |
| Plafond par connexion, politesse (§3, §4) | Défaut 8 connexions, plafond configurable ; repli exponentiel, `Retry-After` honoré ; N réduit quand le serveur renvoie 429/503 ou refuse `Range` |
| Reprise fragile sur ETag faible (§3) | Bitmap des morceaux vérifiés dans `.part` ; ETag fort exigé pour reprendre sans revérifier ; la racine Merkle fait foi |
| Vérification fichier entier coûteuse (§3) | BLAKE3 par morceau à l'arrivée, redemande du seul morceau rejeté ; racine recalculée à la fin (comme xet-core) |
| Tranches de cache CDN fixes (§1, §5) | Plages alignées sur des multiples de puissance de deux quand l'archive le permet ; mesure à ajouter au banc |
| Hachage ≫ débit réseau (§3) | Vérification synchrone dans le pipeline de réception, pas d'étage différé |
| Écriture creuse et fsync (§3) | Fichier préalloué à la taille finale, écriture aux offsets des frames, état `.part` écrit atomiquement au plus une fois par seconde |
| Tendance « morceaux immuables sur stockage statique » (§5) | Un fichier `.vault` unique sur un serveur statique avec `Range` suffit ; pas de service d'hébergement requis (D5 inchangé) |

## Sources

1. https://www.rfc-editor.org/rfc/rfc6928 — RFC 6928, Increasing TCP's Initial Window (avril 2013).
2. https://en.wikipedia.org/wiki/Bandwidth-delay_product — Bandwidth-delay product.
3. https://www.rfc-editor.org/info/rfc7323 — RFC 7323, TCP Extensions for High Performance (sept. 2014).
4. https://oneuptime.com/blog/post/2026-03-20-tcp-window-size-throughput-impact/view — TCP Window Size and Throughput (2026-03-20).
5. https://www.rfc-editor.org/rfc/rfc9438.html — RFC 9438, CUBIC (août 2023).
6. https://research.google/pubs/pub45646/ — Cardwell et al., BBR: Congestion-Based Congestion Control, ACM Queue (2016).
7. https://datatracker.ietf.org/doc/html/draft-ietf-ccwg-bbr — Internet-Draft BBR (mars 2026).
8. https://www.rfc-editor.org/rfc/rfc2068 — RFC 2068, HTTP/1.1 (janvier 1997).
9. https://http.dev/if-range — If-Range (RFC 9110 §13.1.5).
10. https://lists.w3.org/Archives/Public/ietf-http-wg/2007AprJun/0108.html — ietf-http-wg, If-Range et ETag forts (2007).
11. https://stevesouders.com/blog/2008/03/20/roundup-on-parallel-connections/ — Roundup on Parallel Connections (2008).
12. https://fitech101.aalto.fi/designing-and-building-scalable-web-applications/dab-15-content-delivery-networks/1-some-http-limitations — Some HTTP limitations.
13. https://www.rfc-editor.org/rfc/rfc7540 — RFC 7540, HTTP/2 (mai 2015).
14. https://www.chromium.org/quic — QUIC, a multiplexed transport over UDP.
15. https://www.rfc-editor.org/rfc/rfc9114 — RFC 9114, HTTP/3 (juin 2022).
16. https://blog.cloudflare.com/origin-range-requests/ — Origin Range Requests (2026-10-23).
17. https://wiki.archlinux.org/title/aria2 — aria2, `--file-allocation`.
18. https://manpages.debian.org/wheezy/aria2/aria2c.1.en.html — aria2c(1).
19. https://forum.rclone.org/t/fragmentation-issue-with-multithread/51936 — Fragmentation issue with multithread.
20. https://www.rfc-editor.org/rfc/rfc114 — RFC 114, File Transfer Protocol (16 avril 1971).
21. https://www.rfc-editor.org/rfc/rfc0959 — RFC 959, FTP (octobre 1985).
22. https://www.historyofinformation.com/detail.php?id=4992 — Ward Christensen Writes XMODEM (1977).
23. https://www.tuhs.org/Usenet/comp.sources.unix/1986-May/004372.html — ZMODEM Protocol Description (mai 1986).
24. https://en.wikipedia.org/wiki/Chuck_Forsberg — Chuck Forsberg.
25. https://en.wikipedia.org/wiki/Rsync — rsync.
26. https://www.andrew.cmu.edu/course/15-749/READINGS/required/cas/tridgell96.pdf — Tridgell, Mackerras, The rsync algorithm (1996).
27. https://getright.com/getright_history.html — GetRight history.
28. https://getright.com/segmented.html — GetRight segmented downloading.
29. https://en.wikipedia.org/wiki/Go!Zilla — Go!Zilla.
30. https://www.cs.princeton.edu/courses/archive/fall14/cos561/papers/BitTorrent03.pdf — Cohen, Incentives Build Robustness in BitTorrent (2003).
31. https://sites.cs.ucsb.edu/~almeroth/classes/W10.290F/exams/03.pdf — Revue du papier BitTorrent.
32. https://wiki.anidb.net/Ed2k — Ed2k hash.
33. https://wiki.amule.org/wiki/MD4_hash — aMule, MD4 hash.
34. https://zsync.moria.org.uk/paper200501/index.html — Phipps, zsync (2005-01-30).
35. https://www.rfc-editor.org/rfc/rfc5854 — RFC 5854, Metalink (juin 2010).
36. https://datatracker.ietf.org/doc/rfc6249/history/ — RFC 6249, Metalink/HTTP (juin 2011).
37. https://sourceforge.net/p/aria2/news/?page=1 — aria2 news.
38. https://github.com/git-lfs/git-lfs/blob/release-2.4/docs/spec.md — Git LFS spec.
39. https://www.usenix.org/conference/atc16/technical-sessions/presentation/xia — Xia et al., FastCDC, USENIX ATC '16.
40. https://lwn.net/Articles/726625/ — Poettering announces casync (2017-06-20).
41. https://sources.debian.org/src/casync/2%2B20201210-2/README.md — casync README.
42. https://pkg.go.dev/github.com/folbricht/desync — desync.
43. https://github.com/BLAKE3-team/BLAKE3 — BLAKE3.
44. https://phoronix.com/forums/node/1269466 — BLAKE3 v1.0 Released (2021).
45. https://github.com/oconnor663/bao — Bao, BLAKE3 verified streaming.
46. https://rikettsie.github.io/posts/bao-verified-streaming/ — Bao verified streaming et iroh-blobs.
47. https://huggingface.co/blog/from-files-to-chunks — From Files to Chunks (2024-11-20).
48. https://huggingface.co/blog/from-chunks-to-blocks — From Chunks to Blocks (2025-02-12).
49. https://huggingface.co/docs/hub/xet/deduplication — Xet deduplication.
50. https://huggingface.co/blog/migrating-the-hub-to-xet — Migrating the Hub from Git LFS to Xet.
51. https://datatracker.ietf.org/doc/html/draft-denis-xet-04 — Internet-Draft XET (juin 2026).
52. https://textslashplain.com/2024/11/22/parallel-downloading/ — Lawrence, Parallel Downloading (2024, mis à jour 2026-05-29).
53. https://docs.rs/zsync-rs/0.1.2/src/zsync_rs/http.rs.html — zsync-rs, fusion des plages.
54. https://docs.rs/crate/oxigdal-streaming/0.1.4/source/src/io_coalescing.rs — oxigdal-streaming, io_coalescing.
55. https://mojoauth.com/security-guides/sha-256-vs-blake3.md — SHA-256 vs BLAKE3 (mesures 2025, EPYC 4245P).
56. https://mojoauth.com/security-guides/sha3-256-vs-blake3 — BLAKE3 vs SHA3 (mesures Apple M1).
57. https://developer.mozilla.org/docs/Web/HTTP/Reference/Status/429 — 429 Too Many Requests.
58. https://zuplo.com/learning-center/http-429-too-many-requests-guide — HTTP 429 guide.
59. https://storagemojo.com/2007/09/19/cerns-data-corruption-research/ — CERN's Data Corruption Research (2007).
60. https://indico.desy.de/event/257/contributions/58082/attachments/37574/46878/kelemen-2007-HEPiX-Silent_Corruptions.pdf — Kelemen, Silent Corruptions, HEPiX (2007).
61. https://datatracker.ietf.org/doc/id/draft-polli-ratelimit-headers-05.html — RateLimit Header Fields for HTTP (brouillon).

Toutes les sources ont été consultées le 2026-11-20.
