// VAULT 3D helper: toon (anime) look on Three.js 0.147 (global THREE). Deterministic: no clock, no Math.random.
(function () {
  const PAL = {
    bg: 0x0B1220, ground: 0x121D33, ink: 0x060B16, text: 0xEEF2F7,
    amber: 0xF5B400, red: 0xE5484D, cyan: 0x4FB3C9, green: 0x5BB98C,
    chunks: [0x4C7BD9, 0x5BB98C, 0xB08BE6, 0xE0905A, 0x4FB3C9, 0xC9A54F, 0x7A8FE0, 0xD97BA6],
  };

  function rng(seed) { let a = seed | 0; return function () { a |= 0; a = (a + 0x6d2b79f5) | 0; let t = Math.imul(a ^ (a >>> 15), 1 | a); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; }

  let gradient = null;
  function gradientMap() {
    if (gradient) return gradient;
    const data = new Uint8Array([55, 120, 190, 235]);
    gradient = new THREE.DataTexture(data, 4, 1, THREE.RedFormat);
    gradient.minFilter = THREE.NearestFilter; gradient.magFilter = THREE.NearestFilter; gradient.needsUpdate = true;
    return gradient;
  }

  function toon(color, opts) {
    const m = new THREE.MeshToonMaterial(Object.assign({ color, gradientMap: gradientMap() }, opts || {}));
    return m;
  }

  const outlineMat = () => new THREE.MeshBasicMaterial({ color: PAL.ink, side: THREE.BackSide });

  // Beveled box: the anime "soft cube". Returns a Group with fill + outline, exposes .fill and .line.
  function box(w, h, d, color, opts) {
    opts = opts || {};
    const r = opts.radius != null ? opts.radius : Math.min(w, h, d) * 0.12;
    const shape = new THREE.Shape();
    const x = -w / 2, y = -h / 2;
    shape.moveTo(x + r, y); shape.lineTo(x + w - r, y); shape.quadraticCurveTo(x + w, y, x + w, y + r);
    shape.lineTo(x + w, y + h - r); shape.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
    shape.lineTo(x + r, y + h); shape.quadraticCurveTo(x, y + h, x, y + h - r);
    shape.lineTo(x, y + r); shape.quadraticCurveTo(x, y, x + r, y);
    const geo = new THREE.ExtrudeGeometry(shape, { depth: d - r * 2, bevelEnabled: true, bevelThickness: r, bevelSize: r, bevelSegments: 3, curveSegments: 6 });
    geo.center();
    const g = new THREE.Group();
    const fill = new THREE.Mesh(geo, toon(color, { emissive: color, emissiveIntensity: opts.glow || 0 }));
    fill.castShadow = true; fill.receiveShadow = true;
    const line = new THREE.Mesh(geo, outlineMat());
    const k = opts.outline != null ? opts.outline : 0.035;
    line.scale.set(1 + k / w, 1 + k / h, 1 + k / d);
    g.add(line); g.add(fill); g.fill = fill; g.line = line;
    return g;
  }

  function orb(radius, color, glow) {
    const g = new THREE.Group();
    const geo = new THREE.SphereGeometry(radius, 24, 16);
    const fill = new THREE.Mesh(geo, toon(color, { emissive: color, emissiveIntensity: glow || 0.6 }));
    fill.castShadow = true;
    const line = new THREE.Mesh(geo, outlineMat()); line.scale.setScalar(1 + 0.04 / radius * 0.5);
    g.add(line); g.add(fill); g.fill = fill; g.line = line;
    return g;
  }

  // Thin glowing rod between two points (updated via setEnds).
  function rod(color, radius) {
    const geo = new THREE.CylinderGeometry(radius || 0.03, radius || 0.03, 1, 8, 1);
    geo.translate(0, 0.5, 0);
    const m = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 1 }));
    m.setEnds = function (a, b) {
      const d = new THREE.Vector3().subVectors(b, a); const len = d.length();
      m.position.copy(a); m.scale.set(1, len, 1);
      m.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), d.normalize());
    };
    return m;
  }

  // A standard set: renderer, scene, camera, lights, ground, dust. st holds camera state for GSAP.
  function stage(canvas, opts) {
    opts = opts || {};
    const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, preserveDrawingBuffer: true });
    renderer.setPixelRatio(1); renderer.setSize(1920, 1080, false);
    renderer.shadowMap.enabled = true; renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    renderer.outputEncoding = THREE.sRGBEncoding; if (THREE.ColorManagement) THREE.ColorManagement.legacyMode = false;
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(PAL.bg);
    scene.fog = new THREE.Fog(PAL.bg, opts.fogNear || 18, opts.fogFar || 40);
    const camera = new THREE.PerspectiveCamera(opts.fov || 38, 1920 / 1080, 0.1, 100);

    scene.add(new THREE.HemisphereLight(0x8FA6D9, 0x0B1220, 0.4));
    const key = new THREE.DirectionalLight(0xFFE2B0, 1.0); key.position.set(6, 10, 7);
    key.castShadow = true; key.shadow.mapSize.set(2048, 2048);
    key.shadow.camera.left = -14; key.shadow.camera.right = 14; key.shadow.camera.top = 14; key.shadow.camera.bottom = -14; key.shadow.bias = -0.0015;
    scene.add(key);
    const rim = new THREE.DirectionalLight(0x4FB3C9, 0.55); rim.position.set(-8, 5, -6); scene.add(rim);

    if (opts.ground !== false) {
      const ground = new THREE.Mesh(new THREE.PlaneGeometry(120, 120), toon(PAL.ground));
      ground.rotation.x = -Math.PI / 2; ground.position.y = opts.groundY != null ? opts.groundY : -1.2; ground.receiveShadow = true;
      scene.add(ground);
    }
    // dust: seeded points for depth
    const r = rng(opts.seed || 7), n = 160, pos = new Float32Array(n * 3);
    for (let i = 0; i < n; i++) { pos[i * 3] = (r() - 0.5) * 40; pos[i * 3 + 1] = r() * 10 - 1; pos[i * 3 + 2] = (r() - 0.5) * 30 - 4; }
    const dustGeo = new THREE.BufferGeometry(); dustGeo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    const dust = new THREE.Points(dustGeo, new THREE.PointsMaterial({ color: 0xF5B400, size: 0.06, transparent: true, opacity: 0.55 }));
    scene.add(dust);

    const st = { cx: 0, cy: 3, cz: 12, tx: 0, ty: 0, tz: 0, roll: 0 };
    function render() {
      camera.position.set(st.cx, st.cy, st.cz);
      camera.lookAt(st.tx, st.ty, st.tz);
      camera.rotation.z += st.roll;
      renderer.render(scene, camera);
    }
    return { renderer, scene, camera, key, rim, dust, st, render, rng: rng(opts.seed || 7) };
  }

  window.V3D = { PAL, rng, toon, box, orb, rod, stage };
})();
