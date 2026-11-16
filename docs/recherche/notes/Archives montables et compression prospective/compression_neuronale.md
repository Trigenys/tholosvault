# Compression neuronale / apprise sans perte et frontière de recherche (2019-2026)

Notes de recherche, rédigées le 2026-11-16. Chiffres datés et contextualisés matériellement quand la source le permet ; les estimations sont marquées « (estimation) » ; la spéculation est isolée dans les sous-sections « Inferences ».

## Cadre théorique : Kolmogorov, MDL, « prédiction = compression »

### Takeaway
Toute la lignée (PAQ → cmix → NNCP → LLM) repose sur une seule identité : un prédicteur probabiliste + codage arithmétique = compresseur sans perte, et un bon compresseur approxime la complexité de Kolmogorov / l'induction de Solomonoff. La différence entre générations est le coût (taille du modèle, calcul) et la manière de l'amortir (modèle entraîné en ligne vs pré-entraîné partagé).

### Cited Findings
- Delétang et al. (DeepMind, 2023) formalisent l'équivalence prédiction–compression : « any compressor (e.g., gzip) can be used to build a conditional generative model » et les LLM sont vus comme des « strong general-purpose predictors » ; la vue compression éclaire scaling laws, tokenisation et in-context learning — [arXiv 2309.10668](https://arxiv.org/abs/2309.10668)
- Delétang et al. introduisent un « adjusted compression rate » : la taille des paramètres (2 octets/param) est ajoutée au flux compressé. Sur 1 Go, Chinchilla 70B passe de 8,3 % (raw) à 14 008,3 % (adjusted) sur enwik9 ; Llama 2 7B de 8,9 % à 1 408,9 % ; un Transformer 3,2 M params entraîné sur enwik8 atteint 17,0 % raw / 17,7 % adjusted. « A foundation model only achieves non-trivial adjusted rates on datasets in the TB range or larger » — [arXiv 2309.10668v2](https://arxiv.org/html/2309.10668v2)
- Même source : compromis modèle–données (« each dataset has an optimal model size ») ; la tokenisation agit comme une « lossless pre-compression » dont l'effet dépend de la taille du modèle ; contexte limité à 2048 octets par chunk — [arXiv 2309.10668v2](https://arxiv.org/html/2309.10668v2)
- LMCompress (Li et al., 2024-2025, Nature Machine Intelligence 2025) : les grands modèles génératifs sont traités comme « approximations of Solomonoff induction » et pilotent un codeur arithmétique ; thèse « the better a large model understands the data, the better LMCompress compresses » — [arXiv 2407.07723](https://arxiv.org/abs/2407.07723)
- Monographie de référence « An Introduction to Neural Data Compression » (Yang, Mandt, Theis, 2022/2023), chapitre lossless : modèles autorégressifs et à variables latentes — [arXiv 2202.06533](https://arxiv.org/pdf/2202.06533)
- Le Hutter Prize est explicitement motivé par le lien compression ↔ intelligence/induction : 5 000 € par point de pourcentage d'amélioration sur enwik9, 500 000 € de dotation totale — [Wikipedia Hutter Prize](https://en.wikipedia.org/wiki/Hutter_Prize)

### Inferences
- Le « adjusted rate » est le vrai arbitre économique : un modèle de 8B params (16 Go) ne se justifie que si le corpus à compresser est ≫ 16 Go, ou si le modèle est déjà présent des deux côtés (voir « modèle comme dictionnaire »).
- MDL (Minimum Description Length) explique la hiérarchie actuelle du LTCB : les gagnants sont des cmix (modèle appris en ligne, décompresseur de quelques Mo) et non des LLM pré-entraînés, car le LTCB compte la taille du décompresseur.

### Gaps
- Je n'ai pas trouvé de source 2024-2026 chiffrant explicitement le seuil « taille du corpus à partir duquel un LLM pré-entraîné bat cmix en adjusted rate » au-delà de l'ordre de grandeur « TB range » de DeepMind.

## Lignée context-mixing : paq8, paq8px, cmix — et état du LTCB / Hutter Prize (fin 2026)

### Takeaway
En novembre 2026, le haut du Large Text Compression Benchmark est entièrement occupé par des dérivés de cmix (CPU, ~10 Go RAM, 1-2 jours par Go), le meilleur relevé ≈ 89,3 Mo sur enwik9 (cmix-neif 2.0, 30 oct. 2026) ; NNCP v3.2 (GPU) à 107,3 Mo et cmix v21 à 108,2 Mo ont été dépassés en 2026 par une vague de variantes cmix+transformer. Le record Hutter Prize (contraintes temps/mémoire plus strictes) est fx2-cmix-T à 100 424 672 octets (juil. 2026).

### Cited Findings
- LTCB, entrées visibles (classées par enwik9 + décompresseur zippé) : cmix-neif 2.0 — 89 346 392 o. (archive auto-extractible, décompresseur compté 0), 177 048 ns/octet compression (≈ 49 h pour 1 Go), 9 770 Mo RAM, Google Cloud c2d, AMD EPYC 7B13 4 cœurs, soumis 30 oct. 2026 (soumission Hutter Prize en attente) ; cmix-views 1.0 — 90 515 901 o., ~210 000 ns/o., 14 005 Mo, Ryzen 9 9950X3D un cœur, 28 oct. 2026 ; cmix-wolf-p14 — 90 735 088 o., 125 201 ns/o., 27 oct. 2026 ; ex-girlfriend 1.0 — 91 778 237 o. ; fx2-cmix-transformer — 96 996 198 o., 133 766 ns/o., 24 juil. 2026 ; fractal-zip — 97 617 144 o. (enwik8 : 12 303 845) — [LTCB](https://www.mattmahoney.net/dc/text.html)
- LTCB : nncp v3.2 (Bellard, 23 oct. 2023) — enwik9 106 632 363 o., total 107 261 318 o., 241 871 ns/o. compression, 238 670 ns/o. décompression (≈ 67 h/Go), 7 600 Mo, Xeon E3-1230 v6 + RTX 3090, Transformer + préprocesseur dictionnaire — [LTCB](https://www.mattmahoney.net/dc/text.html)
- LTCB : cmix v21 (Byron Knoll, 17 sept. 2024) — enwik9 107 963 380 o., total 108 244 767 o., 622 949 ns/o. (≈ 173 h/Go), 30 950 Mo RAM, CPU ; paq8px_v206fix1 -12L — 124 696 410 o., 291 916 ns/o., 28 151 Mo, Core i9 12900KS — [LTCB](https://www.mattmahoney.net/dc/text.html)
- Références classiques LTCB : xz 5.2.1 — 197 331 816 o., 5 876 ns/o. compression, 20 ns/o. décompression (≈ 50 Mo/s), 6 000 Mo ; zstd 0.6.0 -22 --ultra — 215 674 670 o., 701 ns/o. compression, 2,2 ns/o. décompression (≈ 450 Mo/s) ; brotli -q 11 — 223 597 884 o., 3 400 / 5,9 ns/o. — [LTCB](https://www.mattmahoney.net/dc/text.html)
- Règles LTCB : classement sur « compressed size of enwik9 plus the size of a zip archive containing the decompresser » ; temps = min(process time sommé, real time) ; « Timing information… may vary widely depending on the test machine » ; Mahoney : « I have not verified results submitted by others » — [LTCB](https://www.mattmahoney.net/dc/text.html)
- Hutter Prize, palmarès enwik9 : fx2-cmix-T (Vladimir Ivanov, 24 juil. 2026) 100 424 672 o., 37 300 € ; cmix-obias (David Freelan, 19 juil. 2026) 108 521 870 o. ; cmix-lex (Marcouch & Orav, 26 juin 2026) 109 671 639 o. ; fx2-cmix (Orav & Knoll, 3 sept. 2024) 110 793 128 o. ; fx-cmix (Orav, 2 fév. 2024) 112 578 322 o. ; fast cmix (S. Kumar, 16 juil. 2023) 114 156 155 o. ; starlit (Margaritov, 31 mai 2021) 115 352 938 o. ; phda9v1.8 (Rhatushnyak, 4 juil. 2019) 116 673 681 o. (référence, sans prix) — [Wikipedia Hutter Prize](https://en.wikipedia.org/wiki/Hutter_Prize)
- Règles Hutter Prize (résumé Wikipedia) : améliorer ≥ 1 % le précédent gagnant (fichier + décompresseur Win32/Linux), 5 000 €/%, publication 30 jours pour vérification, code libre depuis 2017 ; contraintes de temps et de mémoire existent mais ne sont pas chiffrées sur la page Wikipedia — [Wikipedia Hutter Prize](https://en.wikipedia.org/wiki/Hutter_Prize)
- Nacrith (2026) résume le coût de cmix : « roughly 0.86 bpb on enwik9 but at extreme cost (16–64 GB RAM, 0.5–5 KB/s) » (estimation de l'auteur, non vérifiée indépendamment) — [arXiv 2602.19626](https://arxiv.org/html/2602.19626v2)

### Inferences
- Vitesses dérivées des ns/octet LTCB (estimation) : cmix-neif 2.0 ≈ 5,6 Ko/s ; nncp v3.2 ≈ 4,1 Ko/s (GPU RTX 3090) ; cmix v21 ≈ 1,6 Ko/s ; xz ≈ 170 Ko/s compression / 50 Mo/s décompression ; zstd -22 ≈ 1,4 Mo/s / 450 Mo/s. Écart ratio cmix vs zstd : 2,4× ; écart vitesse de décompression : ~10⁵.
- Les variantes 2026 (« cmix-*-transformer », « lex-transformer ») indiquent que l'intégration d'un petit transformer entraîné en ligne dans cmix est devenue le vecteur de progrès — le LLM pré-entraîné reste exclu par la règle « décompresseur compté ».
- L'écart entre LTCB (89,3 Mo) et Hutter Prize (100,4 Mo) mesure le prix des contraintes de ressources du Hutter Prize.

### Gaps
- Les rangs 1-3 du LTCB n'étaient pas présents dans le texte récupéré (la table scrappée commence au rang 4) ; il peut exister des entrées < 89,3 Mo non relevées ici. À vérifier sur la page.
- Les constantes précises du Hutter Prize (heures, Go RAM, machine de test) n'ont pu être extraites : la page prize.hutter1.net renvoie un certificat TLS expiré.
- ts_zip n'apparaît pas dans la portion lue du LTCB.

## NNCP (Bellard), ts_zip (Bellard), LLMZip, LMCompress, Nacrith : ratios, vitesses, matériel

### Takeaway
Les compresseurs à LLM pré-entraîné atteignent 0,66-1,1 bpb sur enwik9 (vs 1,7 pour xz, ~0,85 pour nncp/cmix) ; le meilleur ratio brut (Chinchilla 70B, 0,664 bpb) exige un modèle de 140 Go. ts_zip est le seul outil « utilisable » : ~1 Mo/s sur RTX 4090, 1,084 bpb sur enwik9 (135,4 Mo), déterministe entre machines. Tout le reste tourne de quelques dizaines de tokens/s à quelques Ko/s.

### Cited Findings
- NNCP : enwik8 14 915 298 o. (1,19 bpb), enwik9 106 632 363 o. (0,853 bpb), décompresseur zippé 628 955 o., total 107 261 318 o. (résultats datés 2023-10-21) ; v3.3 (2024-06-05) Linux/Windows, C + LibNC, CUDA ; v2 (2021) Python/PyTorch, GPU requis ; la page ne donne ni vitesse ni discussion du déterminisme — [bellard.org/nncp](https://bellard.org/nncp/)
- ts_zip : modèle RWKV 169M v4, quantifié 8 bits/paramètre, évalué en BF16 ; enwik8 13 825 741 o. (1,106 bpb) vs xz 24 865 244 (1,989) ; enwik9 135 443 237 o. (1,084 bpb) vs xz 213 370 900 (1,707) ; alice29.txt 21 713 o. (1,142) ; book1 137 477 o. (1,431) ; linux-1.2.13.tar 1 196 859 o. (1,021) vs xz 1 689 468 ; vitesse « up to 1 MB/s » compression et décompression sur RTX 4090 ; ≥ 4 Go RAM ; GPU nécessaire ; « supports only text. Binary files compress poorly » ; fichiers datés 2024-03-02 — [bellard.org/ts_zip](https://bellard.org/ts_zip/)
- ts_zip, déterminisme : « the model is evaluated in a deterministic, reproducible way » ; les résultats « do not depend on the CPU or GPU model or the thread count » ; un fichier compressé sur une configuration se décompresse sur une autre — [bellard.org/ts_zip](https://bellard.org/ts_zip/)
- LLMZip (Valmeekam et al., juin 2023) : LLaMA-7B + codage arithmétique, 0,7101 bits/caractère sur 1 Mo de text8 (borne entropique estimée 0,709), vs zpaq 1,4 et paq8h 1,2 ; 0,8426 b/c sur 100 Ko de Gutenberg ; les auteurs n'ont pas pu traiter les 100 Mo complets et avertissent que text8 (dérivé de Wikipedia) rend les résultats « optimistic » — [arXiv 2306.04050](https://arxiv.org/pdf/2306.04050)
- Delétang 2023 (raw, chunks 2048 o., 1 Go) : enwik9 Chinchilla 70B 8,3 %, Chinchilla 7B 10,2 %, Llama 2 7B 8,9 %, Chinchilla 1B 11,3 % ; LZMA2 23,0 %, gzip 32,3 % (non chunké). ImageNet : Chinchilla 70B 48,0 % vs PNG 58,5 % ; LibriSpeech : 21,0 % vs FLAC 30,9 %. Données aléatoires : 100,8 % (70B), 103,2 % (Llama 2 7B), i.e. expansion — [arXiv 2309.10668v2](https://arxiv.org/html/2309.10668v2)
- LMCompress (2024-2025) : texte via LLaMA3-8B + LoRA de domaine (MeDAL médical 1 104 Mo ; Pile of Law eurlex) : ratio « almost triple the best traditional methods », +8,5 % (MeDAL) et +38,4 % (eurlex) vs LLaMA3-8B brut ; images via iGPT, 197 images (128 Mo) : CLIC2019 6,32× vs JPEG-XL 2,93× / PNG 2,20× ; ILSVRC 4,79× vs WebP 2,04× / Chinchilla 70B 2,08× ; audio LibriSpeech 6,07× vs FLAC 3,23× / Chinchilla 70B 4,76× ; vidéo lossless frame par frame (sans inter-frame) +20 % (statique) à +50 % (dynamique). Aucun temps, débit ni GPU rapporté ; déploiement évoqué en spéculation (liens 6G, modèles aux deux bouts) — [arXiv 2407.07723v3](https://arxiv.org/html/2407.07723v3)
- Revue indépendante de LMCompress (A. Wegner, août 2025, rédigée avec aide ChatGPT) : coûts temporels « somewhat high » hors audio, « far from being ready for deployment » — [buttondown.com/anatol](https://buttondown.com/anatol/archive/lmcompress-lossless-data-compression/)
- Nacrith (R. Tacconelli, chercheur indépendant, 24 fév. 2026) : SmolLM2-135M (~500 Mo GGUF F32, 1,2 Go VRAM/worker) + N-grammes ordre 1-4 + mixeur adaptatif + « confidence skip » + codeur arithmétique 32 bits à CDF 2^24 ; enwik8 0,9389 bpb (11,74 %) vs cmix v21 1,17, ts_zip ≈ 1,11, FineZip (8B) 1,024, xz 1,989, zstd -19 2,156 ; alice29 0,918 bpb vs cmix 1,63 ; pas de résultat enwik9 ; débit 50-70 tokens/s en début de fichier puis 20-30 tokens/s sur GTX 1050 Ti (≈ 21 tokens/s cité en limite) ; déterminisme : LLM en FP32, têtes en float64, CDF entière, mais « bit-exact reproduction requires the same GPU architecture, BLAS library, and driver version » ; contamination possible des jeux de test par l'entraînement — [arXiv 2602.19626v2](https://arxiv.org/html/2602.19626v2)
- Benchmark NNLCB (Sun et al., 28 jeux de données ≈ 8,9 Go, 4× RTX 4090, 4× Xeon Silver 4310) : NNCP meilleur ratio pondéré (économie de stockage 47,7 %), temps total ≈ 943 h de compression ; DZip 10,1 Go RAM, DeepZip 13,7 Go ; PAC et TRACE meilleur compromis (70-130 h) ; cmix et LLMZip listés mais sans résultats — [NNLCB](http://huisun.top/NNLCB)
- Débits NNLCB (estimation par division des temps totaux par 8,9 Go) : NNCP ≈ 2,6 Ko/s ; lstm-compress ≈ 5 Ko/s ; DZip ≈ 7,4 / 16,6 Ko/s (comp./décomp.) ; DeepZip ≈ 9,9 / 47 Ko/s ; PAC ≈ 33 / 21 Ko/s ; TRACE ≈ 36 / 19 Ko/s ; gzip ≈ 5,5 Mo/s / 95 Mo/s — [NNLCB](http://huisun.top/NNLCB)

### Inferences
- Hiérarchie ratios enwik9 fin 2026 (bpb) : Chinchilla 70B 0,664 (raw, 140 Go de poids) < cmix-neif 2.0 ≈ 0,715 < nncp v3.2 0,853 ≈ cmix v21 0,864 < ts_zip 1,084 < xz 1,707 < zstd -22 1,725. Seul ts_zip dépasse 100 Ko/s.
- ts_zip montre qu'un modèle de 169 M params quantifié peut être rendu bit-exact multi-plateforme ; Nacrith montre l'inverse (llama.cpp en FP32 non portable) : la portabilité est une propriété de l'implémentation, pas du modèle.
- Le ratio ts_zip sur linux-1.2.13.tar (1,021 bpb vs 1,441 xz) est le seul chiffre trouvé d'un LLM sur du contenu « source + binaire » ; il reste du texte à 90 %.

### Gaps
- Aucune vitesse publiée pour NNCP v3.x par Bellard ; seules les ns/octet LTCB (≈ 4 Ko/s sur RTX 3090) donnent une estimation.
- LMCompress ne publie ni débit ni GPU : impossible de chiffrer le coût ; l'ordre de grandeur « 8B params, 2048 tokens de contexte » suggère quelques Ko/s sur GPU grand public (estimation, non sourcée).
- Aucun résultat enwik9 pour Nacrith, LLMZip, LMCompress texte (ils travaillent sur des sous-ensembles de 1-10 Mo).

## Le problème du déterminisme (« GPU Butterfly Effect ») et les réponses des papiers

### Takeaway
Le codage arithmétique exige des probabilités bit-identiques à l'encodeur et au décodeur ; la non-associativité du flottant (parallélisme GPU, kernels, batch, fonctions transcendantes) produit des dérives ~10⁻⁷ suffisantes pour désynchroniser le décodeur dès le 12e token entre deux GPU différents. Trois familles de réponses : (1) pile d'inférence appariée + quantification entière des CDF (ts_zip, Nacrith, cmix historique) ; (2) quantification des logits avant softmax en float64 côté hôte (Hybrid-LLM 2026) ; (3) codes tolérants au mismatch (PMATIC 2026, et une variante sans codage arithmétique), validés uniquement sur bruit simulé.

### Cited Findings
- Armstrong et al. (U. Houston, 26 mars 2026) nomment le « GPU Butterfly Effect » : opérations flottantes non associatives → l'inférence parallèle (encodage) donne des probabilités légèrement différentes de l'inférence série (décodage) ; dérives ~10⁻⁷ font choisir la mauvaise branche ; exp et autres transcendantes non bit-exactes entre architectures — [arXiv 2603.25526v1](https://arxiv.org/html/2603.25526v1)
- Même source, test croisé : compression sur NVIDIA L4, décompression sur RTX 3090 ; sans quantification « decoding failed at token 12 » ; avec logits arrondis à 3 décimales sur GPU puis softmax/CDF en float64 sur l'hôte : zéro erreur binaire ; pénalité < 0,002 bpc (mesurée seulement sur Alice) — [arXiv 2603.25526v1](https://arxiv.org/html/2603.25526v1)
- Une revue note que les premiers travaux LLM ont tourné encodeur et décodeur sur une même machine, « where cross-run non-determinism is very small, so the issue went unnoticed » ; les configurations multi-machines l'exposent — [themoonlight.io review](https://www.themoonlight.io/review/synchronizing-probabilities-in-model-driven-lossless-compression)
- PMATIC (Adler & Tang, janv. 2026) : modélise un mismatch borné ε sur les logits (L∞) → distance en variation totale ≤ ε/2 ; chaque bit de token a sa probabilité quantifiée en bins de rayon r et un « helper bit » bas-entropie indique centre ou frontière ; surcoût total O(√(δ log 1/δ)) ; expériences : Llama 3.1 8B quantifié, 10 Mo d'enwik8 en fichiers de 5 Ko, mismatch simulé (bruit uniforme) ; enwik8 : 2,38 bits/token sans mismatch, 2,59 (δ=1e-5) et 4,13 (δ=1e-3) avec PMATIC, gzip 14,11 ; « validation uses synthetic mismatch, not real LLM non-determinism » ; vitesse non quantifiée — [arXiv 2601.10678v1](https://arxiv.org/html/2601.10678v1)
- Un second papier (arXiv 2601.17684) propose une méthode robuste à des mismatches « arbitrarily large, but structured », sans codage arithmétique, testée avec Llama 3.1 et « significantly outperforms gzip » — [arXiv 2601.17684](https://web3.arxiv.org/pdf/2601.17684)
- « Haiku to Opus in Just 10 bits » (Rinberg, Carrell, Henniger, Carlini, Warr, avr. 2026) : « assume encoder and decoder produce identical model outputs… not guaranteed in practice », robustesse au non-déterminisme traitée comme problème ouvert ; pistes : piles appariées + seeds fixes, canal unidirectionnel, auto-compression « PMATIC-style » (~40× sur transcripts QA) — [alphaxiv 2604.02343](https://www.alphaxiv.org/abs/2604.02343.md)
- ts_zip : résultats indépendants du CPU/GPU et du nombre de threads (modèle 8 bits, évaluation BF16 déterministe) — [bellard.org/ts_zip](https://bellard.org/ts_zip/)
- Nacrith : LLM en FP32 pour des distributions « consistent », CDF entières 2^24, mais reproduction bit-exacte conditionnée à la même architecture GPU / BLAS / driver — [arXiv 2602.19626v2](https://arxiv.org/html/2602.19626v2)
- JPEG AI (lossy) contourne le problème par la norme : conformité bit-exacte exigée seulement jusqu'au décodage entropique, laissant la synthèse neuronale libre d'être approximée — [streaminglearningcenter.com](https://streaminglearningcenter.com/codecs/jpeg-ai-is-coming-what-you-need-to-know.html)

### Inferences
- Deux philosophies : « rendre le modèle déterministe » (coûte en vitesse : FP32, kernels séquentiels, pas de batch ; ts_zip prouve que c'est faisable avec un petit modèle quantifié) vs « rendre le code tolérant » (coûte en bits : +0,2 à +2 bits/token chez PMATIC, soit +8 % à +75 % sur enwik8). Aucune des deux n'a encore été démontrée à l'échelle d'un 8B sur matériel hétérogène en conditions réelles.
- Le déterminisme est l'obstacle n°1 à un « service de compression » distribué : sans lui, l'archive est liée à un triplet (GPU, driver, BLAS), ce qui est pire que n'importe quel format propriétaire pour l'archivage.

### Gaps
- Aucune mesure publiée de la fréquence réelle de désynchronisation entre familles de GPU (NVIDIA/AMD/NPU Apple) pour un même modèle : les papiers 2026 utilisent soit un seul couple (L4/3090), soit du bruit simulé.
- Pas de source trouvée sur l'usage d'inférence entière pure (int8 bout en bout, sans softmax flottant) pour la compression sans perte au-delà de la mention « quantized to 8 bits » de ts_zip.

## Modèles rapides à entraînement en ligne (DZip, TRACE, PAC) et « learned entropy coding » pour données binaires

### Takeaway
Les compresseurs neuronaux « adaptatifs » (entraînés en ligne, sans poids pré-entraînés à transporter) plafonnent à 20-40 Ko/s sur GPU et gagnent 5-10 % sur DZip ; sur Silesia (dont les exécutables mozilla), TRACE est même légèrement moins bon que DZip. Aucune publication trouvée ne traite spécifiquement d'exécutables ou d'assets de jeux en sans-perte neuronal.

### Cited Findings
- DZip (Goyal et al., 2019) : réseau neuronal + codage arithmétique, « novel hybrid training approach », comparé à gzip, BSC, 7zip, ZPAQ et aux compresseurs RNN ; le corpus inclut Silesia « mozilla 51.2M… Tarred executables of Mozilla 1.0 » — [arXiv 1911.03572](https://arxiv.org/pdf/1911.03572)
- TRACE (WWW 2022) : transformer à une couche ; 952,3 ko/min en moyenne (≈ 16 Ko/s), « 3x improvement over Dzip » ; ratios moyens 2,16:1 (son), 1,81:1 (image), 1,28:1 (flottants) vs DZip 2,04 / 1,72 / 1,26 ; sur Silesia 203 Mo « slightly worse than Dzip… because the transformer's complex structure leads to slow convergence » — [arXiv 2203.16114](https://arxiv.org/pdf/2203.16114)
- Survey FCS 2025 (Sun, Ma, Ling et al., Frontiers of Computer Science 19:197360, 21 mars 2025) : taxonomie en trois types — static pre-training, adaptive, semi-adaptive — ; 19 métriques unifiées ; constat que « the research survey for the NN-based universal lossless compressors has not been conducted yet » ; communiqué : le goulot de performance « has been overcome thanks to hardware accelerators » — [FCS](https://journal.hep.com.cn/fcs/EN/10.1007/s11704-024-40300-5) ; [EurekAlert](https://www.eurekalert.org/news-releases/1111971)
- NNLCB (même équipe) : PAC ≈ 33 Ko/s, TRACE ≈ 36 Ko/s compression (estimation), 6,1 Go RAM chacun ; DeepZip* (taille modèle comptée) peut dilater certaines données — [NNLCB](http://huisun.top/NNLCB)
- Hybrid-LLM 2026 (Houston) utilise un « Scout » zstd -1 pour router : blocs à ratio ≤ 1,05 (quasi-aléatoire) ou > 3,0 (très redondant) → zstd -19 sur CPU ; seule la plage intermédiaire va au LLM ; sur kennedy.xls et ptt5 le routage vers zstd donne de meilleurs ratios que LLMZip (15,88× vs 12,41× ; 11,76× vs 7,75×) en < 1 s — [arXiv 2603.25526v1](https://arxiv.org/html/2603.25526v1)
- Textures de jeu (lossy, hors sujet strict) : NVIDIA Neural Texture Compression, démo GTC 2026, VRAM 6,5 Go → 970 Mo, mode « On Sample » ≈ −30 % de performance ; Intel TSNC > 9× (variante A) et > 18× (variante B) vs ≈ 4,8× pour BC ; AMD Neural Texture Block Compression vise la taille de téléchargement/installation sans changer l'exécution runtime ; critique : NTC « replacing one lossy method with another » (BC7 déjà lossy) — [guru3d](https://www.guru3d.com/story/nvidia-neural-texture-compression-slashes-vram-use-in-gtc-2026-demo/) ; [itbrief](https://itbrief.co.uk/story/intel-releases-neural-texture-compression-sdk-for-game-devs) ; [tweaktown](https://www.tweaktown.com/news/99029/amd-is-introducing-new-ai-texture-compression-for-gaming-to-reduce-download-and-install-sizes/index.html)
- ts_zip : « Binary files compress poorly » (déclaration de l'auteur) — [bellard.org/ts_zip](https://bellard.org/ts_zip/)

### Inferences
- Pour les binaires, l'état de l'art reste le context-mixing classique (paq8px avec ses modèles x86/exe dédiés, cmix) : les réseaux pré-entraînés sur texte n'ont aucun avantage, et les réseaux entraînés en ligne convergent trop lentement sur des données hétérogènes (constat TRACE/Silesia).
- Le « learned entropy coding » qui marche vraiment sur binaires en production est lossy et domaine-spécifique (textures NTC/TSNC) : il remplace le format, pas le compresseur générique.

### Gaps
- Aucun papier trouvé (2019-2026) évaluant un compresseur neuronal sans perte sur un corpus d'exécutables / assets de jeux (PAK, meshes, shaders) ; Silesia « mozilla » est le seul proxy, sans résultat détaillé extrait.
- Je n'ai pas pu lire les détails de FADE (ACL 2026, « splits probability modeling (GPU) from entropy coding (CPU) », figure ratio vs throughput) : PDF non extractible — [aclanthology 2026.acl-long.324](https://aclanthology.org/2026.acl-long.324.pdf)

## Hybrides LZ + modèle appris à > 100 Mo/s, et « modèle comme dictionnaire » / compression en service

### Takeaway
Aucun système publié ne combine LZ pour le volume et un petit modèle appris pour le résidu à > 100 Mo/s : le plus proche est OpenZL (Meta, 2026) où un petit réseau choisit les codecs (467 Mo/s compression, 3 Go/s décompression, ratio égal à zstd) — l'apprentissage sert à la sélection, pas à la modélisation. L'idée « modèle partagé = dictionnaire » existe en théorie (DeepMind, LMCompress, « Haiku to Opus ») et en pratique chez Bellard (ts_zip / ts_sms), mais aucune offre de service avec poids cachés partagés n'a été trouvée.

### Cited Findings
- OpenZL v0.3 (Meta) : décompression 3 062 Mo/s vs 1 254 Mo/s pour zstd, ratio 2,73 vs 2,74, compression 467 Mo/s vs 419 Mo/s (zstd au réglage le plus rapide) ; « a small neural network scores the candidate codecs for each stream, one decision at a time » — [newreleases.io OpenZL 0.3.0](https://newreleases.io/project/github/facebook/openzl/release/v0.3.0)
- Ordres de grandeur zstd : « typical usage compresses on the order of 100 MB/s and decompresses on the order of 1 GB/s » — [résultat de recherche, hwbusters](https://hwbusters.com/?p=117728)
- Hybrid-LLM 2026 : inférence « about 2600× slower than Zstd », limite l'usage à l'archivage « ultra-cold » ; tables de probabilités ≈ 50 Mo par bloc ; déploiement Cloud Run 10 workers ≈ 12 min dont 11,5 min de téléchargement du modèle (cold start) ; Alice (151 Ko) 20,5× en 22 min sur RTX 3090 vs LLMZip 9,5×, ZPAQ 5,7×, gzip 3,0× — [arXiv 2603.25526v1](https://arxiv.org/html/2603.25526v1)
- « Haiku to Opus in Just 10 bits » (avr. 2026) : le récepteur exécute localement le même petit modèle ; seuls les bits de réponse oui/non du grand modèle transitent (~1 bit/question, ≤ 10 questions) ; ratios 0,0006-0,004 (« over 100× better than prior LLM-based lossless methods », mais lossy sur la forme) ; en lossless, codage arithmétique sur LMSYS-Chat : gzip ≈ 0,77, modèle de base ≈ 0,18, LoRA de domaine ≈ 0,09 (métrique normalisée par token) ; routeur d'adaptateurs LoRA ~55 % de précision sur prompt seul, conservant ~1,9× de gain — [alphaxiv 2604.02343](https://www.alphaxiv.org/abs/2604.02343.md)
- LMCompress conclusion : usage envisagé sur liens 6G/satellite avec « large models at both ends », compression avant chiffrement, diffusion décodable seulement par les modèles appariés — spéculatif selon le papier lui-même — [arXiv 2407.07723v3](https://arxiv.org/html/2407.07723v3)
- Discussions publiques : « both encoder and decoder need access to the same model weights, which can be gigabytes, and that is the main cost » ; analogie du modèle comme « shared lookup table » — [lemmy.world](https://lemmy.world/post/29678360) ; [hackaday 2024](https://hackaday.com/2024/04/28/ai-can-now-compress-text)
- ts_zip renvoie vers ts_sms pour les petits messages (compression de SMS avec le modèle partagé) — [bellard.org/ts_zip](https://bellard.org/ts_zip/)
- « Lossless prompt compression via dictionary-encoding » (avr. 2026) : méta-tokens + dictionnaire dans le prompt système, −18 à −27 % de tokens, −33 à −47 % de calcul — c'est de la compression pour LLM, pas par LLM — [alphaxiv 2604.13066](https://www.alphaxiv.org/abs/2604.13066.md)
- Génomique (domaine où le « modèle appris » avance le plus vite) : DeepGeCo (AAAI 2025) jusqu'à 22,9× de débit et −31 % de taille vs ses baselines ; PMKLC (juil. 2025) encodeur (s,k)-mer GPU + partition en blocs, ×3,0 (1 GPU) à ×10,7 (multi-GPU) de débit ; AgentGC (janv. 2026) ; MFCompress classique ≈ 10 Mo/s cité comme référence, les méthodes apprises restant en Ko/s — [arXiv 2507.12805](https://arxiv.org/pdf/2507.12805) ; [AAAI 2025 DeepGeCo](https://ojs.aaai.org/index.php/AAAI/article/view/33371) ; [arXiv 2601.13559](https://arxiv.org/pdf/2601.13559)

### Inferences
- Le « modèle comme dictionnaire » est économiquement identique au dictionnaire zstd partagé (RFC 8878) ou Brotli (dictionnaire statique 120 Ko) : il n'est rentable que si (a) le modèle est déjà présent (OS, navigateur, GPU) et (b) versionné/figé pour toujours. Le point (b) est incompatible avec le rythme actuel des modèles (ts_zip : « compatibility between versions is not guaranteed »).
- Un hybride LZ + petit modèle à > 100 Mo/s n'existe pas en publication ; le chemin plausible (estimation) est celui d'OpenZL : apprentissage hors ligne pour la sélection/paramétrage de codecs classiques, puis entropie classique (FSE/rANS) — zéro inférence au décodage.

### Gaps
- Aucune source sur un service commercial de compression LLM à poids cachés partagés (ni cloud, ni CDN) ; le seul « service » trouvé est un prototype Cloud Run académique.
- Pas de chiffre trouvé pour FADE ni pour un quelconque compresseur appris dépassant ~1 Mo/s en texte (ts_zip) ; le mur des 10 Mo/s n'est franchi par aucun système lu.

## Images : JPEG XL (parties « apprises ») et JPEG AI 2025 — le fossé pratique

### Takeaway
JPEG XL (sans perte, mode modulaire) utilise un prédicteur « appris » au sens classique : un arbre MA par image choisissant prédicteur et contexte par pixel, plus un prédicteur pondéré auto-adaptatif — sans réseau de neurones ni GPU. JPEG AI (ISO/IEC 6048-1:2025, lossy) est le premier codec image normalisé bout-en-bout neuronal, pensé pour NPU mobiles, avec conformité bit-exacte seulement jusqu'au décodage entropique ; aucune mesure de débit publique n'a été trouvée.

### Cited Findings
- JPEG XL modular : « Predictors can be changed on a per-pixel basis… A decision tree (called the MA tree, for 'meta-adaptive') selects which one to use », signalé dans le flux ; prédicteur pondéré « consists of four foundational sub-predictors », efficace sur photos ; niveaux d'effort = compromis taille/temps ; aucun chiffre Mo/s dans les sources lues — [Cloudinary](https://cloudinary.com/blog/jpeg-xls-modular-mode-explained) ; [arXiv 2506.05987](https://arxiv.org/pdf/2506.05987)
- JPEG AI : « first international image coding standard based on an end-to-end learning-based approach » ; v1 = ITU-T T.840.1 / ISO/IEC 6048-1:2025 (sept. 2025) ; Partie 2 (profils) datée 2026 ; points de fonctionnement simplified/base limités à ReLU/ReLU6/convolution ; démo décodage 4K sur Huawei Mate50 Pro (Snapdragon 8+ Gen1) sans temps publiés — [jpeg.org/jpegai](https://jpeg.org/jpegai/) ; [ISO 6048-1](https://standards.iteh.ai/catalog/standards/iso/35a56bff-ccdf-42de-bc64-9f3e40015e18/iso-iec-6048-1-2025) ; [streaminglearningcenter](https://streaminglearningcenter.com/codecs/jpeg-ai-is-coming-what-you-need-to-know.html)
- LMCompress sur images sans perte : 6,32× (CLIC2019) vs JPEG-XL 2,93× — mais via iGPT par canal RVB sur segments de 1 024 pixels, sans débit publié — [arXiv 2407.07723v3](https://arxiv.org/html/2407.07723v3)

### Inferences
- Le fossé est de 4 à 6 ordres de grandeur : xz décompresse à ~50 Mo/s, zstd à ~450 Mo/s-3 Go/s ; les compresseurs neuronaux sans perte vont de 2 Ko/s (NNCP) à 1 Mo/s (ts_zip, RTX 4090). Un GPU de 400 W pour 1 Mo/s représente ~0,1 kWh/Go (estimation) contre quelques J/Go pour zstd.
- JPEG AI, lossy, peut tolérer une synthèse non bit-exacte ; un codec neuronal sans perte ne le peut pas — c'est pourquoi le sans-perte neuronal normalisé n'existe pas en 2026.

### Gaps
- Aucune mesure de temps de décodage JPEG AI (ms/image, par NPU) trouvée ; la synthèse arXiv 2510.13867 n'a pas été lue.
- Aucun débit Mo/s de libjxl par niveau d'effort dans les sources consultées.

## Où les chercheurs placent eux-mêmes les lacunes (2024-2026)

### Takeaway
Les auteurs convergent sur cinq lacunes : débit (Ko/s), coût d'inférence/énergie, non-déterminisme inter-matériel non validé en conditions réelles, contexte court (2 048 tokens/octets) fragmentant la prédiction, et contamination des benchmarks (enwik8/Alice présents dans les corpus d'entraînement).

### Cited Findings
- Hybrid-LLM 2026 : latence 2 600× zstd ; bande passante des tables de probabilité (~50 Mo/bloc) ; « context fragmentation » aux frontières de blocs ; économie du stockage froid estimée sans mesure d'énergie ; biais linguistiques possibles — [arXiv 2603.25526v1](https://arxiv.org/html/2603.25526v1)
- PMATIC : bornes dures sur les logits non réalistes, bruit synthétique, texte seul, un seul modèle, « fundamental limits on compression cost under mismatch are unknown », runtime non quantifié — [arXiv 2601.10678v1](https://arxiv.org/html/2601.10678v1)
- Nacrith : débit ~21 tokens/s, modèle 500 Mo aux deux bouts, contamination possible des jeux de test, contexte 2 048, anglais seulement, résultats auto-rapportés d'un auteur unique — [arXiv 2602.19626v2](https://arxiv.org/html/2602.19626v2)
- LLMZip : impossible de traiter 100 Mo avec LLaMA-7B ; résultats text8 « likely… optimistic » (Wikipedia dans l'entraînement) — [arXiv 2306.04050](https://arxiv.org/pdf/2306.04050)
- Delétang : adjusted rate non trivial seulement au-delà du To ; contexte 2 048 octets — [arXiv 2309.10668v2](https://arxiv.org/html/2309.10668v2)
- « Haiku to Opus » : robustesse au non-déterminisme « open problem » ; pas de mesure de latence ; régressions quand le grand modèle se trompe — [alphaxiv 2604.02343](https://www.alphaxiv.org/abs/2604.02343.md)
- Survey FCS 2025 : absence de survey antérieur ; nécessité de 19 métriques unifiées (ratio seul insuffisant) — [FCS](https://journal.hep.com.cn/fcs/EN/10.1007/s11704-024-40300-5)
- Génomique : « existing learned lossless compressors have low compression and decompression throughput » (PMKLC) — [arXiv 2507.12805](https://arxiv.org/pdf/2507.12805)
- LTCB lui-même : « I have not verified results submitted by others » ; temps « may vary widely » selon la machine — [LTCB](https://www.mattmahoney.net/dc/text.html)

### Inferences
- Le champ manque d'un benchmark à la fois (a) multi-matériel pour le déterminisme, (b) comptant la taille du modèle, (c) rapportant Ko/s et J/Go. NNLCB couvre (b) partiellement, LTCB couvre (b), personne ne couvre (a) et (c).

### Gaps
- Les sections « open challenges » du survey FCS 2025 n'ont pas pu être lues (accès Springer bloqué, mirroir HEP en erreur réseau) ; les lacunes ci-dessus proviennent des papiers primaires.

## Spéculative decoding, distillation, petits modèles pour la compression

### Takeaway
La « spéculation » appliquée à la compression prend deux formes documentées : (1) un petit modèle draft qui saute le grand modèle sur les tokens prévisibles (Nacrith « confidence skip » ; routage zstd/LLM d'Hybrid-LLM) ; (2) un petit modèle partagé des deux côtés, le grand modèle n'envoyant que des bits de correction (« Haiku to Opus »). La distillation vers de petits modèles dédiés (SmolLM2-135M, RWKV-169M) est de facto la voie pratique, pas un sujet de recherche nommé.

### Cited Findings
- Nacrith : « confidence-based skip that bypasses the LLM on predictable tokens », N-grammes + mixeur adaptatif devant le transformer 135M — [arXiv 2602.19626v2](https://arxiv.org/html/2602.19626v2)
- Hybrid-LLM : Scout zstd -1 → seuls les blocs de compressibilité intermédiaire vont au LLM — [arXiv 2603.25526v1](https://arxiv.org/html/2603.25526v1)
- « Haiku to Opus » : petit modèle génère déterministement ses questions, grand modèle répond oui/non ; « the questions don't even need to be sent, since the large model can simulate the small one » ; récupère 23-72 % de l'écart petit/grand sur tâches faciles, 7-38 % sur difficiles — [alphaxiv 2604.02343](https://www.alphaxiv.org/abs/2604.02343.md)
- ts_zip : RWKV 169M 8 bits = petit modèle spécialisé rendu déterministe, 1 Mo/s — [bellard.org/ts_zip](https://bellard.org/ts_zip/)
- Delétang : les petits transformers (200K-3,2M) entraînés sur enwik8 battent LZMA2 sur enwik9 (17,0 % vs 23,0 %) avec un adjusted rate quasi identique (17,7 %), mais s'effondrent hors domaine (ImageNet 215,8 %) — [arXiv 2309.10668v2](https://arxiv.org/html/2309.10668v2)

### Inferences
- La courbe utile n'est pas « plus gros modèle = mieux » mais « modèle de 10⁸ params, quantifié, domaine-spécifique, déterministe » : c'est exactement le point de ts_zip et Nacrith, et le seul qui approche l'usage réel.

### Gaps
- Aucun papier trouvé intitulé/consacré à « speculative decoding for lossless compression » ; aucun travail trouvé distillant explicitement cmix ou un LLM vers un modèle entier/fixed-point pour la compression.

## Prospective 2027-2030 : où la compression neuronale devient pratique, où elle ne gagnera jamais

### Takeaway
Les faits établis (ratios 1,5-2,5× meilleurs que xz sur texte ; vitesses Ko/s à 1 Mo/s ; non-déterminisme inter-GPU ; expansion sur l'aléatoire) permettent une prospective sobre : gains plausibles sur texte/logs archivés à froid, génomique et télémétrie structurée via petits modèles déterministes et domaine-spécifiques ; aucun gain possible sur médias déjà compressés et données aléatoires, et pas de remplacement de zstd/LZ4 sur le chemin chaud avant 2030.

### Cited Findings
- Données aléatoires : Chinchilla 70B 100,8 %, Llama 2 7B 103,2 %, Transformer 3,2M 224 % — expansion — [arXiv 2309.10668v2](https://arxiv.org/html/2309.10668v2)
- Hybrid-LLM route les blocs ≤ 1,05 de ratio (déjà compressés/aléatoires) vers zstd et se positionne « ultra-cold » — [arXiv 2603.25526v1](https://arxiv.org/html/2603.25526v1)
- Domaines à gain mesuré : texte médical/juridique (LoRA, ×3 vs classique), audio brut (6,07× vs FLAC 3,23×), images brutes (6,32× vs JPEG-XL 2,93×) — [arXiv 2407.07723v3](https://arxiv.org/html/2407.07723v3) ; génomique (DeepGeCo, PMKLC, AgentGC 2025-2026) — [arXiv 2507.12805](https://arxiv.org/pdf/2507.12805)
- Le seul outil au Mo/s : ts_zip, 1 Mo/s sur RTX 4090 (2024) — [bellard.org/ts_zip](https://bellard.org/ts_zip/)
- OpenZL : apprentissage pour choisir des codecs classiques, 3 Go/s en décompression — [OpenZL 0.3.0](https://newreleases.io/project/github/facebook/openzl/release/v0.3.0)
- Le coût mémoire des approches context-mixing reste 10-30 Go pour 1 Go de texte (cmix v21 : 30 950 Mo ; cmix-neif : 9 770 Mo) — [LTCB](https://www.mattmahoney.net/dc/text.html)

### Inferences (spéculation explicite, 2027-2030)
- **Texte archivé à froid (Wikipedia-like, docs, mails)** : probable premier marché. Un modèle 100-200 M params, int8, déterministe (modèle ts_zip), ~1-5 Mo/s sur GPU/NPU grand public, ≈ 1,0 bpb vs 1,7 pour xz : −40 % de stockage. Condition : un format qui embarque un identifiant de modèle figé et une procédure de déterminisme (CDF entières). Horizon : 2027-2028 pour des outils, pas pour des standards.
- **Logs / télémétrie** : fort potentiel de ratio (texte très régulier, vocabulaire fermé), mais le gain marginal sur zstd + dictionnaire + colonne (ClickHouse, Parquet) est plus faible qu'on ne le pense ; les approches « apprises » qui s'imposeront d'abord sont de type OpenZL (sélection/paramétrage de codecs), pas de l'inférence au décodage. Horizon : 2027-2029.
- **Génomique** : le domaine le plus avancé en recherche (séquences, vocabulaire de 4 lettres, modèles dédiés) ; des compresseurs appris à 10-100 Mo/s multi-GPU sont plausibles d'ici 2028-2030 pour les pipelines centralisés ; pas de déploiement sur poste.
- **Images/vidéo sans perte** : les ratios LMCompress (2× JPEG-XL) sont réels mais à des débits inconnus et probablement en Ko/s ; l'avenir neuronal des images est lossy (JPEG AI) avec NPU. Pas de sans-perte neuronal normalisé avant 2030.
- **Ne gagnera jamais** : données déjà compressées (JPEG, MP4, ZIP, chiffré), aléatoires ou à haute entropie — borne de Shannon atteinte, toute approche modèle les dilate (100,8-224 % mesurés). Exécutables et assets de jeux binaires : pas de preuve d'un quelconque avantage neuronal, cmix/paq8px (modèles x86 dédiés) restent la référence lente, zstd/LZ4/Oodle la référence rapide.
- **Chemin chaud (> 100 Mo/s, décodage côté client)** : hors de portée de toute inférence neuronale par token d'ici 2030 (facteur 10³-10⁵ à combler) ; les seuls « apprentissages » rentables y sont hors ligne (choix de codec, dictionnaires, paramètres).
- **Risque systémique** : la dépendance au triplet (modèle, pile d'inférence, matériel) crée un problème d'archivage long terme inédit ; un standard devra figer un modèle en entier/fixed-point avec implémentation de référence, à la manière du décodeur de référence JPEG — rien de tel n'existe en 2026.

### Gaps
- Aucune feuille de route publique (JPEG, MPEG, IETF) trouvée pour un codec neuronal sans perte généraliste.
- Aucun chiffre énergétique (J/Go) mesuré pour un compresseur LLM n'a été trouvé ; les estimations ci-dessus sont des ordres de grandeur dérivés de la puissance GPU et du débit.
