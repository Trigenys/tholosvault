// Builds compositions/3d-<id>.html: a Three.js toon scene per chapter (scenes3d/<id>.js) + captions, and index.html.
// Idempotent: same inputs, same files. Run: node tools/build3d.cjs
const fs = require('fs');
const { S, helpers, writeIndex } = require('./scenes.cjs');

for (const s of S) {
  const snippet = fs.readFileSync(`scenes3d/${s.id}.js`, 'utf8');
  const mid = s.dur * 0.5;
  const html = `<template>
  <style>
    #root { position: absolute; inset: 0; overflow: hidden; font-family: Inter, sans-serif; color: #EEF2F7; background: #0B1220; }
    #${s.id}-gl { position: absolute; inset: 0; width: 1920px; height: 1080px; }
    #${s.id}-shade { position: absolute; inset: 0; background: linear-gradient(180deg, rgba(11,18,32,.25) 0%, rgba(11,18,32,0) 28%, rgba(11,18,32,0) 58%, rgba(11,18,32,.88) 100%); }
    #${s.id}-vig { position: absolute; inset: 0; background: radial-gradient(ellipse at 50% 45%, rgba(0,0,0,0) 55%, rgba(0,0,0,.45) 100%); }
    #${s.id}-flash { position: absolute; inset: 0; background: #0B1220; opacity: 0; }
    .line { position: absolute; left: 10%; right: 10%; bottom: 11%; font-size: 56px; font-weight: 700; letter-spacing: -0.02em; text-align: center; line-height: 1.2; text-shadow: 0 3px 28px rgba(0,0,0,.8), 0 1px 2px rgba(0,0,0,.7); }
    .w { display: inline-block; opacity: 0; }
    .acc { color: #F5B400; }
  </style>
  <div id="root" data-composition-id="${s.id}" data-layout-allow-overflow data-layout-allow-overlap>
    <canvas id="${s.id}-gl" width="1920" height="1080"></canvas>
    <div id="${s.id}-vig"></div>
    <div id="${s.id}-shade"></div>
    <p class="line" id="${s.id}-line1">${s.l1}</p>
    ${s.l2 ? `<p class="line" id="${s.id}-line2">${s.l2}</p>` : ''}
    <div id="${s.id}-flash"></div>
  </div>
  <script>${helpers}

    (function () {
      const T = ${s.dur}, V = window.V3D, PAL = V.PAL;
      const tl = gsap.timeline({ paused: true });
      const stg = V.stage(document.getElementById('${s.id}-gl'), { seed: ${11 + S.indexOf(s) * 101} });
      const { scene, st, rng } = stg;
      const C = PAL.chunks.map(c => new THREE.Color(c));
      const col = (mat, hex, at, d) => { const c = new THREE.Color(hex); tl.to(mat.color, { r: c.r, g: c.g, b: c.b, duration: d || 0.3 }, at); };
      const glow = (mat, hex, i, at, d) => { const c = new THREE.Color(hex); tl.to(mat.emissive, { r: c.r, g: c.g, b: c.b, duration: d || 0.3 }, at); tl.to(mat, { emissiveIntensity: i, duration: d || 0.3 }, at); };
      // --- scene: ${s.id} ---
${snippet}
      // --- captions + cut ---
      tl.fromTo('#${s.id}-flash', { opacity: 1 }, { opacity: 0, duration: 0.6, ease: 'power2.out' }, 0);
      const w1 = lineIn(tl, '#${s.id}-line1', 0.5);
${s.l2 ? `      lineOut(tl, w1, ${(mid - 0.4).toFixed(2)});
      const w2 = lineIn(tl, '#${s.id}-line2', ${mid.toFixed(2)});
      lineOut(tl, w2, ${(s.dur - 0.6).toFixed(2)});` : `      lineOut(tl, w1, ${(s.dur - 0.6).toFixed(2)});`}
      tl.to('#${s.id}-flash', { opacity: 1, duration: 0.35, ease: 'power2.in' }, ${(s.dur - 0.35).toFixed(2)});
      tl.eventCallback('onUpdate', stg.render);
      window.__timelines['${s.id}'] = tl;
      stg.render();
    })();
  </script>
</template>
`;
  fs.writeFileSync(`compositions/3d-${s.id}.html`, html);
}

writeIndex(fs, '3d-', `  <script src="https://cdn.jsdelivr.net/npm/three@0.147.0/build/three.min.js"></script>
  <script src="assets/js/vault3d.js"></script>`);
console.log('ok 3d', S.length, 'scenes');
