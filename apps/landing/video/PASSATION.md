
## 2026-11-16 09:09 — Voix off ajoutée
- TTS HeyGen refusé (plan HyperFrames payant). Remplacé par edge-tts (gratuit, voix fr-FR-HenriNeural), script `tools/tts.py <request.json> <voice> <rate>` → `assets/voice/<id>.mp3`.
- 8 clips `<audio id="vo-*">` dans index.html, musique baissée à 0.3. check : 0 finding ; audio check : 0 finding.
- Rendu `renders/video-vault_2026-11-16_09-07-27.mp4` → livré `../VAULT-principe-voix.mp4` (65 s).
- Prochaine étape éventuelle : autre voix (fr-FR-DeniseNeural), ajuster `data-volume` ou offsets.

## 2026-11-16 09:57 — v2 : voix ElevenLabs + polish visuel + rendu HQ
- Voix off régénérée avec ElevenLabs (voix « Adam - Formation, Tutos & Education », modèle eleven_multilingual_v2) via `tools/eleven.py` ; clé lue depuis la variable d'env ELEVENLABS_API_KEY (injectée depuis le .env.local de casino-de-la-reine, jamais journalisée). Fichiers : `assets/voice/*.mp3`, ~850 caractères de quota consommés.
- Lignes cdc et outro raccourcies dans `voice_request.json` pour tenir dans leurs scènes ; `data-start`/`data-duration` des clips `vo-*` recalés sur les durées réelles.
- Polish visuel : `tools/polish.cjs` (idempotent) + fond dégradé/grille/vignette, marque VAULT, barre de progression dans `index.html` ; scènes agrandies de 12 % (règle `/* scene-scale */`) ; `#parity-tag` élargi (left 1060, width 660) pour éviter le retour à la ligne.
- Rendu : `npx hyperframes render --quality delivery --crf 14` → `renders/video-vault_2026-11-16_09-48-21.mp4` (65 s, 592 kbps vidéo, check et audio à 0 finding). Livrable : `../VAULT-principe-v2.mp4`.
- Reste possible : autre voix (Guillaume ohItIVrXTBI80RrUECOD, Camille hFgOzpmS0CMtL2to8sAl), rendu 4K (`--resolution 4k`).

## 2026-11-16 10:31 — v3 : visuels 3D/anime générés (Magnific) + caméra HyperFrames

- Demande : remplacer les graphiques vectoriels (jugés laids) par du 3D / anime.
- Contraintes constatées : ElevenLabs n'expose aucune génération image/vidéo par API (sondes 404, fonction web-app uniquement) ; Magnific en plan Free : vidéo (kling) refusée, images `auto` gratuites (20/jour, 1280x720).
- Fait : 8 illustrations 3D stylisé anime générées (Magnific, projet Personal) → `assets/art/<id>.png` ; planche `art-sheet.jpg`.
- `tools/art.cjs` (idempotent) génère `compositions/art-<id>.html` (image plein cadre, push-in/pan GSAP sur `transform`, balayage lumineux, dégradé bas pour les légendes, fondu noir entrée/sortie, légendes mot à mot) et le nouveau `index.html` (GSAP CDN, data-width/height, bgm 0.28, 8 voix ElevenLabs aux durées réelles).
- Chemin d'upgrade : déposer un clip Google Flow dans `assets/clips/<id>.mp4` puis `node tools/art.cjs` → la scène passe automatiquement en `<video muted>` avec la même caméra.
- Ancienne version vectorielle conservée : `variants/index-graphics.html` (déplacée hors racine : deux index à la racine = double audio au check).
- Vérif : `npx hyperframes check --json` ok (0 non-info, 8 sections vertes) ; planche `snaps-v3/contact-sheet-1.jpg` relue.
- Rendu : `--quality delivery --crf 14` → `renders/` puis copie `../VAULT-principe-v3.mp4`.
- Suite possible : clips Flow (vrai mouvement 3D/anime), upscale Magnific des PNG si plan payant, autre voix.
- Incident (near miss) : le découpage mot à mot utilisait une regex `\s` dans un template literal → les barres obliques inverses étaient avalées à la génération (« un eul fichier »). Résolution : `split(' ')` et `charAt(0)` sans échappement dans `tools/art.cjs`. Leçon : pas de séquences d'échappement dans le code émis par un générateur ; vérifier une légende en gros plan (crop ffmpeg) avant rendu.
- Voix : data-duration alignées sur les durées réelles des MP3 (plus d'avertissement runtime).
- Livré : `renders/video-vault_2026-11-16_10-40-11.mp4` → `../VAULT-principe-v3.mp4` (65,0 s, 1920x1080, ~7,5 Mbit/s vidéo, audio 0 finding). Envoyé à l'utilisateur. Preview non lancée (pas nécessaire).

## 2026-11-16 10:48 — Veo via ElevenLabs : vérifié, bloqué par le plan
- L'API ElevenLabs Image & Video (POST /v1/flows/video, modèles Veo 3.1) existe mais exige le plan Pro+. Compte actuel : tier **starter** → 402 sur /v1/flows/video. Google Flow : crédits épuisés (dit par l'utilisateur).
- État : v3 livrée (stills Magnific + caméra). Dès que des clips existent, les déposer en assets/clips/<id>.mp4 puis `node tools/art.cjs` + render.

