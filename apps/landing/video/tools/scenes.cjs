// Shared scene table: timing and captions. Used by art.cjs (stills) and build3d.cjs (Three.js).
const S = [
  { id: 'intro',  start: 0,  dur: 8,  cam: { from: 'scale(1.00) translate(0%,0%)', to: 'scale(1.10) translate(-1.5%,1%)' }, l1: 'Pour lire un seul fichier, il faut <b>tout sortir</b>.', l2: 'Un quart d\'heure, <b>un disque plein</b>.' },
  { id: 'mount',  start: 8,  dur: 7,  cam: { from: 'scale(1.12) translate(2%,1%)', to: 'scale(1.02) translate(0%,0%)' },     l1: 'VAULT : on <b>branche</b> l\'archive, on ne l\'ouvre pas.', l2: 'Prêt en <b>moins d\'une seconde</b>.' },
  { id: 'cdc',    start: 15, dur: 10, cam: { from: 'scale(1.00) translate(0%,0%)', to: 'scale(1.14) translate(2%,-1.5%)' }, l1: 'On coupe là où le <b>contenu</b> le décide.', l2: 'Un ajout au début : <b>un seul</b> morceau change.' },
  { id: 'zstd',   start: 25, dur: 9,  cam: { from: 'scale(1.14) translate(-2%,0%)', to: 'scale(1.02) translate(0%,0%)' },    l1: 'Chaque morceau est compressé <b>seul</b>.', l2: 'Lire un morceau : <b>0,1 ms</b> au lieu de 122.' },
  { id: 'merkle', start: 34, dur: 9,  cam: { from: 'scale(1.00) translate(0%,2%)', to: 'scale(1.12) translate(0%,-2%)' },   l1: 'Une <b>empreinte</b> par morceau, résumées en une racine.', l2: 'Un octet abîmé : on sait <b>exactement où</b>.' },
  { id: 'parity', start: 43, dur: 8,  cam: { from: 'scale(1.10) translate(1%,1%)', to: 'scale(1.00) translate(0%,0%)' },    l1: '3 à 5 % de <b>parité</b> : on répare sans retélécharger.', l2: null },
  { id: 'delta',  start: 51, dur: 7,  cam: { from: 'scale(1.00) translate(0%,0%)', to: 'scale(1.12) translate(0%,1%)' },    l1: 'Nouvelle version : seuls les morceaux <b>changés</b> voyagent.', l2: null },
  { id: 'outro',  start: 58, dur: 7,  cam: { from: 'scale(1.06) translate(0%,0%)', to: 'scale(1.00) translate(0%,0%)' },    l1: 'Une seule <b>découpe</b> sert à tout.', l2: 'L\'intelligence à l\'encodage. Un lecteur <b>simple, rapide, prévisible</b>.' },
];
const VO = { intro: [0.5, 6.04], mount: [8.4, 5.62], cdc: [15.3, 9.10], zstd: [25.5, 6.73], merkle: [34.4, 7.11], parity: [43.5, 5.48], delta: [51.6, 3.11], outro: [58.4, 6.36] };

const helpers = `
    function vwords(sel) {
      const el = document.querySelector(sel); const out = [];
      [...el.childNodes].forEach(n => {
        if (n.nodeType === 3) { const raw = n.textContent; raw.trim().split(' ').filter(Boolean).forEach((w, i) => { if (i === 0 && out.length && raw.charAt(0) !== ' ') { out[out.length - 1].textContent += w; return; } const s = document.createElement('span'); s.className = 'w'; s.textContent = w; out.push(s); }); }
        else { const s = document.createElement('span'); s.className = 'w acc'; s.textContent = n.textContent; out.push(s); }
      });
      el.innerHTML = ''; out.forEach(s => { el.appendChild(s); el.appendChild(document.createTextNode(' ')); });
      return out;
    }
    function lineIn(tl, sel, at) { const w = vwords(sel); tl.fromTo(w, { opacity: 0, y: 18, filter: 'blur(6px)' }, { opacity: 1, y: 0, filter: 'blur(0px)', duration: 0.3, stagger: 0.06, ease: 'power4.out' }, at); return w; }
    function lineOut(tl, w, at) { tl.to(w, { opacity: 0, y: -10, filter: 'blur(6px)', duration: 0.2, stagger: 0.02, ease: 'power3.in' }, at); }`;

// index.html shared by both generators. extraHead: scripts to add (e.g. Three.js).
function writeIndex(fs, compPrefix, extraHead) {
  const hosts = S.map(s => `    <div id="${s.id}" data-composition-id="${s.id}" class="clip" data-composition-src="compositions/${compPrefix}${s.id}.html" data-start="${s.start}" data-duration="${s.dur}" data-track-index="0" data-track-kind="graphics" data-width="1920" data-height="1080"></div>`).join('\n');
  const vo = Object.entries(VO).map(([id, [st, d]]) => `    <audio id="vo-${id}" class="clip" src="assets/voice/${id}.mp3" data-start="${st}" data-duration="${d}" data-volume="1"></audio>`).join('\n');
  fs.writeFileSync('index.html', `<!doctype html>
<html lang="fr">
<head>
  <meta charset="utf-8">
  <title>VAULT, le principe</title>
  <script src="https://cdn.jsdelivr.net/npm/gsap@3.15.0/dist/gsap.min.js"></script>
${extraHead || ''}
  <style>
    html, body { margin: 0; background: #0B1220; }
    #root { position: relative; width: 1920px; height: 1080px; overflow: hidden; background: #0B1220; font-family: Inter, sans-serif; }
    #brand { position: absolute; left: 96px; top: 60px; z-index: 5; font-family: 'JetBrains Mono', monospace; font-size: 28px; letter-spacing: .18em; color: #EEF2F7; opacity: .92; text-shadow: 0 2px 12px rgba(0,0,0,.7); }
    #brand i { display: inline-block; width: 12px; height: 12px; border-radius: 50%; background: #F5B400; margin-right: 14px; box-shadow: 0 0 18px #F5B400; }
    #prog { position: absolute; left: 0; bottom: 0; height: 5px; width: 100%; background: #F5B400; transform-origin: 0 50%; z-index: 6; box-shadow: 0 0 16px rgba(245,180,0,.6); }
  </style>
</head>
<body>
  <div id="root" class="clip" data-composition-id="main" data-start="0" data-duration="65" data-width="1920" data-height="1080">
    <audio id="bgm" class="clip" src="assets/bgm/track65.mp3" data-start="0" data-duration="65" data-volume="0.28"></audio>
${vo}
${hosts}
    <div id="brand"><i></i>VAULT</div>
    <div id="prog"></div>
  </div>
  <script>
    window.__timelines = window.__timelines || {};
    const main = gsap.timeline({ paused: true });
    main.fromTo('#prog', { scaleX: 0 }, { scaleX: 1, duration: 65, ease: 'none' }, 0);
    window.__timelines["main"] = main;
  </script>
</body>
</html>
`);
}

module.exports = { S, VO, helpers, writeIndex };
