# Clips vidéo — à générer dans l'app ElevenLabs (Image & Video)

Réglages communs : modèle Veo 3.1 Fast (ou Fast disponible), 16:9, **son désactivé**, image de départ = le PNG indiqué.
Style à rappeler dans chaque prompt : « stylized 3D anime illustration, cinematic lighting, deep navy background #0B1220 with warm amber #F5B400 accents, smooth slow camera, no text, no letters ».
Nommer le fichier exporté exactement comme indiqué et le déposer dans `assets/clips/`.

| Scène | Durée | Image de départ | Fichier à déposer | Prompt |
|---|---|---|---|---|
| intro | 8 s | assets/art/intro.png | intro.mp4 | A giant sealed archive crate slowly cracking open, a flood of files pouring out and burying the desk, progress bar crawling, hard drive glowing red as it fills up. Slow push-in, oppressive mood. |
| mount | 7 s | assets/art/mount.png | mount.mp4 | A glowing amber cable plugs the archive into a computer like an external drive; folders and files light up instantly in a clean grid, snapping into place. Fast, satisfying, relief. Slow pull-back. |
| cdc | 10 s | assets/art/cdc.png | cdc.mp4 | A long stream of data blocks glides past; glowing blades cut it into chunks of uneven sizes exactly where the pattern changes. A small insert is added at the front: only the first chunk reshapes, every other chunk stays identical. Lateral tracking shot. |
| zstd | 9 s | assets/art/zstd.png | zstd.mp4 | Colorful data chunks each compress individually into dense glowing cubes; one single cube is plucked out and unfolds alone in a fraction of a second while the others stay sealed. Macro shot, slow drift. |
| merkle | 9 s | assets/art/merkle.png | merkle.mp4 | Rows of chunks each emit a small glowing fingerprint; fingerprints rise and merge pair by pair into a tree up to one bright root. One chunk turns red, the red path climbs the tree to the root. Slow tilt up. |
| parity | 8 s | assets/art/parity.png | parity.mp4 | A row of data chunks with two amber parity blocks at the end; one chunk shatters and vanishes, then threads of light from the neighbours and the parity blocks weave it back together, whole again. Slow push-in. |
| delta | 7 s | assets/art/delta.png | delta.mp4 | Two versions of a game archive side by side; only three chunks differ, and only those three fly across the gap to the second version, the rest stays still. Clean, minimal, slow lateral move. |
| outro | 7 s | assets/art/outro.png | outro.mp4 | A single elegant archive object floating in space, one cut pattern radiating outward and powering several uses at once: mounting, verifying, repairing, updating. Calm hero shot, slow orbit, ending on a steady hold. |

Ensuite :

    node tools/art.cjs && npx hyperframes check --json && npx hyperframes render --quality delivery --crf 14
