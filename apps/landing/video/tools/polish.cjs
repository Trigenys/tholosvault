const fs = require('fs');
const extra = `
    /* polish */
    .ck, .pr, .nd, #cdc-ins, .f { background-image: linear-gradient(180deg, rgba(255,255,255,.26) 0%, rgba(255,255,255,.06) 45%, rgba(0,0,0,.10) 100%); box-shadow: 0 14px 34px rgba(0,0,0,.45), inset 0 1px 0 rgba(255,255,255,.30), inset 0 -1px 0 rgba(0,0,0,.25); }
    .line { text-shadow: 0 3px 28px rgba(0,0,0,.65), 0 1px 2px rgba(0,0,0,.5); }
    .lab, .vl, #zstd-tag, #merkle-tag, #parity-tag, #intro-disklabel, #cdc-chg, #delta-count, #mount-ready, #parity-lost, #parity-ok { text-transform: uppercase; letter-spacing: .10em; font-weight: 600; }
    #intro-body, #mount-body { background-image: linear-gradient(160deg, #24365A 0%, #16233C 100%); box-shadow: 0 30px 70px rgba(0,0,0,.55), inset 0 1px 0 rgba(255,255,255,.12); }
    #intro-lid, #mount-lid { background-image: linear-gradient(180deg, #3A4F7E, #27395E); box-shadow: 0 6px 18px rgba(0,0,0,.4); }
    #mount-drive, #mount-plug { box-shadow: 0 0 0 6px rgba(245,180,0,.18), 0 0 60px rgba(245,180,0,.45), 0 14px 34px rgba(0,0,0,.45); background-image: linear-gradient(180deg, #FFD24A, #E9A800); }
    #mount-cable { box-shadow: 0 0 24px rgba(245,180,0,.55); }
    #merkle-root { box-shadow: 0 0 40px rgba(238,242,247,.35), 0 14px 34px rgba(0,0,0,.45); }
    #intro-bar { box-shadow: inset 0 2px 8px rgba(0,0,0,.6); }
    #intro-fill { background-image: linear-gradient(90deg, #4C7BD9, #6E9BF0); }
    #intro-full { box-shadow: 0 0 40px rgba(229,72,77,.55); }
    .read .n { text-shadow: 0 0 40px rgba(245,180,0,.25); }
    #outro-hero { text-shadow: 0 0 60px rgba(245,180,0,.25), 0 4px 30px rgba(0,0,0,.6); }
  </style>`;
for (const f of fs.readdirSync('compositions')) {
  const p = 'compositions/' + f; let h = fs.readFileSync(p, 'utf8');
  h = h.replace(/style="([^"]*)background: #/g, 'style="$1background-color: #');
  if (!h.includes('/* polish */')) h = h.replace('</style>', extra);
  fs.writeFileSync(p, h);
}
let ix = fs.readFileSync('index.html', 'utf8');
if (!ix.includes('#bg-grid')) {
  ix = ix.replace(`#bg { position: absolute; inset: 0; background: var(--ground, #0B1220); }`,
`#bg { position: absolute; inset: 0; background: radial-gradient(1100px 700px at 78% 18%, rgba(245,180,0,.14), transparent 60%), radial-gradient(1200px 800px at 15% 90%, rgba(76,123,217,.22), transparent 60%), var(--ground, #0B1220); }
      #bg-grid { position: absolute; inset: 0; background-image: linear-gradient(rgba(255,255,255,.045) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,.045) 1px, transparent 1px); background-size: 96px 96px; mask-image: radial-gradient(ellipse at 50% 50%, #000 40%, transparent 95%); -webkit-mask-image: radial-gradient(ellipse at 50% 50%, #000 40%, transparent 95%); }
      #bg-vignette { position: absolute; inset: 0; background: radial-gradient(ellipse at 50% 50%, transparent 55%, rgba(0,0,0,.55) 100%); }
      #brand { position: absolute; left: 96px; top: 60px; font-family: 'JetBrains Mono', monospace; font-size: 28px; font-weight: 700; letter-spacing: .18em; color: #EEF2F7; z-index: 5; display: flex; align-items: center; gap: 14px; }
      #brand::before { content: ''; width: 14px; height: 14px; border-radius: 50%; background: var(--accent, #F5B400); box-shadow: 0 0 18px rgba(245,180,0,.8); }
      #prog { position: absolute; left: 0; right: 0; bottom: 0; height: 5px; background: var(--accent, #F5B400); transform-origin: 0 50%; z-index: 5; box-shadow: 0 0 16px rgba(245,180,0,.6); }`);
  ix = ix.replace(`<div id="bg"></div>`, `<div id="bg"></div>\n      <div id="bg-grid"></div>\n      <div id="bg-vignette"></div>\n      <div id="brand">VAULT</div>\n      <div id="prog"></div>`);
  ix = ix.replace(`window.__timelines["main"] = gsap.timeline({ paused: true });`,
`const main = gsap.timeline({ paused: true });
      main.fromTo('#prog', { scaleX: 0 }, { scaleX: 1, duration: 65, ease: 'none' }, 0);
      window.__timelines["main"] = main;`);
  fs.writeFileSync('index.html', ix);
}
console.log('polish applied');
