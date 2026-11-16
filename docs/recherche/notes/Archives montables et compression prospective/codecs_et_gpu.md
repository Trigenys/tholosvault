# Codecs haut débit et (dé)compression accélérée matériellement — état de l'art 2024-2026

Notes de recherche (rédigées le 2026-11-16). Toutes les valeurs sont datées et contextualisées (CPU/GPU, version, corpus). Les estimations ou calculs dérivés sont marqués **[estimation]** ou **[calcul]**.

## KQ1 — Table ratio / vitesse de décompression : zstd (niveaux), xz, LZMA2 ultra, Brotli 11, Oodle Kraken/Leviathan, sur Silesia

### Takeaway
Sur Silesia (lzbench 2.4, 1 thread, EPYC 9555P), zstd décompresse entre ~1,1 et 1,35 GB/s quel que soit le niveau (ratio 2,9:1 → 4,05:1), Brotli ~0,38-0,44 GB/s (4,2:1 à -11), xz/LZMA ~0,07-0,12 GB/s (4,35:1), LZ4 ~3,8 GB/s (2,1:1). Oodle Kraken/Leviathan (mesures RAD 2018, i7-3770) atteignent 4,09-4,35:1 à 0,8-0,9 GB/s, soit le ratio de LZMA à ~10× sa vitesse de décodage ; aucune mesure indépendante récente d'Oodle n'a été trouvée.

### Cited Findings

