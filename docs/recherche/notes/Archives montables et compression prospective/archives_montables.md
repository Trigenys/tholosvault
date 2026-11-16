# Archives montables et systèmes de fichiers compressés à accès aléatoire (état de l'art 2025-2026)

Notes denses, en français, termes techniques anglais conservés. Les chiffres sont ceux des sources primaires quand elles existent ; tout ce qui est estimé ou mesuré localement est marqué **[estimé]** ou **[mesure locale]**.

## Q1. Tailles de bloc (EROFS, squashfs, DwarFS, zstd-seekable) et compromis ratio/latence vs archives solides (7z/xz)

### Takeaway
Les systèmes montables compressent par blocs indépendants (4 KiB → 1 MiB pour EROFS/squashfs, 16-64 MiB pour DwarFS, frames libres pour zstd-seekable) ; la perte de ratio vs une archive solide est compensée chez DwarFS par la déduplication/segmentation, qui le rend *plus petit* que 7z/xz solide sur des corpus redondants, tout en montant en < 10 ms et en lisant à plusieurs GB/s.

### Cited Findings
- **squashfs** : bloc de données par défaut 128 KiB, maximum 1 MiB (« Block sizes greater than 4K are supported up to a maximum of 1Mbytes ») ; métadonnées (inodes, répertoires) compressées en blocs de 8 KiB ; tails de petits fichiers regroupés dans des *fragment blocks* ; compresseurs zlib, lz4, lzo, xz, zstd ; décompresseurs `single` (défaut), `multi` (2/cœur), `percpu` ; lecture seule, max fichier ≈ 2 TiB. — [Kernel docs squashfs](https://docs.kernel.org/filesystems/squashfs.html)
- **EROFS** : « fixed-sized output compression » (entrée variable → blocs compressés de taille fixe, pclusters), bloc 4 KiB (exemple), adressage 48 bits ; *big pcluster* depuis Linux 5.13 ; depuis 6.1 un pcluster peut contenir plusieurs extents → dédup de données compressées ; algos LZ4, MicroLZMA, DEFLATE, Zstandard sélectionnables par inode ; *tailpacking inline* ; décompression *in-place* avec stratégies de cache `disabled / readahead / readaround` ; écriture recommandée via overlayfs ; *chunk-based files* (5.15) non compressés. — [Kernel docs EROFS](https://docs.kernel.org/filesystems/erofs.html)
- **mkfs.erofs -C** : taille max du pcluster compressé en octets, défaut 4 KiB, > 4 KiB requiert big pcluster (Linux 5.13+) ; support noyau LZ4 5.3+, LZMA 5.16+, DEFLATE 6.6+, zstd 6.10+ ; avertissement officiel : « large pcluster size can cause bad random performance, so please evaluate carefully » ; pour comparer équitablement les tailles, utiliser `-Ededupe -E(all-)fragments`. — [man mkfs.erofs](https://www.mankier.com/1/mkfs.erofs) ; [EROFS FAQ](https://erofs.docs.kernel.org/en/latest/faq.html)
- **EROFS papier ATC'19** (Huawei + SJTU) : critique du « fixed-sized input compression » (squashfs) qui cause de l'amplification I/O ; EROFS réduit le boot time d'applis réelles jusqu'à 22,9 % et divise presque par deux le stockage ; déployé sur des dizaines de millions de smartphones. — [USENIX ATC'19 Gao et al.](https://www.usenix.org/conference/atc19/presentation/gao)
- **DwarFS** : bloc par défaut 16 MiB, 64 MiB avec `-l9` ; `-S20` = 1 MiB ; `-B3`/`-B64` autorisent les références à N blocs précédents ; zstd par défaut, lzma « about an order of magnitude slower to access on mounted images », brotli dispo. — [DwarFS README](https://github.com/mhx/dwarfs)
- **zstd seekable format** v0.1.0 : suite de frames zstd indépendants + un *skippable frame* final contenant la seek table (magic 0x184D2A5E, footer 9 octets, magic 0x8F92EAB1) ; entrée par frame 8 octets (12 avec checksum XXH64 tronqué 32 bits) ; overhead fixe 17 octets ; aucune taille de frame par défaut ni recommandation dans la spec. — [zstd_seekable_compression_format.md](https://github.com/facebook/zstd/blob/dev/contrib/seekable_format/zstd_seekable_compression_format.md)
- **Ratio vs taille de bloc (zstd niveau 1, données BDD, 2016, zstd 0.4.1)** : ≈ 3,95 à 4 KiB → ≈ 6,41 à 64 KiB → ≈ 7,17 à 256 KiB ; gain ≥ 20 % de 4 → 16 KiB mais seulement 4-6 % de 64 → 256 KiB (« flattens out »). — [Percona, Evaluating Database Compression Methods](https://www.percona.com/blog/evaluating-database-compression-methods/)
- **squashfs vs EROFS vs tar (image Debian live ~2 GiB, xz, 2022, avant big pcluster)** : tar.xz 1,7 GiB ; squashfs 128 KiB 2,0 GiB ; squashfs 4 KiB 2,6 GiB ; EROFS 2,7 GiB → ≈ +18 % pour squashfs 128K vs solide, ≈ +50-60 % à 4 KiB. — [sigma-star blog](https://sigma-star.at/blog/2022/07/squashfs-erofs/)
- **xz vs zstd dans squashfs-tools-ng** : zstd décompresse l'image entière en ~6 % du temps de xz pour ~17 % de taille en plus. — [squashfs-tools-ng benchmark.txt](https://git.infraroot.at/squashfs-tools-ng.git/tree/doc/benchmark.txt?h=v1.0.4)
- **Même taille de bloc (1 MiB), même corpus Perl 47,5 GiB** : squashfs 3,41 GB en 15m43s vs DwarFS `-l9 -S20 -B3` 936 MB en 4m26s. — [DwarFS README](https://github.com/mhx/dwarfs)
- **Raspberry Pi OS root (1007 MiB)** : DwarFS défaut 287 MiB, DwarFS -l9 244 MiB, squashfs zstd-22 364 MiB, xz de l'image 297 MiB, tar.xz 247 MiB → DwarFS -l9 ≈ tar.xz solide, squashfs ≈ +47 % vs tar.xz. — [DwarFS README](https://github.com/mhx/dwarfs)
- **Latence d'accès aléatoire DwarFS (perfmon)** : `fuse.op_read` 45 145 échantillons, moyenne 71,2 µs, p99 262,1 µs. — [DwarFS README](https://github.com/mhx/dwarfs)
- **Débit lecture aléatoire (fio)** : squashfuse/squashfuse_ll ≈ 230 MiB/s ; DwarFS ≈ 20,2 GiB/s (données en cache). — [DwarFS README](https://github.com/mhx/dwarfs)
- **DwarFS vs EROFS (Perl 47,5 GiB, 16 processus parallèles)** : EROFS `-zlzma,9 -C1048576` 2,3 GB en 2h39 (MicroLZMA mono-thread), lecture FUSE 8,6 MB/s (arrêté à 5 %) ; EROFS `-zlz4hc,12` 23 GB, 3,1 GB/s ; `+ -Ededupe` 5,8 GB, 3,2 GB/s ; DwarFS 1 MiB lzma 806 MiB, 2,6 GB/s ; DwarFS 1 MiB zstd 3,1 GB/s ; le driver noyau EROFS n'était pas plus rapide que son driver FUSE ; FUSE EROFS lz4hc ≈ 5× moins de CPU que DwarFS. — [DwarFS README (section EROFS)](https://github.com/mhx/dwarfs)
- **ZFS zstd** : l'auteur trouve que 128K et 256K « seem to give good results ». — [blog ZFS zstd](https://evren-yurtesen.blogspot.com/2021/04/zfs-zstd-zstdcompress-feature.html)

### Inferences
- **[estimé]** Perte de ratio typique d'un découpage en blocs vs solide (même codec) : ≈ +40-60 % à 4 KiB, ≈ +15-25 % à 64-128 KiB, ≈ +5-10 % à 1 MiB, quasi nulle ≥ 16 MiB — cohérent avec Percona (4K→64K), sigma-star (4K/128K vs tar.xz) et DwarFS (16 MiB ≈ tar.xz). Au-delà d'un ratio, c'est surtout la fenêtre LZ77 (≤ 8 MiB pour zstd -19, 64 MiB pour xz -9) qui fixe le plafond.
- La latence d'un accès aléatoire = lecture + décompression d'un bloc entier (amplification). Avec zstd (~1-2 GB/s de décompression par cœur) : ~2-4 µs pour 4 KiB, ~50-100 µs pour 128 KiB, ~0,5-1 ms pour 1 MiB, ~10-30 ms pour 16 MiB **[estimé]** ; avec lzma (~50-100 MB/s/cœur) multiplier par ~10-20, ce qui explique le « order of magnitude slower » de DwarFS-lzma et les 8,6 MB/s d'EROFS lzma 1 MiB.
- DwarFS n'échappe pas à ce coût par bloc : il le masque par un cache de blocs décompressés (`cachesize=1g`) et compense le gros bloc par la dédup/segmentation, d'où un ratio ≥ 7z solide malgré 16-64 MiB.
- EROFS est conçu pour la **latence** (lz4, 4 KiB, in-place, zéro copie, boot Android), pas pour le ratio ; squashfs est l'équilibre historique (128 KiB) ; DwarFS vise le ratio avec cache RAM.

### Gaps
- Aucun benchmark primaire 2024-2026 mesurant zstd à exactement 4 KiB / 64 KiB / 1 MiB / solide sur un corpus standard (Silesia) ; voir la **mesure locale** en Q6.
- Pas de chiffre public de latence p50/p99 pour squashfs ou EROFS noyau sur accès 4 KiB aléatoire à froid (seuls des débits agrégés).

## Q2. Comment DwarFS obtient son ratio (segmentation, ordre par similarité, nilsimsa) et benchmarks vs squashfs

### Takeaway
DwarFS combine (1) dédup exacte de fichiers, (2) tri des fichiers par similarité nilsimsa pour rapprocher les octets semblables dans de gros blocs, (3) *segmentation* (dédup de sous-blocs à la volée via hash glissant + Bloom filter, références inter-blocs `-B`) et (4) catégorisation par type (FLAC pour PCM, skip des incompressibles) ; résultat typique sur corpus redondants : 10× plus petit que squashfs, mount en ~10 ms, accès 5-6× plus rapide à froid.

### Cited Findings
- Ordre nilsimsa : 144 675 inodes triés en 91,13 s (corpus Perl) ; segmentation : 454 708 bons matches / 464 247 ; Bloom filter rejette 96,017 % des lookups (4,74 Mrd lookups) ; collisions L1 0,008 %. — [DwarFS README](https://github.com/mhx/dwarfs)
- Dédup fichiers exacte : 28,2 GiB économisés sur 47,65 GiB (1 782 826 fichiers sur 1 927 501) ; la segmentation économise ≈ 15,19 GiB supplémentaires. — [DwarFS README](https://github.com/mhx/dwarfs)
- Corpus Perl (47,49 GiB, 1,9 M fichiers) : tar.gz 12,17 GiB (3,9×) ; tar.zst 0,387 GiB (122,7×) ; 7zip -mx=7 1,219 GiB (39×) ; squashfs 3,245 GiB (14,6×) ; DwarFS zstd 0,352 GiB (134,9×) ; DwarFS lzma 0,310 GiB (153,2×). Mount : squashfs 0,011 s, DwarFS 0,009 s, 7z 3,6 s, tar.gz 2m07. Checksum de 1139 fichiers (2,58 GiB) : squashfs 1,54 s, DwarFS zstd 1,13 s, 7z ~5 h. — [DwarFS README](https://github.com/mhx/dwarfs)
- Construction : mkdwarfs 5m23 → 447 MB vs mksquashfs (zstd 22) 32m54 → 4,75 GB ; accès aléatoire 20 processus à froid : DwarFS 1,149 s vs squashfs 6,733 s (5,85×) ; à chaud DwarFS ≈ +13 % vs répertoire natif (342 vs 302 ms) ; CPU total 5,7 s vs 20,2 s. — [DwarFS README](https://github.com/mhx/dwarfs)
- Artefacts CI DwarFS (465 GiB, 3,6 M fichiers) : tar.gz 142,9 GiB ; 7zip 60,04 GiB ; DwarFS zstd 28,63 GiB (16,25×) ; mount 0,024 s vs 7z 10,5 s. — [DwarFS README](https://github.com/mhx/dwarfs)
- Audio Sonniss (3,07 GiB WAV) : DwarFS `--categorize` (FLAC) 1,664 GiB (1,85×) en 3,98 s vs 7zip 2,255 GiB (1,36×). Vidéo 39 GiB (fichiers frères, pistes audio différentes) : DwarFS ≈ 14 GB, squashfs 39 GB (segmentation inter-fichiers). — [DwarFS README](https://github.com/mhx/dwarfs)
- Sources Wine (64,85 GiB, ~3 M fichiers) : `mkdwarfs -l9` 93,2 MiB en 2m17 ; brotli + nilsimsa 73,52 MiB (~1000×) ; tar.xz ≈ 7 GiB, squashfs ≈ 1,6 GiB, tar.xz trié par similarité 399 MiB ; zpaqfranz 589 MiB en 18m58. — [DwarFS README](https://github.com/mhx/dwarfs)
- Comparatifs solides : zpaq -m5 490 MB en ~47 min, extraction 46m38 (~700× plus lent que DwarFS) ; lrzip 500 MB en 57m32 ; xz -9 sur tar 4,3 GiB en 34m55 ; wimlib solid LZMS 1,017 GB en 15m23 (DwarFS 3× plus rapide à construire, < moitié de la taille). — [DwarFS README](https://github.com/mhx/dwarfs)
- fuse-archive (zip/7z/tar.* montés via FUSE) : mount boost 7z 0,321 s, tar.bz2 15,7 s, DwarFS 0,013 s ; accès aléatoire : DwarFS 598 MB/s, zip 1,8 MB/s, tar.gz 1,2 MB/s, 7z/tar.bz2 0,2-0,3 MB/s (« grinds to a halt »). — [DwarFS README](https://github.com/mhx/dwarfs)
- Overhead FUSE : init du driver 61 ms → 2,9 ms avec métadonnées non compressées ; options de benchmark `-o cachesize=1g -o workers=4`. — [DwarFS README](https://github.com/mhx/dwarfs)

### Inferences
- Le gain principal de DwarFS vient de la **redondance inter-fichiers** (dédup + segmentation + tri), pas du codec : sur un corpus sans redondance (audio, vidéo unique), le ratio retombe à celui du codec/catégoriseur.
- Les benchmarks sont ceux de l'auteur (mhx) ; ils sont détaillés et reproductibles mais non indépendants. Le point faible connu : RAM (cache de blocs décompressés de 16-64 MiB) et CPU de construction ; l'accès à froid d'un bloc 64 MiB lzma coûte des dizaines de ms **[estimé]**.

### Gaps
- Pas de benchmark indépendant (hors README) DwarFS vs squashfs vs EROFS trouvé en 2024-2026.
- Pas de latence p99 à froid (cache vide) publiée pour DwarFS par taille de bloc.

## Q3. Plafond de débit WinFsp vs driver noyau, et ce qu'offre ProjFS

### Takeaway
WinFsp (user-mode via un driver FSD noyau signé) atteint des performances comparables à NTFS, parfois supérieures en I/O cache (MEMFS/NTPTFS), mais Microsoft ne publie pas de MB/s et il n'existe pas de comparaison WinFsp/Dokany chiffrée récente ; ProjFS est le mécanisme Microsoft natif (Windows 10 1809+) de « projection » paresseuse avec hydratation de fichiers placeholder, conçu pour des backings rapides et sans notion de progression/offline.

### Cited Findings
- WinFsp Performance Testing : résultats normalisés à NTFS = 1 (pas de MB/s absolus) ; MEMFS meilleur sur file_create/overwrite/list/delete ; NTFS meilleur sur iter.file_open et file_list_single ; lecture/écriture cachée et non cachée 1 page : MEMFS et NTPTFS « faster than NTFS by a considerable margin » ; mmap identique à NTFS ; explication : implémentation « fast I/O » de WinFsp plus performante que celle de NTFS. Setup : Dell XPS 13 9300, i7-1065G7, 32 GB, NVMe 2 TB, Windows 11 21H2 build 22000.258, WinFsp v1.11B3 ; antivirus désactivé sur le FS inférieur pour NTPTFS. — [WinFsp Performance Testing](https://winfsp.dev/doc/WinFsp-Performance-Testing/)
- Pas de chiffres WinFsp vs Dokany : fil 2018 où l'auteur dit ne plus avoir relancé les benchmarks ; « WinFsp is similar to Dokan in functionality although rather different under the hood ». — [Google Groups winfsp](https://groups.google.com/g/winfsp/c/gzul_OmWH_E)
- Cas réel SecureFS+WinFsp : ~300 MB/s sur disque interne, 10 MB/s lecture / 3 MB/s écriture sur SSD USB-C (goulot USB, pas WinFsp). — [Google Groups winfsp](https://groups.google.com/g/winfsp/c/nMZZiiPuxDo)
- DwarFS sur Windows (WinFsp) : expérimental ; `winfsp-x64.dll` chargée paresseusement ; case-sensitive par défaut ; symlinks ne peuvent pointer hors du volume (limite WinFsp) ; hardlink count non propagé ; création de liens requiert admin. — [DwarFS README](https://github.com/mhx/dwarfs)
- ProjFS : un « provider » user-mode projette une hiérarchie depuis un backing store (ex. registre, VFS for Git) ; « designed for use with high-speed backing data stores » ; ne fournit ni progression de rappel, ni état online/offline → utiliser Cloud Files API pour backings lents ; doc datée 2018-09-14, maj 2025-04-15. — [Microsoft Learn ProjFS](https://learn.microsoft.com/en-us/windows/win32/projfs/projected-file-system)
- Signature de drivers : Microsoft recommande HLK (signe pour toutes versions) ; l'attestation signing « doesn't require HLK testing » mais est « for testing purposes only » / non « Windows Certified », limitée à Windows 10 desktop+ ; EV cert requis pour soumettre (source vendeur). — [Microsoft Learn driver signing offerings](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/driver-signing-offerings) ; [OSR minifilter thread](https://community.osr.com/t/re-2-do-i-need-hck-logo-certification-for-minifilter-driver/54055)
- Alternative native sans driver tiers : `compact /EXE` (WOF) avec XPRESS4K (défaut, le plus rapide), XPRESS8K, XPRESS16K, LZX (le plus compact), « optimized for executable files that are read frequently, but not modified ». — [Microsoft Learn compact](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/compact)
- CompactGUI (sur WOF) : fichier 46,6 MB → LZX 15,7 MB, XPRESS16K 20,1, XPRESS8K 21,1, XPRESS4K 23,0, LZNT1 26,2 ; Valorant 57,4 GB → 31 GB (X16, −46 %) avec « marginal increase in initial load time » ; incompatible avec les jeux DirectStorage (décompression CPU obligatoire). — [aryan-gupta blog 2025](https://aryan-gupta.is-a.dev/blog/2025/compact-gui-deep-dive) ; [Neowin](https://www.neowin.net/news/free-app-based-on-official-microsoft-tech-saves-huge-windows-space-with-no-performance-loss/)

### Inferences
- **[estimé]** Le vrai plafond d'un FS user-mode Windows n'est pas le débit séquentiel (WinFsp sature un NVMe grâce au cache manager) mais le coût par opération métadonnée (open/lookup : 2 transitions user↔kernel, ~10-30 µs) — même diagnostic que FUSE Linux (cf. CoFS Q5). Un jeu qui ouvre des milliers de petits fichiers souffrira davantage qu'un jeu à gros `.pak`.
- ProjFS est un outil de **matérialisation paresseuse** (hydratation puis fichier NTFS réel), pas de lecture compressée en place : après hydratation, l'espace disque est consommé en clair. Pour « jouer depuis l'archive sans extraire », WOF/compact est le seul mécanisme Microsoft natif à décompression transparente par bloc.
- Trois voies sur Windows : (a) driver tiers WinFsp/Dokany (signé par leurs auteurs, perf OK, risque anti-cheat, Q4) ; (b) ProjFS (natif, zéro driver tiers, mais hydratation = extraction différée) ; (c) WOF compact (natif, ratio modeste 1,3-2×, bloc 4-16 KiB ou LZX 32 KiB **[estimé]**).

### Gaps
- Aucun MB/s ou IOPS absolu publié par WinFsp ; aucune comparaison WinFsp/Dokany/ProjFS chiffrée récente.
- Coût HLK/attestation 2024-2026 non trouvé dans une source fiable.

## Q4. Qui a livré « jouer directement depuis une archive compressée » et comment ? Bloqueurs (anti-cheat, antivirus, macOS kext)

### Takeaway
Tous les grands distributeurs lisent déjà en place depuis des conteneurs chunkés/compressés — Blizzard CASC (BLTE zlib/LZ4 par blocs de 16-256 KiB, lecture par offset dans `data.NNN`), consoles (PS5 Kraken HW 5,5 GB/s brut → 8-9 GB/s), DirectStorage GDeflate (tuiles 64 KiB décompressées GPU) — mais côté distribution Steam/Riot, les chunks (≈1 MiB LZMA, FastCDC) servent au téléchargement puis sont **extraits** sur disque. Les anti-cheat bloquent VM/drivers non approuvés et UNC paths ; aucune preuve primaire d'un blocage spécifique de WinFsp/Dokan n'a été trouvée.

### Cited Findings
- **Steam** : SteamPipe « initially splits each file into roughly one megabyte (MB) chunks. Each chunk is then compressed and encrypted ». — [Steamworks docs](https://partner.steamgames.com/doc/sdk/uploading) ; SteamDB (reverse) : LZMA + AES-256, manifest avec ID, Adler-32, offset, tailles ; bibliothèque steamroom : décode « Valve LZMA, Valve zstd, and zip » → plusieurs codecs en pratique (non confirmé par Valve). — [SteamDB](https://steamdb.info/blog/steam-download-system) ; [steamroom](https://docs.rs/crate/steamroom/0.3.0)
- **Riot** : nouveau patcher à content-defined chunking (FastCDC, > 1 GB/s/cœur), les chunks manquants seulement sont téléchargés ; « patch sizes … reasonably close to the old one ». — [Riot Tech Blog](https://technology.riotgames.com/node/99)
- **Blizzard CASC (local)** : archives `data.NNN` sous `Data\data\`, 16 fichiers `.idx` ; entrée 18 octets = clé 9 o + offset 5 o (archive n° + offset 30 bits) + taille 4 o → résolution directe clé → (archive, offset, taille) ; max install 256 GiB. — [wowdev.wiki CASC](https://wowdev.wiki/CASC)
- **BLTE** : magic + headerSize ; table de chunks (rawSize, logicalSize, MD5 16 o) ; modes par bloc N (brut), Z (zlib), 4 (LZ4 sub-blocs 2^5..2^16), E (Salsa20/ARC4), F (récursif, déprécié TACT 3.9.6 02/2025) ; ESpec `b:{256K*=z}` = blocs 256 KiB zlib ; exemples 16K et 256K. — [wowdev.wiki BLTE](https://wowdev.wiki/BLTE)
- **DirectStorage 1.1 / GDeflate** : flux découpé en tuiles 64 KiB compressées indépendamment, un thread (CPU) ou un thread-group (GPU) par tuile ; démo : 0,8 s (GPU GDeflate) vs 2,36 s (CPU zlib). — [DirectX blog](https://devblogs.microsoft.com/directx/directstorage-1-1-coming-soon/) ; Intel : jusqu'à 2,7× (Arc A770 vs i9-12900K), garder les requêtes compressées > 64 KB pour limiter l'overfetch. — [Intel](https://www.intel.com/content/www/us/en/developer/articles/news/directstorage-on-intel-gpus.html) ; NVIDIA : streaming non compressé limité ~3 GB/s par PCIe Gen3, staging buffer 32 → ≥ 128 MB. — [NVIDIA](https://developer.nvidia.com/blog/accelerating-load-times-for-directx-games-and-apps-with-gdeflate-for-directstorage)
- **PS5** : SSD 5,5 GB/s brut, 8-9 GB/s décompressé typique (≈1,5:1), décompresseur Kraken HW ≈ 9 cœurs Zen 2 ; le « 22 GB/s » est un pic dérivé, faiblement sourcé. — [Tom's Guide/RAD](https://www.tomsguide.com/news/this-ps5-upgrade-will-give-gamers-the-fastest-data-loading-ever) ; [Notebookcheck](https://www.notebookcheck.net/The-PlayStation-5-SSD-comes-in-an-odd-capacity-and-can-fill-16-GB-of-RAM-in-just-two-seconds.458481.0.html)
- **Anti-cheat — VM** : EAC et BattlEye bloquent les VM ; rapport 01/2026 : une maj EAC rend le blocage VM par défaut ; EAC flagge une machine si les drivers VirtIO sont installés même sans device. — [betahub](https://app.betahub.io/projects/pr-7682550116/issues/846) ; [Proxmox](https://forum.proxmox.com/posts/854128/)
- **Anti-cheat — chemins/drivers** : launchers Ubisoft et autres refusent les chemins UNC ; BattlEye tourne en SYSTEM et ne voit pas les lecteurs réseau mappés par l'utilisateur (contournement proposé : VHD). — [Unraid](https://forums.unraid.net/topic/127639-easy-anti-cheat-launch-error-cannot-run-under-virtual-machine/page/2/) ; [Steam ARK thread](https://steamcommunity.com/app/346110/discussions/0/1744515852428171500) ; le minifilter EAC `EasyAntiCheat_EOSSys` « does not support IO bypass » (BypassIO/DirectStorage). — [ElevenForum](https://www.elevenforum.com/t/easyanticheat-driver-eossys-sys-loads-often-does-not-support-bypassio-may-conflict-with-directstorage.38005) ; EAC « Untrusted system file » sur DLL/drivers considérés modifiés ou non signés. — [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/5840101/easy-anti-cheat-untrusted-system-file-globalrootde)
- **Vanguard** : driver `vgk.sys` chargé au boot, « aggressive stance on driver allowlisting », scanne et bloque les drivers vulnérables. — [Riot](https://www.riotgames.com/en/news/a-message-about-vanguard-from-our-security-privacy-teams) ; [s4dbrd](https://s4dbrd.github.io/posts/how-kernel-anti-cheats-work)
- **macOS** : macOS ne charge plus par défaut les kexts utilisant des KPI dépréciées depuis Big Sur. — [Apple kernel-extensions](https://developer.apple.com/support/kernel-extensions) ; FSKit : « essentially undocumented » en juin 2024, framework public à partir de macOS 15.4 (une source dit privé depuis 14.0). — [Eclectic Light](https://eclecticlight.co/2024/06/26/how-file-systems-can-change-in-sequoia-with-fskit/) ; macFUSE 5 : backend FSKit (`-obackend=fskit`), kext reste défaut, « FSKit API … still not on par with the VFS kernel API » ; macFUSE 5.0.7 supporte macOS 12→26 ; plus besoin de recovery mode avec FSKit. — [ifun.de](https://www.ifun.de/macfuse-5-0-neue-version-mit-unterstuetzung-fuer-apples-fskit-253976/) ; [macFUSE 5.0.7 release](https://newreleases.io/project/github/macfuse/macfuse/release/macfuse-5.0.7) ; alternative sans kext : FUSE-T (NFS local). — [fuse-t](https://awesome.ecosyste.ms/projects/github.com%2Fmacos-fuse-t%2Ffuse-t)
- **Nintendo / Xbox** : non couverts par des sources primaires trouvées (voir Gaps).

### Inferences
- Le modèle qui « marche en prod » dans le jeu est **l'archive applicative propriétaire lue en place par le moteur** (CASC, .pak UE, DirectStorage) — pas un FS virtuel : zéro driver, zéro anti-cheat, décompression par blocs 64-256 KiB. Un FS virtuel générique (WinFsp/Dokan) ajoute un driver noyau tiers que Vanguard/EAC traitent par allowlist → risque réel mais **non documenté publiquement** ; un VHDX/Dev Drive natif ou WOF `compact` est la voie « zéro driver tiers ».
- Antivirus : la seule donnée mesurée est méthodologique (WinFsp désactive l'AV sur le FS inférieur pour ses benchmarks), signe que le scan en temps réel double les coûts sur un passthrough **[inférence]**.
- Sur macOS, la trajectoire est claire (kext → FSKit user-space), mais la parité API n'est pas atteinte en 2025 ; viser FSKit + fallback FUSE-T.

### Gaps
- Aucun ticket primaire (EAC/BattlEye/Vanguard, Dokany, WinFsp) confirmant un blocage de FS virtuel par anti-cheat ; preuve indirecte seulement (politique de drivers, UNC, VM).
- Pas de source primaire sur le format NCA (Switch) ni XVC/MSIXVC (Xbox) ; Blizzard TACT CDN (archives 256 MB) non vérifié.
- Pas de mesure publique d'impact antivirus sur FUSE/WinFsp.

## Q5. Papiers académiques (FAST, ATC, SOSP, EuroSys, NSDI 2016-2026) sur compression à accès aléatoire et matérialisation paresseuse

### Takeaway
La littérature est dominée par le *lazy pulling* d'images conteneur : Slacker (FAST'16, ~6,4 % des données lues au démarrage), EROFS (ATC'19), DADI/Overlaybd (ATC'20, bloc), eStargz/SOCI (index sur gzip), FlacIO (FAST'25) et CoFS (FAST'26, KylinSoft) qui montre que le chemin EROFS→fscache→daemon userspace est **plus lent que Nydus-FUSE** au premier accès et que FUSE coûte surtout en lookup de métadonnées (CoFS +73-86 % vs fuse-loopback).

### Cited Findings
- **Slacker (FAST'16)** : image médiane 117 MB compressée, ~20 MB lus au démarrage ; origine du lazy loading Docker. — [slacker-fast16.pdf](https://research.cs.wisc.edu/wind/Publications/slacker-fast16.pdf)
- **EROFS (ATC'19)** : fixed-sized output compression ; −22,9 % boot time, ≈ −50 % stockage. — [USENIX ATC'19](https://www.usenix.org/conference/atc19/presentation/gao)
- **DADI (ATC'20, Alibaba, pp. 727-740)** : image = block device virtuel en couches, fetch à la demande, P2P ; successeur open source Overlaybd (containerd Accelerated Container Image). — [USENIX ATC'20](https://www.usenix.org/conference/atc20/presentation/li-huiba) ; [overlaybd](https://github.com/alibaba/overlaybd)
- **eStargz** : tar.gz compatible, chaque fichier (ou chunk de gros fichier) extractible et vérifiable séparément ; stargz-snapshotter préfetch les fichiers probables. — [stargz-snapshotter docs](https://github.com/containerd/stargz-snapshotter/blob/v0.2.0/docs/stargz-estargz.md)
- **SOCI (AWS)** : index externe sur couches gzip OCI standard (frontières pour accès aléatoire) ; pull 20 s → 2,5 s, indépendant de la taille d'image. — [SOCI paper](https://cdn.amazon.science/84/96/51ea2e0d40fca8cfaee9328278b9/soci-paper.pdf) ; arXiv 2026 « Seekable OCI: Lazy-Loading Container Images via Range-Request Indexing ». — [arXiv 2607.06868](https://arxiv.org/pdf/2607.06868)
- **Nydus / EROFS over fscache** : backend noyau depuis Linux 5.19, cache hits servis sans passage userspace. — [d7y.io evolution of Nydus](https://d7y.io/blog/2022/06/06/evolution-of-nydus/)
- **FlacIO (FAST'25)** : « runtime image » de l'état mémoire du rootfs ; cold-start réduit jusqu'à 23× vs full-image et 4,6× vs lazy loading. — [USENIX FAST'25](https://www.usenix.org/conference/fast25/presentation/liu-yubo)
- **CoFS (FAST'26, KylinSoft)** : « pulling image accounts for 76 % of container start time, but only 6.4 % of that data is read » ; Nydus-fuse et eStargz (FUSE) → latence de lookup métadonnées + context switch + copies ; Nydus-erofs : au premier accès le daemon télécharge, écrit synchrone sur disque puis notifie fscache → « long call chain … makes its performance even lower than Nydus-fuse », éviction fscache imprévisible ; CoFS (FUSE étendu + MPHF, lookup en ≤ 1 I/O) : +73-86 % de lookup vs fuse-loopback ; évalué avec Nydus 2.2.5, eStargz 0.15.1, bucketbench, cache vidé, 10 runs. — [fast26-wang-li.pdf](https://www.usenix.org/system/files/fast26-wang-li.pdf)
- Analyse Microsoft (cold start LLM) : le pull domine pour les gros modèles mais le gain réel de démarrage est faible quand l'app démarre déjà vite. — [Microsoft Tech Community](https://techcommunity.microsoft.com/blog/linuxandopensourceblog/dissecting-llm-container-cold-start-where-the-time-actually-goes/4508831)
- **composefs** (Red Hat/containers) : image EROFS loopback ne contenant que les métadonnées + overlayfs `trusted.overlay.redirect` vers un store content-addressed, fs-verity optionnel ; partage de page cache entre images ; « does not store any persistent data itself » ; pas de chiffres publiés. — [containers/composefs](https://github.com/containers/composefs)
- **casync** (Poettering) : chunking content-defined buzhash, 64 K moyen (`--chunk-size=`), chunks xz nommés par SHA256 dans `.castr`, index `.caibx/.caidx`, format `catar` à accès aléatoire, `casync mount` via FUSE ; critique d'OSTree : un GET par fichier, deltas précalculés. — [0pointer casync](https://0pointer.net/blog/casync-a-tool-for-distributing-file-system-images.html)
- **OSTree** : store content-addressed SHA256 (commit/dirtree/dirmeta/content/xattrs), timestamps forcés à 0, mode `archive` = objets gzip `.filez` pour HTTP, modes `bare*` = checkouts par hardlinks, static deltas listés dans `summary`. — [ostree repo docs](https://ostreedev.github.io/ostree/repo/)
- **Nix** : nix-casync stocke les NAR chunkés via casync (dédup, chunk moyen configurable au-delà des frontières de fichiers), compression zstd recommandée à l'upload ; un test : 2 487 089 (3,22×) brut vs 1 223 264 (6,55×) zstd ; Attic chunke les NAR ≥ 128 KiB (`nar-size-threshold = 131072`). — [nix-casync Discourse](https://discourse.nixos.org/t/nix-casync-a-more-efficient-way-to-store-and-substitute-nix-store-paths/16539) ; [Attic](https://github.com/zhaofengli/attic)

### Inferences
- Consensus 2016-2026 : seule une petite fraction (≈ 6-20 %) d'un artefact est lue au lancement → la matérialisation paresseuse (ProjFS, eStargz, Nydus, casync mount) est le levier n° 1 sur le *time-to-first-use*, la compression en place le levier n° 1 sur l'espace. Les deux se combinent (Nydus = chunks compressés + index + lazy).
- Le coût FUSE est **par opération métadonnée**, pas par octet : les systèmes récents (CoFS, DwarFS métadonnées non compressées, EROFS in-place) optimisent le lookup. La leçon vaut pour WinFsp.
- fscache/EROFS on-demand n'est pas automatiquement meilleur que FUSE : la chaîne d'appel au cache miss domine (CoFS). Préférer un cache local pré-rempli (préfetch par trace) + chemin noyau pour les hits.

### Gaps
- Starlight (NSDI'22) non retrouvé ; pas de papier SOSP/EuroSys spécifique au compromis ratio/taille de bloc.
- Tableaux chiffrés complets de CoFS (startup absolu) non extraits (texte PDF partiel).
- Pas de papier sur les archives de jeux (CASC, DirectStorage) dans ces venues.

## Q6. Quantification solide vs accès aléatoire (zstd à 4 KiB / 64 KiB / 1 MiB / solide)

### Takeaway
Les sources publiques donnent la *forme* de la courbe (gain fort 4→64 KiB, plateau au-delà de 256 KiB-1 MiB) ; une mesure locale sur deux corpus (code source, binaires x64) est fournie ci-dessous pour chiffrer zstd -3 / -19 et xz -6 par taille de bloc, avec la latence de décodage d'un bloc.

### Cited Findings
- Percona (zstd 0.4.1 niveau 1, données BDD 2016) : 4 KiB ≈ 3,95 ; 64 KiB ≈ 6,41 ; 256 KiB ≈ 7,17 ; +≥ 20 % de 4→16 KiB, +4-6 % de 64→256 KiB. — [Percona](https://www.percona.com/blog/evaluating-database-compression-methods/)
- Débit zstd vs taille de bloc (même test) : 178 MB/s à 4 KiB → 396 MB/s à 256 KiB (les petits blocs coûtent aussi en vitesse). — [Percona](https://www.percona.com/blog/evaluating-database-compression-methods/)
- Image Debian live xz : tar.xz solide 1,7 GiB ; squashfs 128 KiB 2,0 GiB (+18 %) ; squashfs 4 KiB 2,6 GiB (+53 %). — [sigma-star](https://sigma-star.at/blog/2022/07/squashfs-erofs/)
- Raspberry Pi OS : tar.xz 247 MiB vs squashfs zstd-22 128 KiB 364 MiB (+47 %) vs DwarFS 16/64 MiB 287/244 MiB. — [DwarFS README](https://github.com/mhx/dwarfs)
- Amplification de lecture : lire 128 KiB aléatoires dans des blocs de 1 MiB impose de lire/décoder 1 MiB (ZFS, même logique pour squashfs). — [TrueNAS forum](https://forums.truenas.com/t/unexpectedly-large-downside-small-upside-to-bs-1m/10964)
- Machine-generated data (arXiv 2023) : « with the increase of block size, the compression ratio of Zstd improves to a considerable level, but the lookup speed deteriorates ». — [arXiv 2311.13947](https://arxiv.org/pdf/2311.13947)

### Mesure locale **[mesure locale, 2026-11-16]**
Protocole : Python 3.14 (`compression.zstd`, `lzma`), Windows 11 ; corpus A = 48 MiB de sources texte (PHP/JS/TS/Dart/JSON/MD) du projet FleetMaster ; corpus B = 48 MiB de binaires x64 (DLL/PYD/EXE de `C:\Python314`). Chaque corpus découpé en blocs indépendants de 4 K / 16 K / 64 K / 256 K / 1 M / 16 M / solide ; ratio = taille brute / somme des blocs compressés ; `ra` = temps de décompression d'un seul bloc (moyenne 20 itérations zstd, 5 xz), i.e. coût incompressible d'un accès aléatoire hors I/O.
Résultats (ratio = brut/compressé ; ra = latence de décompression d'un bloc, µs ; machine de bureau Windows 11, mono-thread, sans I/O) :

| Corpus | Codec | 4 KiB | 16 KiB | 64 KiB | 256 KiB | 1 MiB | 16 MiB | Solide |
|---|---|---|---|---|---|---|---|---|
| Code source 48 MiB | zstd -3 | 2,55 (68 µs) | 3,09 (31 µs) | 3,48 (91 µs) | 3,74 (0,63 ms) | 3,97 (3,5 ms) | 4,10 (44 ms) | 4,11 (126 ms) |
| Code source 48 MiB | zstd -19 | 2,67 (12 µs) | 3,35 (37 µs) | 3,93 (99 µs) | 4,43 (0,40 ms) | 4,86 (2,4 ms) | 5,65 (29 ms) | 5,78 (122 ms) |
| Code source 48 MiB | xz -6 | 2,56 (0,21 ms) | 3,34 (0,33 ms) | 3,99 (1,4 ms) | 4,51 (5,2 ms) | 4,96 (13 ms) | 5,74 (255 ms) | 5,87 (621 ms) |
| Binaires x64 48 MiB | zstd -3 | 1,84 (16 µs) | 2,03 (46 µs) | 2,10 (0,13 ms) | 2,24 (0,43 ms) | 2,31 (2,3 ms) | 2,36 (41 ms) | 2,37 (123 ms) |
| Binaires x64 48 MiB | zstd -19 | 1,97 (14 µs) | 2,22 (51 µs) | 2,38 (0,13 ms) | 2,54 (0,63 ms) | 2,68 (2,2 ms) | 2,86 (52 ms) | 2,89 (172 ms) |
| Binaires x64 48 MiB | xz -6 | 1,96 (0,12 ms) | 2,29 (0,39 ms) | 2,52 (1,2 ms) | 2,70 (4,2 ms) | 2,85 (18 ms) | 3,05 (370 ms) | 3,07 (1,1 s) |

Lecture : pénalité de taille vs solide (même codec) — code source zstd -19 : 4 KiB ×2,16, 64 KiB ×1,47, 256 KiB ×1,30, 1 MiB ×1,19, 16 MiB ×1,02 ; binaires zstd -19 : 4 KiB ×1,47, 64 KiB ×1,21, 1 MiB ×1,08, 16 MiB ×1,01. zstd -3 plafonne plus tôt (fenêtre courte) : 64 KiB déjà à 85 % du solide. xz n'apporte que +2-8 % de ratio sur zstd -19 pour une latence de décodage ×10-15 par bloc. Les latences sous 100 µs (4-16 KiB) sont dominées par l'overhead d'appel Python, à lire comme « < 0,1 ms ». Mesure mono-machine, indicative. **Réplication** : une seconde exécution indépendante du même protocole (autre échantillon de 48 MiB des mêmes corpus, 2026-11-16) reproduit les ratios à ±5 % — code zstd -19 : 4 KiB 2,80 (×2,21 vs solide 6,19), 64 KiB 4,20 (×1,47), 1 MiB 5,25 (×1,18), 16 MiB 6,08 (×1,02) ; binaires zstd -19 : 4 KiB 1,96 (×1,46), 64 KiB 2,37 (×1,21), 1 MiB 2,65 (×1,08) ; xz -6 code : 4 KiB ×2,32, 64 KiB ×1,47, 1 MiB ×1,18 ; latences de décodage 1 MiB : zstd 1,4-2,8 ms, xz 12-15 ms ; 16 MiB : zstd 36-53 ms, xz 180-405 ms. Débit zstd observé ≈ 350-450 MB/s dans ce Python mono-thread, soit ~3× sous le zstd natif : latences absolues pessimistes, rapports entre tailles fiables.

### Inferences
- **[synthèse sources + mesure locale]** Pénalité de ratio vs solide avec zstd/xz : ≈ ×1,5-2,2 à 4 KiB, ≈ ×1,2-1,5 à 64 KiB, ≈ ×1,1-1,2 à 1 MiB, ≈ ×1,0 à 16 MiB (la pénalité croît avec la redondance du corpus : texte/code > binaires) (sauf corpus très redondant où la fenêtre longue de xz/DwarFS continue de gagner). Le « sweet spot » 64-256 KiB expliqué par la fenêtre interne des codecs rapides et le coût de décodage < 0,2 ms.
- Le choix se résume à : latence cible par accès aléatoire (4 KiB ~µs, 1 MiB ~ms, 16 MiB ~10 ms + cache obligatoire) ↔ ratio ; EROFS (4 K-64 K), squashfs (128 K), CASC/DirectStorage (64-256 K), Steam (1 M), DwarFS (16-64 M + cache 1 GiB) occupent chacun un point de cette courbe.

### Gaps
- Pas de benchmark public standardisé (Silesia) 2024-2026 de zstd par taille de bloc ; la mesure locale ci-dessus est mono-machine, deux corpus, à considérer comme indicative.