## 2026-11-16 10:59 — Tarifs ElevenLabs vidéo (vérifiés sur elevenlabs.io)
- Pro = 99 USD/mois, 600k crédits, seul plan avec l'API /v1/flows/video. Génération vidéo dans l'app web : dès Starter (plan actuel, 26,6k crédits restants, ~211 s de vidéo/mois), Creator 22 USD = tous modèles + 1080p (~705 s). Coût crédits par seconde non publié (« varie selon réglages »).
- Option sans surcoût : l'utilisateur génère les 8 clips (~7-10 s chacun) dans l'app ElevenLabs sur Starter, les dépose en assets/clips/<id>.mp4 → `node tools/art.cjs` + render.

## 2026-11-16 11:10 — Clips : génération dans l'app ElevenLabs (plan Starter)
- Décision utilisateur : générer les 8 clips dans l'app web. Fiche prompts + images de départ : PROMPTS-CLIPS.md, dossier assets/clips/ créé. Prochaine étape : dès réception, `node tools/art.cjs`, check, render v4, livrer.

## 2026-11-16 11:30 — v4 : pivot 3D (Three.js toon), plus de génération externe
- Décision : les clips vidéo générés (ElevenLabs/Flow) sont abandonnés, trop longs ; les 8 scènes sont rendues en 3D procédurale dans HyperFrames.
- Fichiers : `assets/js/vault3d.js` (moteur : palette, toon material, box/orb/rod, stage avec caméra pilotée par GSAP via `st`), `scenes3d/<id>.js` (8 snippets de scène), `tools/scenes.cjs` (table partagée scènes/VO/légendes), `tools/build3d.cjs` (génère `compositions/3d-*.html` + `index.html`).
- Régénérer : `node tools/build3d.cjs` puis `npx hyperframes check --json` puis `npx hyperframes render --quality delivery --crf 14`. Variante stills : `node tools/art.cjs` (sauvegardes dans `variants/`).
- Pièges : Three 0.147 → forcer `THREE.ColorManagement.legacyMode = false` sinon couleurs pastel délavées ; glows amber > 0.4 saturent en jaune plat ; cadrer les rangées pour fov 38 (demi-largeur ≈ 0,6 × distance caméra).
- Livré : `../VAULT-principe-v4.mp4` (65 s, 1920×1080, check OK, snapshots relus dans `snaps-v4b/`).

## 2026-11-16 11:57 — v5 : matériaux PBR (plus réalistes)

- Retour utilisateur sur la v4 : « beaucoup mieux, rendre les objets plus réalistes ».
- `assets/js/vault3d.js` : `toon()` renvoie désormais un `MeshPhysicalMaterial` (roughness 0.38, clearcoat 0.45),
  environnement PMREM procédural (pièce sombre + panneaux chaud/froid/ambre), `ACESFilmicToneMapping` exposure 1.15,
  ombres adoucies, contours ink retirés (`box()` outline par défaut 0). Ancienne version toon : `variants/vault3d-toon.js`.
- `scenes3d/outro.js` : plus de coque de contour sur les satellites, émissif 0.18. `scenes3d/zstd.js` : glow ambre 0.18, pad 0.15.
- Régénérer : `node tools/build3d.cjs` → `npx hyperframes check --json` → `npx hyperframes render --quality delivery --crf 14`.
- Livré : `../VAULT-principe-v5.mp4` (65 s, 1920×1080, audio, ~39 Mo). Frames de contrôle : `snaps-v5/`, `snaps-v5b/`.
- Piège : avec le PBR, les émissifs ambre > 0.3 saturent en jaune plat sous ACES ; rester ≤ 0.2 sur les objets éclairés.
