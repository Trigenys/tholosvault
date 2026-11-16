# Recompression sémantique / format-aware, dictionnaires partagés et écosystème « repacker » (état de l'art 2020-2026)

Notes de recherche (français, termes techniques en anglais conservés). Date de rédaction : 2026-11-16. Chaque chiffre est daté/sourcé ; les estimations sont marquées **[estimation]**. Statut de maintenance des outils indiqué quand il a pu être vérifié.

---

## Q1. Precomp / preflate / reflate : reconstruction bit-exacte de zlib (taux de réussite, vitesse, gains)

### Takeaway
La reconstruction bit-exacte de Deflate n'est pas « réussie ou ratée » depuis preflate (2018) : on stocke le texte clair + un petit « diff » de corrections, ce qui marche sur ~100 % des flux valides testés (≈10 000 flux), au prix d'une lenteur notable ; les gains sont spectaculaires sur des conteneurs zip/PDF/PNG (Silesia.zip : 99,7 % → 69,7 % de la taille d'origine), mais les outils historiques (precomp, preflate C++) sont quasi gelés depuis 2019-2021, le relais étant pris par des ports Rust de Microsoft (preflate-rs) et par xtool côté repack.

### Cited Findings

**Precomp (schnaader/precomp-cpp)**
- Precomp décompresse les flux reconnus (zlib/Deflate dans PDF, PNG, ZIP ; bZip2 ; GIF ; JPG via packJPG/brunsli ; MP3 via packMP3) ; « si la recompression reproduit le flux d'origine bit pour bit, les données décompressées sont stockées à la place du flux compressé ». Sortie par défaut : fichier `.pcf` recompressé en LZMA2 ; `-cn` sort les données décompressées + données de reconstruction (plus gros que l'original, mais mieux compressible par un codec plus fort) ; `-r` restaure et le README vérifie par `diff`. — [GitHub precomp-cpp](https://github.com/schnaader/precomp-cpp)
- Bibliothèques embarquées (README) : packJPG v2.5k, brunsli (+ brotli pour les métadonnées JPG), packMP3 v1.0g, zlib 1.2.11, bZip2 1.0.6, GifLib 4.1.4, liblzma 5.2.3, **preflate v0.3.5** « pour créer et utiliser les informations de reconstruction des flux Deflate ». — [GitHub precomp-cpp](https://github.com/schnaader/precomp-cpp)
- Gain mesuré (README) : `silesia.zip` 67 633 896 o → 7-Zip LZMA2 Ultra 67 405 052 o (**99,7 %**) vs Precomp **47 122 779 o (69,7 %)**. — [GitHub precomp-cpp](https://github.com/schnaader/precomp-cpp)
- v0.4.7 : « Merge with the excellent preflate library to support recompression of all zLib streams — thanks to Dirk Steinke ». — [newreleases.io precomp v0.4.7](https://newreleases.io/project/github/schnaader/precomp-cpp/release/v0.4.7)
- Dernière release officielle : **22 février 2019** (« Last Release over 7 years ago »). Le dépôt n'est pas archivé mais aucune release depuis ; copyright 2006-2021, 389 commits. — [releasealert precomp-cpp](https://releasealert.dev/github/schnaader/precomp-cpp) ; [GitHub precomp-cpp](https://github.com/schnaader/precomp-cpp)

**preflate (deus-libri, Dirk Steinke)**
- Principe : scinder un flux raw deflate en données non compressées + « reconstruction information », puis reconstruire l'original exact. Utile « quand la position ou la taille du flux ne doit pas changer, p. ex. flux incorporés dans des exécutables ». — [GitHub preflate](https://github.com/deus-libri/preflate)
- Exactitude : « preflate a inflaté et reconstruit la totalité d'environ **dix mille** flux deflate valides » en test ; l'auteur admet des « corner cases inconnus possibles » et des crashs possibles sur flux invalides — donc pas de garantie formelle 100 %. — [GitHub preflate](https://github.com/deus-libri/preflate)
- Taille des données de reconstruction : flux produits par zlib (tout niveau) → « quelques octets » ; **20-30 % des flux zlib** nécessitent plus de 3 octets, généralement quelques dizaines d'octets. grittibanzli (Google) produit des données de reconstruction compressées typiquement 20-30 % plus grosses ; reflate plus grosses encore. — [GitHub preflate](https://github.com/deus-libri/preflate)
- Vitesse : « quite slow » ; v0.3.3 a ajouté un pool de tâches multi-cœur, mais le mono-thread reste faible ; sur petits flux / mono-cœur, les anciens precomp et reflate sont « environ 50-500 % plus rapides » **[estimation de l'auteur]**. — [GitHub preflate](https://github.com/deus-libri/preflate)
- Version courante **v0.3.5** (bug-fix), 32 commits, Apache-2.0, 41 étoiles ; maintenance de fait arrêtée (pas de date de commit lisible, aucune release depuis l'intégration dans precomp 0.4.7 en 2019). — [GitHub preflate](https://github.com/deus-libri/preflate)
- Historique selon les notes de l'auteur : « precomp » faisait la reconstruction bit-correct très efficacement **mais seulement pour les flux créés par zlib** ; « reflate » reconstruit n'importe quel deflate (7zip, kzip…) mais n'est « proche du parfait que pour zlib niveau 9 ». — [preflate-rs README (docs.rs)](https://docs.rs/crate/preflate-rs/0.7.1/source/README.md)

**Pourquoi le round-trip bit-exact échoue (ou coûte cher)**
- L'encodeur doit rejouer les choix du compresseur d'origine, qui diffèrent par implémentation : zlib « plus ou moins parfaitement modélisé » ; **miniz** utilise en mode rapide une fonction de hachage différente ; **libdeflate** utilise des tables de hachage 4 octets (détection tentée) ; **zlib-ng** correspond au modèle sauf niveau 9 ; les tables de Huffman dynamiques sont estimées par l'algorithme zlib mais « il y a plusieurs façons de construire des codes Huffman limités en longueur plus ou moins optimaux ». Quand la prédiction est fausse, un « difference encoder » enregistre chaque token divergent ; la taille des corrections dépend de la proximité du modèle. — [preflate-rs README](https://docs.rs/crate/preflate-rs/0.7.1/source/README.md)
- **preflate-rs (Microsoft)** : port Rust de preflate C++, 5 releases « breaking », v0.7.1 publiée (actif 2024-2026). — [lib.rs preflate-rs](https://lib.rs/crates/preflate-rs) ; [docs.rs preflate-rs](https://docs.rs/crate/preflate-rs/latest)
- Projet connexe : `deflate_fingerprint` cherche à identifier l'encodeur d'un flux par reproduction byte-exacte (cibles : zlib, libdeflate, 7-Zip, miniz, Go, .NET, Java, Apple CoreFoundation) — projet de rétro-ingénierie actif, pas un outil fini. — [ecosyste.ms deflate_fingerprint](https://awesome.ecosyste.ms/projects/github.com%2Fpmarreck%2Fdeflate_fingerprint)
- Limite pratique côté xtool : « Xtool peut traiter les flux deflate/zlib et les PNG contiennent ces flux, mais ils sont découpés en plusieurs blocs, ce qui empêche parfois le programme de les traiter ». — [krinkels.org XTool updates](https://krinkels.org/resources/xtool-2020.348/updates?page=2)

**paq8px (modèles de formats, recompression zlib par brute force)**
- Modèles/transformations : JPEG (subsampling, thumbnails, MJPEG), x86/x64 (+ pré-entraînement `-E`), DEC Alpha, images 1/8/24-32 bits (BMP, PNG, GIF, TIFF, TGA, PNM ; défiltrage PNG dans la couche transform), audio 8/16 bits + prédiction linéaire WAV, **recompression zlib/DEFLATE avec détection brute-force optionnelle (flag `B`)**, Base64/Base85/TAR, modèles texte (stemmers EN/FR/DE, pré-entraînement dictionnaire `-T`). — [GitHub paq8px](https://github.com/hxim/paq8px)
- Résultats : Silesia `-12L` 27 682 006 o (classé #1, mieux que cmix v21 avec dictionnaire 28 261 094) ; Calgary 568 427 o ; Kodak `-8L` 6 593 713 o (#1 sans modèles pré-entraînés). Vitesse ≈ **6,5 Ko/s** à `-8` (111 261 o en 17,12 s, dérivé) ; mémoire ≈ 583 Mo (`-1`) → 2,4 Go (`-8`) → 24,6 Go (`-12`), 32 Go+ recommandés ; fichiers > 2 Go non supportés. Version **v217 (2026)**, maintenu activement (868 commits). — [GitHub paq8px](https://github.com/hxim/paq8px)

### Inferences
- Le « taux de réussite » au sens precomp 0.4.x (recompression par essai de niveaux zlib) n'est plus la bonne métrique : avec preflate, tout flux valide est reconstruit, la variable devient la **taille du diff** (quelques octets pour zlib, beaucoup plus pour 7-Zip/kzip/libdeflate) et le **temps CPU**.
- Pour du contenu de jeu (pak zlib/LZ4/zstd/Oodle), les gains de precomp pur sont limités : il ne gère que Deflate/bzip2/JPG/MP3/GIF ; d'où l'émergence de xtool qui cible les codecs de jeu.
- Aucun chiffre public « gain typique sur PDF/docx » daté 2020-2026 n'a été trouvé hors l'exemple Silesia ; les docx/PDF étant des conteneurs zip/FlateDecode produits par zlib, on peut s'attendre à un comportement proche de l'exemple Silesia **[estimation]**, mais aucun benchmark récent ne le confirme.

### Gaps
- Pas de benchmark daté 2020-2026 de precomp/preflate sur un corpus de jeux, PDF ou docx (vitesse MB/s, gain %).
- Pas trouvé de fork maintenu de precomp (la recherche d'un « successeur » n'a rien donné) ; l'activité se concentre sur preflate-rs (Microsoft) dont le dépôt GitHub n'a pas été lu directement.
- La page Google Groups/encode.su sur reflate n'a pas été atteinte ; le statut de reflate (Eugene Shelwien) non vérifié.

---

## Q2. Lepton / Brunsli / JPEG XL (jbrd) : gains mesurés, vitesse, statut, adoption 2025-2026

### Takeaway
Les trois approches donnent le même ordre de grandeur (~20-23 % de réduction, bit-exact) ; Lepton est **archivé par Dropbox depuis le 14 février 2023** (vulnérabilités non corrigées), Microsoft maintient un port Rust ; Brunsli (22 %) survit comme bibliothèque peu active ; JPEG XL a absorbé le concept (coefficients DCT copiés dans VarDCT + boîte `jbrd`) et revient dans Chrome 145 (février 2026, derrière flag, décodeur Rust jxl-rs) et Firefox (2026), Safari 17 (2023) et Windows 11 l'ayant déjà.

### Cited Findings

**Lepton (Dropbox, 2016)**
- Réduction moyenne **22 %** sans perte (README) ; le papier Dropbox 2017 donne ≈ **23 %**. — [GitHub dropbox/lepton](https://github.com/dropbox/lepton) ; [arXiv 1704.06192](https://ar5iv.labs.arxiv.org/html/1704.06192)
- Vitesse rapportée (Xeon E5-2650 v2) : ≈ **5 Mo/s en compression, 15 Mo/s en décompression** (le titre du billet Dropbox dit « at 15MB/s »). — [Dropbox Tech blog](https://dropbox.tech/infrastructure/lepton-image-compression-saving-22-losslessly-from-images-at-15mbs) ; [Slashdot 2016](https://news.slashdot.org/story/16/07/14/1638246/dropbox-open-sources-new-lossless-middle-out-image-compression-algorithm)
- Échelle : 16 milliards d'images encodées, « multiples pétaoctets » économisés (2016). — [SourceForge mirror](https://www.sourceforge.net/projects/dropbox-lepton.mirror)
- Sûreté : la vérification se fait dans un **processus séparé (fork)** ; `-skipverify` la désactive ; un échec produit une sortie de 0 octet et un code retour non nul. — [GitHub dropbox/lepton](https://github.com/dropbox/lepton)
- Statut : dépôt **archivé/lecture seule le 14 février 2023** ; notice : Dropbox « n'a plus les ressources pour supporter ce projet », mentionne des **vulnérabilités récemment signalées qu'il ne corrigera pas** ; recommande lepton_jpeg_rust (Microsoft), Brotli, WebP ou JPEG XL. Licence Apache-2.0 ; pas de mention de brevets dans le README. — [GitHub dropbox/lepton](https://github.com/dropbox/lepton)
- **lepton_jpeg_rust (Microsoft)** : port Rust « car Dropbox a déprécié le code C++ » ; baseline + progressif ; « tout le contenu JPEG, y compris métadonnées et segments malformés, est préservé » ; ~22 % ; crate `lepton_jpeg` jusqu'à **0.5.8**, wrapper Python ; Apache-2.0. — [docs.rs lepton_jpeg](https://docs.rs/crate/lepton_jpeg/0.5.8/source/src/lib.rs) ; [README 0.2.0](https://docs.rs/crate/lepton_jpeg/0.2.0/source/README.md)

**Brunsli (Google)**
- « Lossless JPEG repacking library », **22 %** de réduction, récupération octet par octet. — [GitHub google/brunsli](https://github.com/google/brunsli)
- Statut : non archivé, dernier push **13 mai 2024**, 749 étoiles, 22 issues ouvertes (index synchronisé février 2026). — [ecosyste.ms brunsli](https://awesome.ecosyste.ms/api/v1/projects/github.com%2Fgoogle%2Fbrunsli)
- Historique : intégré dans PIK en 2017, outil autonome en 2019 ; JPEG XL a d'abord intégré Brunsli comme mode séparé, puis Luca Versari a déplacé la recompression dans le mode VarDCT, supprimant le « Brunsli mode ». Brunsli a apporté le comptage explicite de coefficients non nuls par bloc (remplace EOB) et les ordres de scan personnalisés. — [arXiv 2506.05987](https://arxiv.org/html/2506.05987v2)

**JPEG XL — recompression JPEG sans perte**
- Mécanisme : « les outils de codage de JPEG XL sont un sur-ensemble de ceux de JPEG » ; les coefficients DCT quantifiés d'un JPEG existant sont stockés dans des blocs VarDCT 8×8 ; le codage entropique Huffman est remplacé par ANS + LZ77 optionnel + context modeling. La boîte **`jbrd` (JPEG Bitstream Reconstruction Data)** est optionnelle, stockée à part, « l'image est décodable sans » ; elle contient les codes entropiques, le scan script progressif, le placement des restart markers et les bits de padding, et sert à la reconstruction **bit-identique**. — [arXiv 2506.05987 (juin 2025)](https://arxiv.org/html/2506.05987v2)
- Gain : « roughly 20 % » (les recompresseurs spécialisés font ~20 % contre 1-3 % pour un compresseur générique sur un JPEG) ; migration « completely risk-free ». — [arXiv 2506.05987](https://arxiv.org/pdf/2506.05987)
- Vitesse : l'article affirme une vitesse d'encodage/décodage « supérieure à JPEG sur processeurs multi-cœur », y compris à partir de données JPEG recompressées, mais **sans chiffre dans la partie lue** ; la Library of Congress cite 132 MP/s en décodage (sans préciser le cas jbrd). — [arXiv 2506.05987](https://arxiv.org/html/2506.05987v2) ; [LoC fdd000536](https://www.loc.gov/preservation/digital/formats/fdd/fdd000536.shtml)
- Limites : les modes JPEG arithmétique, 12 bits, hiérarchique et lossless étaient « largement non supportés dans les déploiements JPEG » ; l'article ne précise pas dans la partie lue lesquels sont exclus de la recompression (GDAL parle d'un « sous-ensemble largement supporté »). — [arXiv 2506.05987](https://arxiv.org/html/2506.05987v2) ; [GDAL JPEGXL driver](https://gdal.org/en/stable/drivers/raster/jpegxl.html)
- Usage pratique : GDAL avec `LOSSLESS=YES` sur une source JPEG fait un « lossless recoding to JPEG XL » et ajoute la « JPEG reconstruction box ». — [GDAL JPEGXL](https://gdal.org/en/stable/drivers/raster/jpegxl.html)

**Adoption JXL 2025-2026**
- Chrome **145 (février 2026)** réintroduit JXL via le décodeur **pur Rust jxl-rs** (remplaçant libjxl C++), derrière `chrome://flags/#enable-jxl-image-format`, pas activé par défaut ; Google a déposé un intent to ship. — [The Register 2026-01-14](https://www.theregister.com/2026/01/14/google_rekindles_relationship_with_jilted/) ; [Neowin](https://www.neowin.net/news/chrome-to-support-much-smaller-faster-images-as-jpeg-xl-makes-a-security-first-comeback/)
- Firefox : sources divergentes — Firefox 152 (16 juin 2026) en Firefox Labs vs intent to ship ciblant Firefox 157, même décodeur jxl-rs. — [hwbusters](https://hwbusters.com/news/jpeg-xl-in-firefox-157-mozilla-made-google-rewrite-the-decoder-in-rust-first/) ; [itdaily](https://itdaily.com/news/software/google-brings-jpeg-xl-back-to-chromium/)
- Safari 17 (2023), sans décodage progressif ; Windows 11 24H2 via extension Store, Microsoft Photos natif build 2025.11030 ; PDF Association ajoute JXL à la spec PDF en **novembre 2025**. — [design-reuse](https://www.design-reuse.com/blog/56307-universal-browser-support-for-jpeg-xl-is-your-hardware-ready-for-the-new-standard-/) ; [fileza 2026](https://fileza.io/articles/jpeg-xl-returns-chrome-2026)

### Inferences
- Pour une archive, JXL+jbrd est aujourd'hui le choix le plus durable pour la recompression JPEG : standard ISO, décodeurs multiples (libjxl C++, jxl-rs Rust), là où Lepton est un format propriétaire figé avec CVE non corrigées côté C++.
- Le « 22 % » est remarquablement stable entre packJPG/Lepton/Brunsli/JXL : c'est la limite pratique de la ré-entropie des coefficients DCT ; aller au-delà exige des modèles de contexte lourds type paq8px (et des vitesses en Ko/s).

### Gaps
- Pas de benchmark 2025 isolant la vitesse de décodage d'un `.jxl` issu de recompression (avec reconstruction JPEG) vs décodage du JPEG d'origine.
- Statut 2025-2026 exact du dépôt lepton_jpeg_rust (dernier commit) non vérifié.
- Aucune source sur des questions de brevets concernant Lepton ; le README et la notice d'archivage n'en parlent pas.

---

## Q3. Toolchain « scene »/repacker (FreeArc, srep, lolz, xtool, razor, zpaq) : ratios sur jeux AAA vs 7z ultra, temps d'installation

### Takeaway
Les seules données publiées de première main sont celles de FitGirl : sur 157 jeux (octobre 2021), 1 370 Go sources → ≈ 700-808 Go, soit ~50 % en moyenne, avec des extrêmes de 17,9 % (Disco Elysium) à 92 % (Boomerang X) et des installs de 4 min 20 s (16 threads) à 2 h (Killing Floor 2 sur 4 threads) ; la raison est structurelle : les outils « precompressent » les flux déjà compressés du jeu (zlib/LZ4/zstd/Oodle) via xtool, dédupliquent à longue portée (srep), puis appliquent LZMA/lolz, ce que 7z ultra ne peut pas faire sur des .pak déjà compressés.

### Cited Findings
- FitGirl FAQ : « J'utilise principalement **FreeArc** pour la compression et **Inno Setup** comme installeur. Certains jeux requièrent d'autres compresseurs, mais dans 99,9 % des cas c'est FreeArc. » — [FitGirl FAQ (miroir)](https://fg-3pc.pages.dev/faq/index.html)
- Rapport « Amelie R&D » daté 21/11/2021 : **157 jeux**, source 1 370 Go (décompressés 1 905 Go), repacks **698-808 Go** selon composants ; taille moyenne 4,5 Go, médiane 1,9 Go ; pire ratio **Disco Elysium 9,5 → 7,8 Go (17,9 %)** ; meilleur **Boomerang X 10,8 Go → 859 Mo (~92 %)** ; plus gros Conan Exiles 105 → 45,5 Go ; vitesse de traitement **63,5 Go de données décompressées/jour (≈ 44 Mo/min)**, écriture compressée ≈ moitié. — [Amelie R&D Department (miroir)](https://fg-3pc.pages.dev/category/amelie-rd-department/index.html)
- Temps d'installation (même rapport) : deux chiffres par repack — CPU moderne 16 threads/16 Go RAM vs ancien 4 threads/8 Go ; moyenne **4 min 20 s** (médiane 2 min) sur 16 threads, **9 min** (médiane 3 min) sur machine lente ; outlier Killing Floor 2 ≈ **2 h** sur 4 threads ; débit disque ≈ 20 Mo/s lecture / 50 Mo/s écriture (rapide) vs 10/24 (lent) ; multiplicateurs : CPU portable ×2, antivirus ×1,2-4. — [Amelie R&D](https://fg-3pc.pages.dev/category/amelie-rd-department/index.html)
- Exemple de listing : installation « de 6 minutes (CPU 16 threads) à 24 minutes (CPU 4 threads) », « le repack utilise la bibliothèque **XTool** de Razor12911 ». — [Résultat de recherche (listing repack)](https://edx.ipeindia.org/restless-stead/snowy-rapids/ultra-street-fighter-ivreloaded-fitgirl-repack) **[source SEO de faible fiabilité]**
- Témoignage utilisateur : la compression extrême « s'accompagne de temps de décompression très longs », jusqu'à ≈ 45 min sur un CPU haut de gamme **[anecdotique]**. — [Lemmy](https://lemmy.4d2.org/post/2032435/2714047)
- **xtool** (Razor12911, Delphi/Pascal, MIT 2016-2022) : dépôt GitHub **archivé le 28 octobre 2023**, 368 étoiles ; arborescence `precompressor`, `dbgenerator`, `cls_xtool` (composant CLS), `io`, `ui`. — [GitHub Razor12911/xtool](https://github.com/Razor12911/xtool)
- Changelog xtool : scanner + codec par bibliothèque ; 0.9.2 « updated lz4/lz4hc universal scanner, lz4/lz4hc/lz4f codecs, zlib codec, zstd codec » ; avertissement : « n'injecter lz4, zstd et oodle que si vous êtes sûr que l'entrée n'aura jamais besoin de **library swaps**, car ces bibliothèques, selon leur version, déterminent le ratio de précompression » ; une fonction de « recompression » (réencoder zlib/zstd/lz4/oodle à un niveau plus fort en exploitant le fait que les décodeurs n'échouent pas sur un flux différent) a été **retirée**. — [krinkels.org XTool updates](https://krinkels.org/resources/xtool-2020.348/updates?page=2)
- Une note de mise à jour xtool mentionne un pipeline **srep + lolz/lzma** en aval. — [krinkels.org XTool](https://krinkels.org/resources/xtool-2020.348/updates)
- **SREP (SuperREP)** : préprocesseur LZ77 « huge-dictionary » de Bulat Ziganshin, dictionnaires plus grands que la RAM disponible ; dernier code connu **3.93a beta, octobre 2014**, re-publié non officiellement sur GitHub. — [GitHub Intensity/srep](https://github.com/Intensity/srep) ; [Softpedia SREP](https://www.softpedia.com/get/Compression-tools/SuperREP.shtml)
- FreeArc : classé 1er du Maximum Compression Benchmark en novembre 2010 (données anciennes). — [Wikipedia FreeArc](https://en.wikipedia.org/wiki/FreeArc)
- Unreal Engine : le plugin **Oodle Data** fournit la compression des `.pak` et **IoStore**, activé par défaut, **Kraken** « compression élevée avec bonne vitesse de décodage, le défaut habituel » ; un utilisateur rapporte qu'IoStore n'accepte qu'Oodle (erreur en passant à Zlib) **[témoignage forum]**. Benchmark UE4 ShooterGame : Kraken 372 845 225 o vs zlib 417 951 648 o. — [Epic Oodle Data](https://dev.epicgames.com/documentation/unreal-engine/oodle-data) ; [cbloomrants 2018](https://cbloomrants.blogspot.com/2018/03/oodle-data-compression-integration-for.html)
- Unity AssetBundles : **LZMA** par défaut à la construction (tout le bundle doit être décompressé en RAM), **LZ4** par chunks de 128 Ko (fichier plus gros mais accès aléatoire), cache LZ4 par défaut ; le Web ne supporte pas LZMA. — [Unity Manual](https://docs.unity3d.com/kr/current/Manual/assetbundles-compression-format.html)

### Inferences
- Pourquoi ces ratios : un `.pak` Oodle/LZ4/zlib est opaque pour 7z ultra (≈ 1-3 % de gain). xtool restitue le texte clair des flux (avec données de reconstruction pour redevenir bit-exact), srep élimine la redondance à longue portée (assets dupliqués entre niveaux/langues, dizaines de Go), puis LZMA/lolz (contexte ~1 Go) compresse. Le gain dépend donc du **contenu** : jeux Unity/Unreal avec textures RDO ou audio déjà compressé (Disco Elysium 17,9 %) vs jeux avec beaucoup d'assets zlib/dupliqués (Boomerang X 92 %).
- Le « library swap » cité par xtool est le talon d'Achille : la reconstruction bit-exacte dépend de la **version exacte** de la bibliothèque (lz4, zstd, oodle) qui a produit le flux ; d'où une architecture par plugins/DLL versionnées (CLS = « Compression Library Standard » de FreeArc) et des bases de données de signatures (`dbgenerator`).
- Le temps d'installation est borné par la **recompression** des flux (réencodage Oodle/zlib au niveau d'origine) plus que par LZMA : ce coût est proportionnel au volume décompressé et mal parallélisable sur 4 threads, d'où les ×2-×4 entre machines.

### Gaps
- Aucune publication FitGirl/DODI/ElAmigos ne détaille un pipeline étape par étape ni la contribution de chaque outil (xtool vs srep vs lolz) au ratio ; la FAQ ne nomme que FreeArc/Inno Setup.
- Pas de thread encode.su primaire récupéré sur lolz (ProFrager) ni sur srep appliqué aux jeux ; les ratios « 40-70 % » circulant sur des pages SEO ne sont pas sourcés.
- Pas de comparaison contrôlée « même jeu : 7z ultra vs FreeArc+xtool+srep+lolz » avec temps mesurés.
- Vérification d'intégrité : les pages FitGirl lues ne décrivent pas la vérification post-installation (CRC/hash) ; non confirmé.

---

## Q4. Oodle Texture / RDO / BC7 : gains sur textures

### Takeaway
Les textures BCn sont incompressibles par LZ (BC7 + Kraken ≈ 1,06:1) ; Oodle Texture RDO (2020) rend les blocs plus redondants (lossy, mais contrôlé) pour atteindre ≈ 2:1 à 2,4:1 avec Kraken, et BC7Prep (transform lossless) ajoute 5-15 % ; chiffres éditeur (RAD/Epic), pas de benchmark indépendant trouvé.

### Cited Findings
- RAD : Oodle Texture RDO « peut réduire la taille des fichiers texture jusqu'à **50 %** ». — [RAD Oodle Texture](https://radgametools.com/oodletexture.htm)
- Benchmark cbloom (juin 2020) sur un jeu BC7 : baseline BC7 + Kraken ≈ **1,06:1** ; RDO lambda 40 (qualité moyenne) + Kraken ≈ **2,06:1** ; + BC7Prep ≈ **2,37:1** (1 079 916 → 455 747) ; lambda ≤ 30 = haute qualité. — [cbloomrants sample run](https://cbloomrants.blogspot.com/2020/06/oodle-texture-sample-run.html)
- **BC7Prep** : transformation **lossless** qui améliore typiquement la compression de BC7 de **5-15 %**, combinable avec RDO. — [cbloomrants BC7Prep data flow](https://cbloomrants.blogspot.com/2020/06/oodle-texture-bc7prep-data-flow.html)
- Exemple mixte BC1-7 : 127 Mo → 78 Mo (zlib) → 70 Mo (Kraken) → **40 Mo (Oodle Texture + Kraken)**. — [cbloomrants juin 2020](https://cbloomrants.blogspot.com/2020/06)
- Unreal : RDO « ne réduit que la taille de distribution » (pas la VRAM) et est « réglé pour fonctionner avec Kraken » ; notes 2.9.13 : RDO 5-8 % de réduction supplémentaire sur le jeu de test. — [Epic Oodle Texture](https://dev.epicgames.com/documentation/unreal-engine/oodle-texture-in-unreal-engine) ; [RAD what's new](https://radgametools.com/oodlewhatsnew.htm)
- Mermaid est « souvent légèrement moins compressif que zstd sur d'autres types de données, mais légèrement plus sur BCn ». — [cbloomrants juillet 2020](https://cbloomrants.blogspot.com/2020/07/performance-of-various-compressors-on.html?m=0)

### Inferences
- Pour un archiveur « sémantique », BC7Prep est le modèle à suivre : transformation **réversible** dépendant du format (réorganisation des champs de bloc) qui n'altère pas la donnée et gagne 5-15 % ; RDO, lui, est une décision d'éditeur (lossy) et n'est pas applicable a posteriori sans perte.
- Les jeux récents (UE5 avec Oodle Texture + Kraken) laissent peu de marge aux repackers sur les textures ; le gain résiduel vient des doublons (srep) et des autres flux.

### Gaps
- Pas de benchmark tiers indépendant d'Oodle Texture, ni de chiffres 2025-2026 pour les versions récentes.

---

## Q5. Dictionnaires partagés : zstd `--train`, dictionnaire Brotli, Shared Brotli, Compression Dictionary Transport (RFC 9842, Chrome 130) — conception, gains, identification/versionnage

### Takeaway
Compression Dictionary Transport (Chrome 130, fin 2024 ; RFC 9842, septembre 2025) identifie un dictionnaire par son **SHA-256** (header `Available-Dictionary`, et hash embarqué dans l'en-tête `dcb`/`dcz`), avec un `id` serveur opaque optionnel ; Google Search mesure **-23 % d'HTML en moyenne** (jusqu'à ~50 % par réponse) et +1,7 % LCP (9 % sur réseaux lents). Leçon pour les archives : l'identité d'un dictionnaire doit être un hash de contenu, jamais un nom ; zstd ne stocke qu'un `dictID` 32 bits aléatoire par défaut, insuffisant seul.

### Cited Findings

**Compression Dictionary Transport**
- Shipped **Chrome 130 / Edge 130 (fin 2024)** ; Firefox Technology Preview seulement ; Safari non ; non-support = ignoré (progressive enhancement). Billet du **14 mai 2025**. — [Chrome Developers blog](https://developer.chrome.com/blog/search-compression-dictionaries)
- Google Search (déploiement printemps 2025, utilisateurs Chrome) : taille moyenne du HTML **-23 % vs Brotli standard** sur tous les utilisateurs Chrome (y compris ceux sans dictionnaire) ; exemple 107 Ko `br` → 60 Ko `dcb` ; **LCP +1,7 %**, jusqu'à **+9 %** sur réseaux à forte latence. Dictionnaire = échantillon représentatif de résultats, **rafraîchi plusieurs fois par jour** par pipeline automatisé. Le prédécesseur SDCH a été désactivé pour risques d'attaques oracle par canal auxiliaire. Inspection : `chrome://net-internals/#sharedDictionary`. — [Chrome Developers blog](https://developer.chrome.com/blog/search-compression-dictionaries)
- RFC 9842 (**septembre 2025**, Standards Track, Meenan/Weiss) : `Use-As-Dictionary` (réponse) avec `match` (URL Pattern, même origine, requis), `match-dest`, `id` (opaque, ≤ 1024 car., « le client ne doit pas le traiter comme garantie du contenu »), `type` (défaut `raw`) ; `Available-Dictionary` (requête) = **SHA-256** du meilleur dictionnaire ; `Dictionary-ID` renvoie l'`id`. Sélection : match-dest > plus long match > plus récent ; le dictionnaire doit être frais (ou stale autorisé). — [RFC 9842](https://www.rfc-editor.org/rfc/rfc9842.html)
- Encodages : **`dcb`** = en-tête 36 o (magic 4 o + SHA-256 32 o du dictionnaire) + flux Shared Brotli, fenêtre jusqu'à 16 Mo ; **`dcz`** = en-tête 40 o (magic 8 o + SHA-256) encapsulé en **skippable frame zstd** (décodeurs existants l'ignorent), fenêtre ≥ max(8 Mo, 1,25× dictionnaire) jusqu'à 128 Mo. `Vary: accept-encoding, available-dictionary` obligatoire pour le cache. — [RFC 9842](https://www.rfc-editor.org/rfc/rfc9842.html)
- Sécurité/vie privée : HTTPS seulement ; même origine (ou CORS lisible) ; ne pas mélanger données publiques/privées dans un même contexte de compression (renvoi à RFC 7457) ; les hashs de dictionnaire pouvant servir de traceurs, ils sont partitionnés et effacés comme des cookies. Le RFC ne définit **aucun schéma de versionnage** ni durée de rétention au-delà de la fraîcheur HTTP ; il prévoit qu'un futur paramètre puisse changer l'algorithme de hash. — [RFC 9842](https://www.rfc-editor.org/rfc/rfc9842.html)
- Deux usages : « delta » (ancienne version `app.v1.js` sert de dictionnaire pour `app.v2.js`) vs « shared dictionary » (fichier séparé commun à plusieurs pages). Un article tiers avance 60-90 %+ de réduction pour le cas delta **[illustratif, non mesuré par Chrome]**. — [Chrome blog](https://developer.chrome.com/blog/search-compression-dictionaries) ; [Web Perf Calendar 2024](https://calendar.perfplanet.com/2024/getting-real-small-with-compression-dictionaries/)

**Brotli (dictionnaire intégré)**
- RFC 7932 (juillet 2016, Informational) : dictionnaire statique de **122 784 octets**, CRC-32 0x5136cb04 ; chaque mot a **121 formes** via 21 transformations élémentaires (Identity, FermentFirst/All, OmitFirst1-9, OmitLast1-9). Le dictionnaire est figé dans la spec (le CRC permet d'en vérifier la conformité) ; aucun mécanisme de version. — [RFC 7932](https://www.rfc-editor.org/rfc/rfc7932.html)

**zstd dictionaries**
- `zstd --train samples/* -o dict`, `--maxdict` ; entraîner **au niveau de compression de production** (statistiques accordées → petit gain) ; > 100 échantillons recommandés. — [zstd docs (miroir mintlify)](https://www.mintlify.com/facebook/zstd/concepts/dictionary-training) ; [man zstd](https://man.archlinux.org/man/zstd.1.en.txt)
- `dictID` **aléatoire par défaut**, réglable `--dictID` (ID < 256 = 1 octet d'en-tête) ; les frames enregistrent l'ID et le décodeur vérifie la concordance (« Zstd will check if there is a dictionary ID mismatch ») ; pattern : comparer `ZSTD_getDictID_fromDDict` et `ZSTD_getDictID_fromFrame` et échouer explicitement. Les **raw-content dictionaries** n'ont ni en-tête ni ID ni tables d'entropie → pas de détection de mismatch. — [zstd CLI dictionary](https://www.mintlify.com/facebook/zstd/cli/dictionary) ; [python-zstandard docs](https://python-zstandard.readthedocs.io/en/0.20.0/dictionaries.html)

### Inferences
- Leçons pour un format d'archive : (1) identifier le dictionnaire par **hash de contenu** (SHA-256) embarqué dans le flux comme `dcb`/`dcz`, pas par un ID 32 bits aléatoire ; (2) rendre l'en-tête **ignorable** par les décodeurs legacy (skippable frame) ; (3) traiter chaque dictionnaire entraîné comme un **artefact immuable** livré avec l'archive (ou référencé par hash) ; (4) séparer les données de confiance différente dans des contextes de compression distincts (risque de fuite type CRIME/BREACH si un attaquant contrôle une partie de l'entrée).
- Le dictionnaire Brotli illustre le coût de la durabilité : figé depuis 2016, impossible à mettre à jour sans nouveau content-encoding — c'est précisément ce qui a motivé Shared Brotli/CDT.
- Un dictionnaire externe non livré avec l'archive est un **point de défaillance de lisibilité à long terme** : perdre le dictionnaire = perdre toutes les frames qui en dépendent.

### Gaps
- Pas de chiffres Chrome publiés pour `dcz` (zstd) ni par percentile ; pas de données d'autres déploiements que Google Search.
- Statut 2026 de Firefox/Safari sur CDT non vérifié au-delà de « Technology Preview / non supporté » (mai 2025).
- Le brouillon Shared Brotli (draft-vandevenne-shared-brotli) n'a pas été lu ; son statut IETF n'est pas confirmé ici.

---

## Q6. Compression « sémantique » de binaires structurés (protobuf, SQLite, saves/assets de jeux) avec transformations réversibles

### Takeaway
La littérature trouvée traite surtout du **columnar / numérique** (Parquet-style preprocessing, Pcodec +29-94 % vs codecs numériques, filtres d'octets réversibles) ; rien de spécifique au wire format protobuf ou à SQLite comme recompression réversible n'a été trouvé — c'est un trou dans la littérature ouverte, les pratiques existantes étant des transformations ad hoc (BC7Prep, x86 E8/E9, délinéarisation PNG dans paq8px).

### Cited Findings
- Pcodec : « 29-94 % de ratio de compression supérieur aux autres codecs numériques sur six jeux de données columnar réels » ; Parquet « supporte une étape de préprocessing avant la compression LZ ». — [arXiv 2502.06112](https://arxiv.org/pdf/2502.06112v2)
- PVLDB 2025 : évaluation de la compression lossless de colonnes flottantes, bibliothèque open source. — [PVLDB vol18 p4396](https://www.vldb.org/pvldb/vol18/p4396-hishida.pdf)
- Transformations réversibles par filtres d'octets pour données scientifiques columnar (projet spectra8) ; réorganisation columnar de JSON/NDJSON avant compression (datacortex) — implémentations, pas recherche. — [docs.rs datacortex-core](https://docs.rs/crate/datacortex-core/0.5.0/source/src/format/mod.rs)
- Transformations réversibles existantes dans les outils : paq8px (x86/x64, DEC Alpha, PNG unfilter, Base64/85, TAR) ; BC7Prep (textures) ; preflate (deflate). — [GitHub paq8px](https://github.com/hxim/paq8px) ; [cbloomrants BC7Prep](https://cbloomrants.blogspot.com/2020/06/oodle-texture-bc7prep-data-flow.html)
- Brevets anciens sur « reversible DCT » / « reversible overlap operator » (Microsoft) montrent que les transformations entières réversibles sont un terrain breveté historiquement. — [USPTO 7551789](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/7551789)

### Inferences
- Le schéma général est toujours le même : **parser → représentation canonique « plate » (colonnes, coefficients, texte clair) + diff de reconstruction → codec générique**. Protobuf (varints, champs répétés) et SQLite (pages B-tree, record format) s'y prêtent en théorie (transposition par champ + varint → delta), mais aucune implémentation publiée avec garantie bit-exacte n'a été identifiée **[estimation]**.
- La difficulté n'est pas la compression mais le **round-trip** : ordre de champs non canonique, champs inconnus, padding, free pages — exactement les cas qui ont imposé le « diff » dans preflate.

### Gaps
- Pas de papier/outil trouvé sur la recompression réversible du wire format protobuf, de bases SQLite, ni de formats de sauvegarde de jeux.

---

## Q7. Risques : corruption silencieuse, garde-fous (hash), aspects légaux, lisibilité à long terme

### Takeaway
Tous les outils sérieux vérifient le round-trip **à l'encodage** (recompression + comparaison, souvent dans un processus séparé) et refusent de remplacer un flux non reproductible ; les risques réels sont ailleurs : dépendance à la **version exacte** d'une bibliothèque tierce (Oodle, zstd, lz4), outils gelés/archivés (precomp 2019, preflate 0.3.5, xtool oct. 2023, Lepton fév. 2023 avec CVE), et dictionnaires externes perdus.

### Cited Findings
- Precomp ne substitue un flux **que si** la recompression reproduit l'original bit à bit ; sinon le flux est laissé tel quel. — [GitHub precomp-cpp](https://github.com/schnaader/precomp-cpp)
- preflate : « corner cases inconnus possibles » sur flux valides, crashs possibles sur flux invalides. — [GitHub preflate](https://github.com/deus-libri/preflate)
- Lepton : vérification dans un processus séparé (fork), désactivable par `-skipverify` ; échec = sortie 0 octet + code non nul ; projet archivé 2023-02-14 avec **vulnérabilités signalées non corrigées**. — [GitHub dropbox/lepton](https://github.com/dropbox/lepton)
- xtool : les bibliothèques lz4/zstd/oodle « selon leur version déterminent le ratio de précompression » (library swaps) ; la fonctionnalité de « recompression » à niveau plus fort reposait sur le fait que ces décodeurs **n'échouent pas sur un flux incorrect** — retirée. — [krinkels.org XTool](https://krinkels.org/resources/xtool-2020.348/updates?page=2)
- JXL : la reconstruction bit-exacte dépend de la boîte `jbrd` **optionnelle** ; sans elle l'image reste décodable mais le JPEG d'origine n'est pas restituable. — [arXiv 2506.05987](https://arxiv.org/html/2506.05987v2)
- Chrome/CDT : SDCH abandonné pour attaques oracle ; RFC 9842 impose HTTPS, même origine, séparation des données sensibles, partitionnement des dictionnaires. — [RFC 9842](https://www.rfc-editor.org/rfc/rfc9842.html)
- zstd : dictID aléatoire vérifié au décodage, mais absent pour raw-content dictionaries. — [zstd CLI dictionary](https://www.mintlify.com/facebook/zstd/cli/dictionary)
- Statuts de maintenance (vérifiés) : precomp dernière release 2019-02-22 ; preflate v0.3.5 ; xtool archivé 2023-10-28 ; Lepton archivé 2023-02-14 ; Brunsli dernier push 2024-05-13 ; SREP 3.93a 2014 ; paq8px v217 actif 2026 ; preflate-rs 0.7.1 et lepton_jpeg 0.5.8 actifs (Microsoft). — sources respectives ci-dessus.

### Inferences
- La « corruption silencieuse » classique (flux réécrit mais différent) est bien couverte par la vérification à l'encodage ; le risque résiduel est (a) un **bug du décodeur de reconstruction** dans une version future différente de celle qui a vérifié, (b) un flux dont la reconstruction dépend d'une **DLL tierce propriétaire** (Oodle) non redistribuable, (c) un dictionnaire externe introuvable. Un format d'archive devrait donc stocker : hash du flux original, hash + version de la bibliothèque de reconstruction, et idéalement le diff suffisant pour reconstruire sans la bibliothèque d'origine (approche preflate plutôt qu'approche « re-run the same lib »).
- Légal : Oodle est propriétaire (gratuit dans Unreal via GitHub, licence sinon) ; Lepton/Brunsli/JXL/preflate sont Apache-2.0 ; aucune source trouvée mentionnant un brevet bloquant sur Lepton. — [Unreal blog Oodle](https://www.unrealengine.com/en-US/blog/oodle-now-free-to-use-in-unreal-engine-via-github)

### Gaps
- Pas de source détaillant les CVE de Lepton (numéros, nature).
- Pas de source sur la façon dont FitGirl/xtool vérifient l'intégrité finale (CRC Inno Setup ? hash par fichier ?).
- Aucune analyse publiée de la « lisibilité à 10 ans » des `.pcf` precomp ou des archives FreeArc/xtool.

---

## Annexe — Audio lossless (FLAC / WavPack) : repères

### Takeaway
Sur l'audio PCM, les écarts entre codecs lossless sont faibles (FLAC à ≤ 3 % des meilleurs, WavPack ≈ 63 % du WAV, meilleur que FLAC sur 24 bits) ; la recompression « sémantique » d'audio déjà lossy (MP3 via packMP3 : présent dans precomp) reste marginale et sans benchmark récent trouvé.

### Cited Findings
- FLAC : décodage le plus rapide, mode d'encodage le plus rapide, compression « within 3 % of even the most complex codecs ». — [xiph FLAC comparison](https://xiph.org/flac/comparison.pdf)
- WavPack ≈ **63 %** de la taille WAV, très proche de FLAC (test 2006), meilleur sur 24 bits ; test Hydrogenaudio 2022 : WavPack devance OptimFROG de > 10 % relatif (3 points) sur son jeu de test. — [bobulous 2006](https://www.bobulous.org.uk/misc/lossless_audio_2006.html) ; [Hydrogenaudio](https://hydrogenaudio.org/index.php?msg=812744)
- paq8px v217 sur RareWares audio : 103 829 383 o vs OptimFROG 103 431 728 o. — [GitHub paq8px](https://github.com/hxim/paq8px)

### Gaps
- Pas de benchmark 2024-2026 unifié ; chiffres dépendants du corpus et des presets.
