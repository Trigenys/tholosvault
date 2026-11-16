// Builds compositions/art-<id>.html (illustration + camera move + caption) and index.html.
// Idempotent: rewrites the same files from the same table.
// A Google Flow clip dropped at assets/clips/<id>.mp4 replaces the still automatically.
const fs = require('fs');
const { S, VO, helpers } = require('./scenes.cjs');

for (const s of S) {
  const clip = `assets/clips/${s.id}.mp4`;
  const hasClip = fs.existsSync(clip);
  const mid = s.dur * 0.5;
  const media = hasClip
    ? `<video id="${s.id}-art" class="clip" src="${clip}" muted data-start="0" data-duration="${s.dur}"></video>`
    : `<img id="${s.id}-art" src="assets/art/${s.id}.png" alt="">`;
  const camTarget = hasClip ? `#${s.id}-cam` : `#${s.id}-art`;
  const wrapOpen = hasClip ? `<div id="${s.id}-cam">` : '';
  const wrapClose = hasClip ? `</div>` : '';
  const html = `<template>
  <style>
    #root { position: absolute; inset: 0; overflow: hidden; font-family: Inter, sans-serif; color: #EEF2F7; background: #0B1220; }
    #${s.id}-cam { position: absolute; inset: 0; will-change: transform; }
    #${s.id}-art { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; will-change: transform; }
    #${s.id}-shade { position: absolute; inset: 0; background: linear-gradient(180deg, rgba(11,18,32,.30) 0%, rgba(11,18,32,0) 30%, rgba(11,18,32,0) 55%, rgba(11,18,32,.90) 100%); }
    #${s.id}-glow { position: absolute; left: -20%; top: -10%; width: 60%; height: 120%; background: linear-gradient(100deg, rgba(255,255,255,0) 40%, rgba(255,230,160,.10) 50%, rgba(255,255,255,0) 60%); opacity: 0; pointer-events: none; }
    #${s.id}-flash { position: absolute; inset: 0; background: #0B1220; opacity: 0; }
    .line { position: absolute; left: 10%; right: 10%; bottom: 11%; font-size: 56px; font-weight: 700; letter-spacing: -0.02em; text-align: center; line-height: 1.2; text-shadow: 0 3px 28px rgba(0,0,0,.8), 0 1px 2px rgba(0,0,0,.7); }
    .w { display: inline-block; opacity: 0; }
    .acc { color: #F5B400; }
  </style>
  <div id="root" data-composition-id="${s.id}" data-layout-allow-overflow data-layout-allow-overlap>
    ${wrapOpen}${media}${wrapClose}
    <div id="${s.id}-shade"></div>
    <div id="${s.id}-glow"></div>
    <p class="line" id="${s.id}-line1">${s.l1}</p>
    ${s.l2 ? `<p class="line" id="${s.id}-line2">${s.l2}</p>` : ''}
    <div id="${s.id}-flash"></div>
  </div>
  <script>${helpers}

    const tl = gsap.timeline({ paused: true });
    tl.fromTo('${camTarget}', { transform: '${s.cam.from}' }, { transform: '${s.cam.to}', duration: ${s.dur}, ease: 'none' }, 0);
    tl.fromTo('#${s.id}-flash', { opacity: 1 }, { opacity: 0, duration: 0.6, ease: 'power2.out' }, 0);
    tl.fromTo('#${s.id}-glow', { opacity: 0, x: 0 }, { opacity: 1, x: 2400, duration: ${s.dur}, ease: 'none' }, 0);
    const w1 = lineIn(tl, '#${s.id}-line1', 0.5);
${s.l2 ? `    lineOut(tl, w1, ${(mid - 0.4).toFixed(2)});
    const w2 = lineIn(tl, '#${s.id}-line2', ${mid.toFixed(2)});
    lineOut(tl, w2, ${(s.dur - 0.6).toFixed(2)});` : `    lineOut(tl, w1, ${(s.dur - 0.6).toFixed(2)});`}
    tl.to('#${s.id}-flash', { opacity: 1, duration: 0.35, ease: 'power2.in' }, ${(s.dur - 0.35).toFixed(2)});
    window.__timelines['${s.id}'] = tl;
  </script>
</template>
`;
  fs.writeFileSync(`compositions/art-${s.id}.html`, html);
}

const { writeIndex } = require('./scenes.cjs'); writeIndex(fs, 'art-', '');
console.log('ok', S.length, 'scenes');