**Table Silesia — lzbench 2.4, `-eALL -t8,8 -o1c4`, silesia.tar (211 947 520 o), AMD EPYC 9555P 3,2 GHz, 1 thread, turbo off, gcc 15.2.0, Ubuntu 26.04 ; date du run non indiquée sur la page (versions de codecs : zstd 1.5.7, xz 5.8.4, lzma SDK 26.03, brotli 1.2.0, lz4 1.10.0 → run 2026)** — [lzbench README](https://github.com/inikep/lzbench). Colonne « ratio x:1 » = 211 947 520 / taille **[calcul]**.

| Codec / niveau | Compr. MB/s | Décompr. MB/s | Taille (o) | % orig. | Ratio x:1 [calcul] |
|---|---|---|---|---|---|
| memcpy | 24 075 | 24 002 | 211 947 520 | 100,00 | 1,00 |
| lz4 1.10.0 --fast -17 | 924 | 3 977 | 131 732 802 | 62,15 | 1,61 |
| lz4 1.10.0 (défaut) | 550 | 3 832 | 100 880 800 | 47,60 | 2,10 |
| lz4hc -1 | 259 | 3 382 | 89 135 429 | 42,06 | 2,38 |
| lz4hc -12 | 10,4 | 3 813 | 77 262 620 | 36,45 | 2,74 |
| zstd --fast --5 | 563 | 1 949 | 103 023 359 | 48,61 | 2,06 |
| zstd --fast --1 | 453 | 1 707 | 86 916 294 | 41,01 | 2,44 |
| zstd -1 | 411 | 1 351 | 73 193 704 | 34,53 | 2,90 |
| zstd -2 | 352 | 1 257 | 69 309 797 | 32,70 | 3,06 |
| zstd -5 | 126 | 1 172 | 62 740 852 | 29,60 | 3,38 |
| zstd -8 | 61,8 | 1 301 | 59 699 824 | 28,17 | 3,55 |
| zstd -11 | 36,1 | 1 320 | 57 963 230 | 27,35 | 3,66 |
| zstd -15 | 9,39 | 1 345 | 56 867 494 | 26,83 | 3,73 |
| zstd -18 | 4,13 | 1 175 | 53 288 948 | 25,14 | 3,98 |
| zstd -22 (--ultra) | 1,99 | 1 087 | 52 284 290 | 24,67 | 4,05 |
| zlib -6 | 26,8 | 314 | 68 228 431 | 32,19 | 3,11 |
| zlib -9 | 11,3 | 318 | 67 644 548 | 31,92 | 3,13 |
| brotli -0 | 341 | 322 | 78 433 298 | 37,01 | 2,70 |
| brotli -2 | 139 | 378 | 68 069 489 | 32,12 | 3,11 |
| brotli -5 | 53,3 | 419 | 59 555 449 | 28,10 | 3,56 |
| brotli -8 | 12,6 | 443 | 57 148 304 | 26,96 | 3,71 |
| brotli -11 | 0,56 | 383 | 50 407 795 | 23,78 | 4,20 |
| xz -1 | 19,2 | 105 | 58 805 676 | 27,75 | 3,60 |
| xz -3 | 9,80 | 116 | 55 972 224 | 26,41 | 3,79 |
| xz -5 | 3,84 | 121 | 50 228 752 | 23,70 | 4,22 |
| xz -7 | 3,01 | 124 | 49 063 436 | 23,15 | 4,32 |
| xz -9 | 2,75 | 122 | 48 766 532 | 23,01 | 4,35 |
| lzma 26.03 -0 | 29,5 | 62,3 | 60 520 844 | 28,55 | 3,50 |
| lzma (7-Zip SDK 26.03) -6 | 3,13 | 78,4 | 49 551 059 | 23,38 | 4,28 |
| lzma 26.03 -9 | 2,55 | 78,2 | 48 682 482 | 22,97 | 4,35 |
| lzham -d26 -1 | 3,16 | 273 | 54 740 589 | 25,83 | 3,87 |
| lizard -29 | 2,08 | 1 470 | 68 694 227 | 32,41 | 3,09 |

- Autres codecs présents dans le même run (non extraits ici) : bzip3, bsc, kanzi, libdeflate, zlib-ng, snappy, lzfse, lzvn, fastlzma2, zpaq, ppmd8, aceapex, etc. — [lzbench README](https://github.com/inikep/lzbench)
- **Oodle sur Silesia (cbloom, RAD, Oodle 2.6.0, 2018, Core i7-3770, Windows x64, 1 thread)** : Leviathan défaut 4,349:1 à 801 MB/s décodage ; Leviathan « max compress » 4,362:1 à 717 MB/s — [cbloom, Oodle 2.6.0 Hydra and space-speed](https://cbloomrants.blogspot.com/2018/02/oodle-260-hydra-and-space-speed.html)
- Kraken -z6 sur Silesia total : 211 938 580 → 51 857 427 o = 4,087:1 ; décodage 913 MB/s mono-thread, 1 343 MB/s en « thread-phased » 2 threads — [cbloom, thread-phased decoding](https://cbloomrants.blogspot.com/2016/05/oodle-kraken-thread-phased-decoding.html) ; ratio Kraken passé de 4,05:1 (avril 2016) à 4,24:1 (Oodle 2.6.0) — [cbloom 2018/02](https://cbloomrants.blogspot.com/2018/02)
- Mermaid (Oodle 2.3.0, -z6) : 3,571:1 à 2 022 MB/s ; Selkie : 1,3× plus rapide à décoder que LZ4 avec ~12 % de compression en plus — [cbloom, introducing Mermaid and Selkie](https://cbloomrants.blogspot.com/2016/07/introducing-oodle-mermaid-and-selkie.html)
- Comparatif RAD multi-corpus (2018-03-06, i7-3770, décodage seul) sur Silesia/mozilla : lzma 16.04 -9 3,832:1 @ 63,8 MB/s ; zstd 1.3.3 -22 3,365:1 @ 503 MB/s ; brotli -11 3,601:1 @ 247 MB/s ; lz4hc -12 2,327:1 @ 2 510 MB/s ; Leviathan 3,831:1 @ 691 MB/s ; Kraken 3,740:1 @ 985 MB/s ; Mermaid 3,335:1 @ 1 834 MB/s ; Selkie 2,793:1 @ 3 146 MB/s. En chunks de 32 kB (corpus « seven ») : Kraken 2,615:1 @ 976 MB/s, Selkie 1,910:1 @ 4 097 MB/s, lzma 2,656:1 @ 43 MB/s — [cbloom, Oodle 2.6.0 some more perf comparisons](https://cbloomrants.blogspot.com/2018/03/oodle-260-some-more-perf-comparisons.html)
- RAD (page produit, consultée 2026) : Kraken « 3-5× plus rapide que zlib avec bien plus de compression » ; Mermaid 5-10× zlib ; Leviathan « compression comparable à 7z/LZMA, décodage 10× plus rapide », ~33 % plus lent que Kraken ; charger 1 Go : ~20 s en LZMA vs < 1 s en Kraken (claims vendeur, pas de corpus/CPU indiqués) — [RAD Oodle compressors](https://www.radgametools.com/oodlecompressors.htm) ; [RAD Leviathan](https://radgametools.com/oodleleviathan.htm)
- Ordre de vitesse de décodage selon Epic (UE5) : Selkie > Mermaid > Kraken > Leviathan — [Epic, Oodle Data](https://dev.epicgames.com/documentation/unreal-engine/oodle-data)
- Thread-phased decoding Kraken : décode un flux normal (sans chunking, sans perte de ratio) sur 2 threads, gain typique 1,4-1,9× ; ex. win81 827 → 1 399 MB/s — [cbloom](https://cbloomrants.blogspot.com/2016/05/oodle-kraken-thread-phased-decoding.html)
- enwik9 (LTCB, Matt Mahoney, màj 2026-03-25) : le classement est par taille compressée + décompresseur, orienté recherche IA, pas usage général — [LTCB](https://mattmahoney.net/dc/text.html) ; un test tiers (2026) : xz -9 ~2 % plus petit que zstd -22 --long mais zstd décompresse ~4× plus vite — [tech-insider](https://tech-insider.org/?p=57709)
- Squash benchmark : données « 8+ years old » selon un mainteneur Fedora — [Fedora devel list](https://lists.fedoraproject.org/archives/list/devel@lists.fedoraproject.org/message/3FEC76KARVJQ5ATCELXZL2BIXNKKJIND) ; TurboBench : dernière release 2023-08-06 — [repositorystats](https://repositorystats.com/powturbo/turbobench) ; OpenBenchmarking propose un test lzbench avec résultats publics jusqu'en juin 2024 — [OpenBenchmarking](https://openbenchmarking.org/test/pts/lzbench-1.1.1)
- zstd CLI : `--long[=#]` windowLog défaut 27 (128 MiB), max 31 ; au-delà de 27 le décompresseur doit recevoir `--long=#`, `--memory=` ou `--max` ; `-T#`/`-T0` (cœurs physiques), défaut `max(1, min(4, nbLogicalCores/4))` ; `--ultra` débloque 20-22 ; `--train` (fastcover par défaut, `--maxdict` 112 640 o) ; `--adapt`, `--rsyncable`, `--patch-from` ; limite mémoire décompression par défaut 128 MiB — [zstd man page](https://raw.githubusercontent.com/facebook/zstd/dev/programs/zstd.1.md)

### Inferences
- Sur CPU moderne, la vitesse de décompression zstd est ~quasi indépendante du niveau (1,1-1,35 GB/s mono-thread) alors que xz/LZMA plafonne à ~0,08-0,12 GB/s : à ratio quasi égal (zstd -22 4,05:1 vs xz -9 4,35:1, écart ~7 %), zstd décode ~9-11× plus vite **[calcul sur le tableau lzbench]**.
- Brotli -11 (4,20:1) se place entre zstd -22 et xz en ratio, mais décode 3× moins vite que zstd ; son coût de compression (0,56 MB/s) le réserve au contenu statique.
- Oodle Kraken/Leviathan (2018, i7-3770 ~2012) décodaient déjà à 0,8-1,0 GB/s mono-thread pour 4,1-4,35:1 ; sur un CPU 2026 on peut extrapoler ~1,5-2 GB/s **[estimation]**, ce qui les place au niveau de ratio de LZMA avec une vitesse ≥ zstd. Oodle reste propriétaire (licence RAD/Epic) : non intégrable dans un outil open source d'archivage.
- Pour un format d'archive « montable », zstd -15…-19 (3,7-4,0:1, décodage > 1,1 GB/s) est le meilleur compromis libre ; LZ4 pour les blocs à accès aléatoire fréquents (3,8 GB/s).

### Gaps
- Aucune mesure Oodle indépendante post-2018 (lzbench n'inclut pas Oodle, code fermé). Les figures Oodle 2.9.x restent celles de RAD.
- Pas de ligne « zstd -19 » ni « xz -9e » dans le run lzbench ; ratio LZMA2 7-Zip « ultra » (-mx9, dict 64 MB+) sur Silesia non isolé (la ligne lzma 26.03 -9 est le SDK LZMA 1 mono-flux).
- Les tables LTCB enwik9 pour zstd/xz/brotli n'ont pas été extraites (page non lue en entier).

## KQ2 — Décompression GPU : chiffres nvCOMP zstd/GDeflate/LZ4, coût PCIe, tailles de bloc

### Takeaway
Sur B200 (Blackwell, moteur matériel de décompression « DE »), nvCOMP atteint ~155 GB/s de sortie en zstd, ~173 GB/s en LZ4, ~84 GB/s en GZIP/Deflate (chunks 64 KB, 100 MB), mais une seule requête ne fait que 0,6-3 GB/s : le débit n'apparaît qu'avec des centaines de chunks en vol et des chunks ≥ 64-256 KB. Dès que les données résident côté hôte, le PCIe (12-25 GB/s ; ~4-5 GB/s de bout en bout mesuré par un projet tiers) devient le goulot, et le GPU n'est plus avantageux que si le ratio est élevé (on transfère le compressé) et si la sortie est consommée sur GPU.

### Cited Findings
- Microbenchmark B200 (Jarmusch & Chandrasekaran, arXiv 2512.02189, déc. 2025), DE, 100 MB, chunks 64 KB : LZ4 (ratio 1,00) 173 GB/s in / 173 GB/s out, 0,61 ms ; Snappy (1,91) 61 in / 117 out ; **Zstd (2,00) 77,5 in / 154,9 out, 0,68 ms** ; GZIP (2,00) 42 in / 83,8 out, 1,25 ms ; Cascaded 213 out ; Bitcomp (3,00) 154 in / 462 out ; ANS 539 out — [arXiv 2512.02189](https://arxiv.org/html/2512.02189)
- Même papier, sensibilité à la taille de chunk (débit pic) : 32 KB → 55,8 GB/s ; 64 KB → 71,7 ; 128 KB → 87,7 ; 256 KB → 112,1 GB/s ; saturation à 256-1 024 opérations en lot ; **une requête isolée : 0,63 GB/s (32 KB) à 3,13 GB/s (256 KB)** ; la sortie plafonne ~160-220 GB/s quel que soit le motif, l'entrée décroît en 1/ratio — [arXiv 2512.02189](https://arxiv.org/html/2512.02189)
- NVIDIA (blog 2025-10-06) : le DE existe seulement sur B200/B300/GB200/GB300 ; sur B200 tout buffer > 4 MB bascule en décompression logicielle sur SM (`CU_DEVICE_ATTRIBUTE_MEM_DECOMPRESS_MAXIMUM_LENGTH`) ; benchmarks en chunks 64 KiB et 512 KiB ; décompression « en transit » sur PCIe/C2C ; nvCOMP 5.0 — [NVIDIA blog DE](https://developer.nvidia.com/blog/speeding-up-data-decompression-with-nvcomp-and-the-nvidia-blackwell-decompression-engine/)
- Page produit nvCOMP : « jusqu'à 600 GB/s » de décompression pour formats standards via DE (nvCOMP ≥ 4.2) ; formats standards : Snappy, ZSTD, Deflate, LZ4 ; formats GPU-optimisés : Bitcomp, GDeflate, gANS, Cascaded — [developer.nvidia.com/nvcomp](https://developer.nvidia.com/nvcomp) (chiffre marketing pic, pas un benchmark)
- Repo GitHub nvcomp archivé ; exemples/benchmarks déplacés dans CUDALibrarySamples ; nvlzcat = outil CLI décompressant un seul flux gzip sur GPU — [GitHub nvcomp](https://github.com/NVIDIA/nvcomp) ; [nvlzcat](https://docs.nvidia.com/cuda/nvcomp/nvlzcat.html)
- Release notes nvCOMP : mode batched « jusqu'à 6× compression, 12× décompression » ; Deflate/GDeflate acceptent des chunks > 64 KB — [nvCOMP release notes](https://docs.nvidia.com/cuda/nvcomp/release_notes.html)
- Voltron Data (2023-09-27, A100, PCIe Gen4) : pour Snappy le débit plafonne vers chunks de 4 MB alors que le ratio se dégrade sous 64 kB ; pour zstd la dégradation de ratio commence à 256 kB ; décompression ~10× plus rapide que compression (propriété des codecs, pas du GPU) — [Voltron Data](https://voltrondata.com/blog/gpus-analytics-experiment-with-tuning-chunking-compression-decompression) (page non fetchable ce jour, résumé via moteur de recherche)
- Projet tiers s4-codec (Rust/nvCOMP) : PCIe ~25 GB/s avec buffers hôte pinned vs ~12 GB/s pageable ; débit **bout en bout PCIe inclus ~4-5 GB/s**, 20-50× sous les chiffres internes GPU — [s4-codec source](https://docs.rs/crate/s4-codec/0.8.9/source/src/ferro_compress/nvcomp.rs)
- Autres points de repère : nvCOMP Snappy ~180 GB/s sur B200 (cité VLDB ADMS 2025) — [ADMS25](https://www.vldb.org/2025/Workshops/VLDB-Workshops-2025/ADMS/ADMS25-01.pdf) ; nvCOMP DEFLATE ~50 GB/s sur A100 (preprint 2026) — [arXiv 2606.18900](https://arxiv.org/html/2606.18900) ; LZ4 69,5 GB/s sur une colonne float (ratio 1,11), deck NVIDIA A100 — [slides](https://reprises.in2p3.fr/ressource/Presentation/CWE41740_Slide_Deck_1648076975326001TKn2.pdf) ; GSST 191 GB/s @ 2,74:1 sur A100 (algo custom) — [TU Delft](https://repository.tudelft.nl/file/File_80803adf-2ac6-4a3a-9c5d-9895e5b9297c)
- Gompresso (Sitaridi et al., 2016, Tesla K40) : pic PCIe mesuré 13 GB/s ; variante octet « PCIe-bound » ; variante bit 2× zlib, variante octet 1,35× LZ4 (vs libs CPU multi-cœurs) — [arXiv 1606.00519](https://arxiv.org/pdf/1606.00519)
- ZipFlow (2026) : overlap PCIe/décompression, suppose PCIe ~32 GB/s théorique ; 2,08× vs nvCOMP, 3,14× vs DuckDB sur TPC-H — [arXiv 2602.08190](https://arxiv.org/pdf/2602.08190)
- FSST-on-GPU (VLDB ADMS 2025) : « LZ-based compression is a poor match to the GPU's SIMT model » — [ADMS25](https://www.vldb.org/2025/Workshops/VLDB-Workshops-2025/ADMS/ADMS25-01.pdf)
- ACEAPEX (2026) : décodage LZ77 parallèle via offsets absolus résolus à l'encodage ; chiffres GPU hors entropie et hors PCIe — [arXiv 2606.04268](https://arxiv.org/pdf/2606.04268)

### Inferences
- **Seuil de rentabilité PCIe [estimation]** : avec un CPU à ~1,3 GB/s/thread en zstd (lzbench) et 8-16 threads (≈ 8-15 GB/s agrégé si l'archive est chunkée), le GPU n'est gagnant que si (a) le compressé transite (coût PCIe = taille compressée / 12-25 GB/s) **et** (b) la sortie reste sur GPU ; si la sortie doit revenir en RAM hôte puis sur disque, le retour D2H (~taille décompressée / 12-25 GB/s) plus la copie annulent l'avantage face à 8+ cœurs CPU. Pour une extraction d'archive vers disque, le GPU est donc rarement rentable hors cas DirectStorage (sortie dans la VRAM).
- Tailles de bloc minimales efficaces : < 32 KB le débit DE chute (55 GB/s pic) et le ratio souffre ; 64 KB est le standard (GDeflate, nvCOMP) ; 256 KB-4 MB maximise le débit mais 4 MB est la limite DE B200 et dégrade le ratio zstd au-delà de 256 KB (Voltron). Zone pratique : **64-256 KB**.
- Les chiffres « 100+ GB/s » supposent des centaines de chunks indépendants en vol : un flux zstd mono-frame classique (sans chunking ni table d'offsets) ne peut pas être décodé ainsi.

### Gaps
- Pas de tableau NVIDIA officiel comparant GDeflate vs zstd vs LZ4 en GB/s sur le même GPU grand public (RTX 40/50) ; les seules données complètes sont B200 (DE) académiques.
- Aucun chiffre AMD (ROCm) ou Intel équivalent à nvCOMP pour des codecs généraux trouvé ; côté grand public l'équivalent AMD/Intel est le chemin DirectStorage (voir KQ3).
- Pas de mesure publiée « CPU N threads vs GPU + PCIe » pour une extraction d'archive de bout en bout ; le seuil ci-dessus est une estimation.

## KQ3 — DirectStorage : adoption 2023-2026, gains mesurés, spec GDeflate et outillage

### Takeaway
L'adoption réelle de la décompression GPU reste faible (Forspoken 2023, Ratchet & Clank 2023, Forza Motorsport 2023, Horizon Forbidden West 2024, Spider-Man 2 2025 ; SteamDB liste ~57 produits embarquant le SDK, sans preuve d'activation). Les gains mesurés sont surtout synthétiques (3DMark) ; en jeu, Spider-Man 2 a montré une **perte** de 8-18 % de fps avec décompression GPU sur RTX 4090/5090 selon Compusemble, corrigée seulement sur Blackwell d'après Tom's Hardware. DirectStorage 1.4 (GDC, 2026-03-11) ajoute Zstandard CPU+GPU (shader open source, chunks ≤ 256 KB) et GACL (+50 % de ratio zstd sur certaines textures BCn), drivers optimisés NVIDIA/AMD attendus S2 2026.

### Cited Findings
- GDeflate (NVIDIA, 2022-11-07) : extension de DEFLATE (RFC 1951) pour traitement data-parallel ; données découpées en **tuiles de 64 KB** décompressées indépendamment (= taille de tuile des ressources tiled DX/Vulkan) ; bitstream réorganisé pour qu'un groupe SIMD coopératif décode une tuile ; streaming non compressé limité à ~3 GB/s (PCIe Gen3) alors que les NVMe Gen4 fournissent jusqu'à 7 GB/s ; staging buffer par défaut 32 MB insuffisant → **128 MB** recommandé ; driver 526.47+ (RTX IO) ; implémentation de référence GDeflate publiée en open source sur GitHub — [NVIDIA blog GDeflate](https://developer.nvidia.com/blog/accelerating-load-times-for-directx-games-and-apps-with-gdeflate-for-directstorage/)
- DirectStorage 1.1 (2022-11) : GDeflate conçu par Microsoft/NVIDIA pour être supporté par tous les IHV ; AMD n'a d'abord fourni qu'un driver « metacommand » sous NDA aux ISV — [Tom's Hardware DS 1.1](https://www.tomshardware.com/news/microsoft-directstorage-11-now-available-amd-intel-nvidia-ready) ; [GPUOpen AMD DS 1.1](https://gpuopen.com/amd-support-for-microsoft-directstorage-1-1/)
- Intel : « jusqu'à 2,7× » de chargement plus rapide avec décompression GPU Arc A770 vs Core i9-12900K (test vendeur) — [Intel DS 1.1](https://intel.com/content/www/us/en/developer/articles/news/directstorage-on-intel-gpus.html)
- Jeux livrés avec DirectStorage : Forspoken (jan. 2023, chargements rapides mais ne sollicite pas le matériel), Ratchet & Clank: Rift Apart (juil. 2023, premier titre GDeflate + décompression GPU, meilleures perfs au lancement en supprimant les DLL DirectStorage, corrigé par patchs/hotfix NVIDIA), Forza Motorsport (oct. 2023), Horizon Forbidden West (mars 2024) ; la présence des DLL ≠ activation — [Corsair, state of DirectStorage](https://www.corsair.com/us/en/explorer/diy-builder/storage/the-state-of-microsoft-directstorage/) ; SteamDB tag « DirectStorage » : 57 produits — [SteamDB](https://steamdb.info/tech/SDK/DirectStorage) ; Ratchet & Clank premier titre GDeflate — [Tom's Hardware](https://tomshardware.com/pc-components/gpus/testing-directstorage-with-gpu-decompression-do-blackwell-gpus-have-the-upper-hand)
- Spider-Man 2 PC (Nixxes, 2025) : utilise la décompression GPU ; Compusemble mesure sur RTX 5090 4K **+8,2 % fps moyen / +10,5 % 1 % low en décompression CPU** vs GPU ; RTX 4090 : +10,3 % / +18 % en faveur du CPU — [Tom's Hardware 5090 vs 4090](https://www.tomshardware.com/pc-components/gpus/nvidia-rtx-5090-allegedly-handles-directstorage-gpu-decompression-better-than-rtx-4090) ; PC Gamer : la décompression GPU dégrade les perfs si activée — [PC Gamer](https://pcgamer.com/hardware/graphics-cards/spider-man-2s-pc-launch-woes-are-woe-ier-than-you-think-as-it-looks-like-gpu-powered-directstorage-is-hobbling-performance) ; Nixxes n'avait utilisé la décompression GPU que dans Ratchet & Clank auparavant (pas Ghost of Tsushima) — [Club386](https://www.club386.com/spider-man-2-performance-issues-partly-stem-from-directstorage)
- Tom's Hardware (test Blackwell, 2025) : RTX 5060/5070 à 1080p/1440p ne perdent aucune perf avec décompression GPU active (GPU > 98 % chargé), les générations précédentes (4090, 3090) souffrent ; le support ne peut pas être « backporté » dans un jeu existant car il faut repacker les assets en GDeflate — [Tom's Hardware](https://tomshardware.com/pc-components/gpus/testing-directstorage-with-gpu-decompression-do-blackwell-gpus-have-the-upper-hand)
- 3DMark DirectStorage feature test (2023) : sous-tests Storage→VRAM, Storage→RAM, RAM→VRAM, GDeflate decompression bandwidth, DS on/off ; synthétique, limite le GPU aux tâches DS ; UL note que mesurer DS en jeu réel est « très difficile » — [UL support](https://support.benchmarks.ul.com/support/solutions/articles/44002566118-3dmark-directstorage-feature-test-scores) ; [Tom's Hardware 3DMark](https://www.tomshardware.com/video-games/pc-gaming/3dmark-gets-new-directstorage-benchmark-gdeflate-measures-transfer-rates-from-nvme-ssd-to-gpu-vram)
- Driver NVIDIA : +17 % de décompression GPU sur SSD PCIe 5.0 (date non confirmée) — [Tom's Hardware](https://www.tomshardware.com/news/nvidia-driver-boosts-gpu-decompression-by-17-percent-on-pcie-50-ssds)
- **DirectStorage 1.4 (preview, 2026-03-11, GDC)** : codec Zstandard ajouté au framework multi-tier **CPU et GPU** ; shader compute zstd open source, optimisé initialement pour contenu chunké ≤ 256 KB ; post-traitement inversant les transformations GACL (shuffle) pour BC1/BC3/BC4/BC5, BC7 à venir ; GACL : « jusqu'à 50 % d'amélioration du ratio zstd » (shuffle lossless + BLER/CLER lossy perceptuel) ; optimisations driver : NVIDIA S2 2026, AMD driver public S2 2026, Intel « mois à venir », Qualcomm avant fin 2026 ; aucun chiffre vs GDeflate publié ; outils : `GpuDecompressionBenchmark` sample, NuGet 1.4.0-preview1 — [DirectX dev blog DS 1.4](https://devblogs.microsoft.com/directx/directstorage-1-4-release-adds-support-for-zstandard/) ; [GPUOpen AMD GDC 2026](https://gpuopen.com/learn/amd-microsoft-gdc-2026/)

### Inferences
- Trois ans après DS 1.1, la décompression GPU n'a de preuve de gain net en jeu que sur Blackwell ; sur RTX 30/40 elle coûte des fps parce que les SM sont partagés avec le rendu. Un outil d'archivage ne peut pas compter sur ce chemin hors contexte « sortie en VRAM ».
- La bascule de Microsoft vers zstd (DS 1.4) valide zstd comme codec commun CPU/GPU ; GDeflate devient un format de transition. Un format d'archive chunké zstd ≤ 256 KB devient compatible avec les deux chemins.
- Le fait que GACL conditionne les textures BCn (shuffle) rappelle qu'une grande part des gains vient des **transformations pré-compression** spécifiques au type de données, pas du codec.

### Gaps
- Aucun chiffre de temps de chargement avant/après (secondes) publié pour Spider-Man 2 ou Horizon FW avec/sans décompression GPU.
- Chiffres GB/s 3DMark par GPU non extraits (article Tom's Hardware tronqué au fetch).
- Pas de confirmation du driver AMD Adrenalin public ayant ajouté les optimisations GDeflate.

## KQ4 — 7-Zip 24.x/25.x : décompression LZMA2 multithread, parallélisable ?

### Takeaway
La décompression LZMA2 multithread existe dans 7-Zip depuis 18.03 (mars 2018) mais ne s'applique qu'aux flux LZMA2 composés de plusieurs blocs indépendants (créés à la compression multi-thread, taille via `-ms`) ; LZMA « classique » et un bloc LZMA2 unique restent strictement séquentiels (~78-122 MB/s mono-thread). Les versions 24.x/25.x n'ont rien changé à cela : 24.03 accélère LZMA/LZMA2 de 20-60 % sur ARM64, 24.09 augmente les dictionnaires par défaut, 25.00 autorise > 64 threads en compression, 26.01 ajoute les huge pages (+10 % compression Linux).

### Cited Findings
- 18.03 beta (2018-03-04) : « 7-Zip now can use multi-threading for 7z/LZMA2 decoding, if there are multiple independent data chunks in LZMA2 stream » — [7-zip.org history.txt](https://7-zip.org/history.txt)
- 24.01 (2024-01-31) : lecture ZSTD (.zst) et ZIP/SquashFS/RPM en zstd, hash XXH64, accélération RAR/CAB/WIM/ZIP/GZ ; 24.03 (2024-03-23) : LZMA/LZMA2 décompression +20-60 % sur ARM64 Windows ; 24.06/24.07 : correctifs zstd (CVE-2024-11477) ; 24.09 (2024-11-29) : dictionnaires LZMA/LZMA2 par défaut agrandis (32-bit ≤ 64 MB) ; 25.00 (2025-07-05) : > 64 threads CPU pour compression zip/7z/xz et benchmark ; 26.01 (2026-04-27) : huge pages 2 MB Linux (+~10 % compression LZMA/LZMA2) ; 26.02 : CVE-2026-14266 (overflow décompression XZ) — [7-zip.org history.txt](https://7-zip.org/history.txt)
- Taille de bloc solide : `-ms[Size]{m|g}` ; LZMA2 peut réinitialiser le dictionnaire entre chunks, permettant la décompression parallèle ; blocs plus petits = meilleur multithreading, ratio légèrement inférieur — [7-Zip history (mirror)](https://mirror-man.bytemark.co.uk/hp/repo/spp-gen11/2025.09.00.00/packages/x64/tools/7-Zip/History.txt) ; [mintlify wiki 7zip LZMA2](https://mintlify.wiki/ip7z/7zip/compression/lzma2) (source tierce)
- xz : diviser en blocs réduit le ratio (pas de matches inter-blocs) — [lrzip-next wiki](https://github.com/pete4abw/lrzip-next/wiki/Chunk-Block-Size-vs-Number-of-Threads) ; Inno Setup expose `LZMABlockSize` / `LZMANumBlockThreads` pour le même compromis — [Inno Setup](https://jrsoftware.org/ishelp/topic_setup_lzmablocksize.htm)
- Un décodage multithread exige de connaître la taille décompressée de chaque bloc à l'avance (feature request SourceForge) ; des utilisateurs rapportent une décompression mono-cœur sur archives non multi-blocs — [SourceForge #1095](https://sourceforge.net/p/sevenzip/feature-requests/1095) ; [Dopus forum](https://resource.dopus.com/t/7zip-lzma2-doesnt-use-multiple-cores/14893)
- Code : `C/Lzma2DecMt.c` (décodeur LZMA2 multi-thread) présent dans 7-Zip 24.09/25.00 — [sources.debian.org Lzma2DecMt.c](https://sources.debian.org/src/7zip-rar/24.09+ds-3~bpo12+1/C/Lzma2DecMt.c)
- Vitesse mono-thread de référence (lzbench 2.4, EPYC 9555P) : lzma SDK 26.03 -9 : 78 MB/s ; xz 5.8.4 -9 : 122 MB/s — [lzbench](https://github.com/inikep/lzbench)

### Inferences
- Parallélisme de décompression LZMA2 = parallélisme de **blocs** décidé à la compression : une archive .7z créée en `-mmt1` ou avec un très gros bloc solide se décompresse à ~80-120 MB/s, 10× sous zstd ; une archive de 8 Go en LZMA2 mono-bloc prend ~70-100 s de CPU pur **[calcul : 8 GB / 80-120 MB/s]**.
- Le ratio perdu par le chunking LZMA2 est faible pour des blocs ≥ 64-256 MB (dictionnaire plein) ; pour un format montable visant l'accès aléatoire, des blocs de l'ordre de 1-4 MB seraient nécessaires, ce qui rapproche LZMA2 des ratios zstd -19 **[estimation ; cf. perte de ratio 32 kB chunks dans les tables cbloom]**.
- Le décodage « thread-phased » (Kraken) montre qu'un décodeur LZ peut être parallélisé intra-bloc sans chunking, mais 7-Zip/xz n'implémentent pas cette approche.

### Gaps
- Pas de benchmark publié de la décompression LZMA2 multi-bloc 7-Zip 25.x (scaling en fonction du nombre de blocs/threads).
- Documentation officielle `-ms`/`-mmt` non récupérée (404) ; détails tirés de l'historique et de sources tierces.

## KQ5 — Pipelines hybrides CPU+GPU pour extraction d'archives ; extraction directe sans fichiers temporaires

### Takeaway
Aucun travail publié ni outil n'a été trouvé sur un pipeline CPU+GPU hybride pour extraire des archives généralistes (zip/7z) ; les travaux GPU (Gompresso 2016, nvCOMP, ZipFlow 2026, ACEAPEX 2026, FSST-GPU 2025) visent l'analytique in-GPU et concluent que LZ est mal adapté au SIMT et que le PCIe est le goulot dès que l'hôte est impliqué. Le seul outil CLI GPU grand public est `nvlzcat` (un flux gzip). Sur l'extraction « direct-to-disk » sans temporaire, rien de publié n'a été trouvé.

### Cited Findings
- Gompresso (2016, K40) : la variante octet est PCIe-bound (pic PCIe 13 GB/s) — [arXiv 1606.00519](https://arxiv.org/pdf/1606.00519)
- Brevet US 9923577 : DEFLATE hybride, matching octet + CRC sur CPU, codage bit sur accélérateur — [USPTO](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/9923577)
- FLARE (2025, compression lossy scientifique) : en configuration CPU-GPU, le PCIe représente 18 % / 22 % du temps de compression / décompression sur Hurricane — [arXiv 2507.01224](https://arxiv.org/pdf/2507.01224)
- ACEAPEX et suite (2026) : résolution d'offsets absolus à l'encodage pour décodage LZ77 parallèle ; variante « tout sur device » pour éliminer les allers-retours hôte des pipelines hybrides — [arXiv 2606.04268](https://arxiv.org/pdf/2606.04268) ; [arXiv 2606.18900](https://arxiv.org/html/2606.18900)
- ZipFlow (2026) : framework compilateur qui compresse sur CPU, transfère compressé, décompresse sur GPU en recouvrant PCIe et calcul — [arXiv 2602.08190](https://arxiv.org/pdf/2602.08190)
- `nvlzcat` : décompresse un seul bitstream gzip sur GPU, nécessite managed memory concurrente, pas de compression — [NVIDIA docs](https://docs.nvidia.com/cuda/nvcomp/nvlzcat.html) ; crush-gpu (Rust, format inspiré GDeflate, backend CUDA/wgpu) — [crush-gpu](https://docs.rs/crate/crush-gpu/latest/source/README.md) ; projet communautaire GPU + multi-process : « 4,53× vs séquentiel » — [GitHub oyanghd](https://github.com/oyanghd/gpu-compress-decompress)
- Blackwell DE : décompression « en transit » PCIe/C2C (copie + décompression fusionnées) — [NVIDIA blog DE](https://developer.nvidia.com/blog/speeding-up-data-decompression-with-nvcomp-and-the-nvidia-blackwell-decompression-engine/)
- DirectStorage = le seul pipeline « disque → GPU sans passer par la RAM décompressée » déployé ; il exige des assets repackés et un staging buffer 128 MB — [NVIDIA blog GDeflate](https://developer.nvidia.com/blog/accelerating-load-times-for-directx-games-and-apps-with-gdeflate-for-directstorage/)

### Inferences
- Un pipeline hybride pertinent pour l'archivage serait : CPU pour parsing conteneur/entropie des petits fichiers, GPU uniquement pour les gros blocs chunkés (≥ 64 KB, centaines en vol) dont la sortie est consommée sur GPU (textures, données analytiques). Pour une extraction vers disque, le gain est nul ou négatif **[inférence à partir du goulot PCIe et de l'absence de littérature]**.
- La « décompression en transit » Blackwell est la seule voie où le GPU décompresse sans surcoût de transfert, mais elle est réservée aux B200/B300 datacenter.
- « Direct-to-disk sans temporaire » relève de l'ingénierie système (streaming décodeur → `write` séquentiel, O_DIRECT/preallocation) plutôt que de la recherche ; aucun papier ne le traite.

### Gaps
- Aucune publication ni outil trouvé sur l'extraction hybride CPU+GPU d'archives zip/7z/tar.
- Aucun travail trouvé sur l'extraction « direct-to-disk » sans fichiers temporaires (ni mesure de son gain).

## KQ6 — Goulot réel à l'extraction d'une archive de 8 Go vers NVMe vs HDD

### Takeaway
Vers HDD, le goulot est le disque (≤ ~285 MB/s séquentiel en pic 7 200 tr/min, bien moins en écriture de nombreux petits fichiers) : 8 Go d'écriture prennent ≥ 30-60 s quel que soit le codec. Vers NVMe (Gen4 ~7 GB/s, Gen5 12-14 GB/s en rafale), le goulot bascule sur le CPU : LZMA/xz mono-bloc (~80-120 MB/s) → 70-100 s, zstd mono-thread (~1,2 GB/s) → ~7 s, zstd/LZ4 multi-thread sur archive chunkée → le disque ou la gestion des fichiers (métadonnées, petits fichiers) redevient limitant. Aucune mesure contrôlée NVMe vs HDD d'un extracteur n'a été trouvée ; les chiffres ci-dessous sont des calculs à partir des débits mesurés.

### Cited Findings
- HDD 7 200 tr/min : plafond ~285 MB/s séquentiel ; les écritures longues ralentissent des pistes externes vers internes — [tech-insider SSD vs HDD 2026](https://tech-insider.org/ssd-vs-hdd-2026/) (source secondaire) ; Tom's Hardware mesure les écritures soutenues HDD sur 15 min Iometer (IronWolf Pro 30 TB) — [Tom's Hardware](https://www.tomshardware.com/pc-components/hdds/seagate-ironwolf-pro-30tb-hdd-review/2)
- NVMe PCIe 5.0 : Crucial T705 14 500 MB/s lecture / 12 700 MB/s écriture ; WD SN8100 14 000 MB/s écriture ; mesure 12 005 MB/s écriture ; throttling possible en soutenu — [TechSpot best storage 2025-26](https://www.techspot.com/bestof/storage-25-26/) ; [gadgetscout 9100 Pro vs SN8100](https://www.gadgetscout.co.uk/articles/samsung-9100-pro-vs-wd-black-sn8100-the-fastest-pcie-5-0.html) ; [Creative Bloq](https://www.creativebloq.com/photography/cameras-accessories/the-best-ssds-for-workstations-in-year)
- NVMe Gen3/Gen4 grand public : jusqu'à 7 GB/s ; PCIe Gen3 ~3 GB/s — [NVIDIA blog GDeflate](https://developer.nvidia.com/blog/accelerating-load-times-for-directx-games-and-apps-with-gdeflate-for-directstorage/)
- Vitesses CPU mono-thread (lzbench 2.4, EPYC 9555P) : lz4 3 832 MB/s ; zstd -1…-22 : 1 087-1 351 MB/s ; brotli -11 383 MB/s ; zlib 314 MB/s ; xz -9 122 MB/s ; lzma -9 78 MB/s — [lzbench](https://github.com/inikep/lzbench)
- Anecdotes forums : extraction d'un RAR de 2 Go à 70-91 MB/s sur HDD Seagate 2 TB + i5-4670K — [AnandTech forums](https://forums.anandtech.com/threads/what-is-your-7-zip-speed-on-ssd-vs-hard-drive.2377756/) ; 20 000 petits fichiers transférés bien plus lentement qu'un fichier de 400 MB — [Tom's Hardware forums](https://forums.tomshardware.com/threads/slow-write-speed-with-nvme-after-extracting-from-archive.3599728) ; extraire d'un HDD vers SSD peut aussi ralentir (lecture source) — [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/a/7682097)
- Intel QAT (Sapphire Rapids, RHEL 9.1, Silesia) : compression zlib/deflate 73 Gb/s (~9 GB/s) pic avec ≥ 4 process et 1,6 cœur ; 1 process 34 Gb/s vs 0,25 Gb/s logiciel ; plugin QAT-zstd compression seulement, pas de décompression, « 3,2× zstd -5 » sur requêtes 16 KB — [Red Hat](https://www.redhat.com/en/blog/red-hat-enterprise-linux-achieves-significant-performance-gains-intels-4th-generation-xeon-scalable-processors) ; [Intel QAT-zstd plugin](https://community.intel.com/t5/Blogs/Tech-Innovation/Artificial-Intelligence-AI/Intel-QuickAssist-Technology-Zstandard-Plugin-an-External/post/1509818) ; [Phoronix](https://www.phoronix.com/news/Intel-QAT-Zstd-0.1)
- PS5 : décompresseur Kraken matériel, SSD 5,5 GB/s → **8-9 GB/s typiques en sortie, 22 GB/s max** ; équivalent ~9 cœurs Zen 2 ; avec Oodle Texture, ratio accru → ~17 GB/s calculés par RAD — [AnandTech](https://at-web1.www.anandtech.com/show/15848/storage-matters-xbox-ps5-new-era-of-gaming/2) ; [cbloom 2020](https://cbloomrants.blogspot.com/2020/09/how-oodle-kraken-and-oodle-texture.html) ; Kraken logiciel sur 9900K ~1-1,5 GB/s (forum) — [Beyond3D](https://forum.beyond3d.com/threads/next-generation-nvme-ssd-and-i-o-technology-pc-ps5-xbsx-s.62015/page-60)
- Apple : aucun bloc matériel de décompression LZ documenté publiquement sur Apple silicon ; LZFSE ≈ zlib -5 à 2-3× la vitesse (logiciel) ; nouveaux codecs **LZMESH** (3,5× plus rapide que zlib au décodage sur Apple silicon, OS 27+), **LZRAVEN** (remplace LZMA, décode 3× plus vite, vectorisé), LZBITMAP (SIMD) — [Apple LZMESH](https://developer.apple.com/documentation/compression/compression_lzmesh.md) ; [Apple LZRAVEN](https://developer.apple.com/documentation/compression/compression_lzraven.md) ; [Apple LZFSE](https://developer.apple.com/documentation/compression/compression_lzfse.md)

### Inferences
- **Budget temps pour 8 Go [calcul]** : écriture HDD (200-285 MB/s) : 28-40 s en séquentiel idéal, souvent 60-120 s avec petits fichiers → le codec est secondaire tant qu'il décode > 300 MB/s (zstd, LZ4, brotli OK ; xz/LZMA mono-bloc devient co-limitant). Écriture NVMe Gen4 (5-7 GB/s soutenu optimiste) : 1,2-1,6 s → le CPU est le goulot pour tout codec mono-thread ; il faut ≥ 4-6 threads zstd (archive chunkée) pour approcher le disque ; LZ4 mono-thread (3,8 GB/s) s'en rapproche seul.
- Les accélérateurs matériels pertinents pour l'extraction sont ceux qui sortent en RAM hôte (QAT : compression seulement pour zstd ; PS5/Blackwell DE : non disponibles sur PC grand public). Sur PC 2026, le chemin le plus efficace reste CPU multi-thread + format chunké.
- La variable dominante après le disque et le codec est le **nombre de fichiers** (création, métadonnées, antivirus) ; un format d'archive montable qui évite l'extraction physique contourne ce coût.

### Gaps
- Aucun benchmark contrôlé « extraction 7z/zip/tar.zst de N Go vers HDD vs SATA SSD vs NVMe » trouvé ; les chiffres sont dérivés.
- Pas de mesure d'écriture soutenue HDD récente extraite (Tom's Hardware IronWolf Pro : chiffres non récupérés).
- Pas de chiffres publics sur un éventuel moteur de décompression matériel Apple ; Qualcomm/Snapdragon X non couverts.
