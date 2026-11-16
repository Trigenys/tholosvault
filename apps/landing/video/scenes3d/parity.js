      // A chunk shatters; neighbours and the two parity blocks weave it back.
      const chunks = [], X = i => -5.9 + i * 1.3;
      for (let i = 0; i < 8; i++) { const c = V.box(1.08, 1.05, 1.05, PAL.chunks[i]); c.position.set(X(i), -0.65, 0); scene.add(c); chunks.push(c); }
      const par = [];
      for (let i = 0; i < 2; i++) { const p = V.box(1.08, 1.05, 1.05, PAL.amber, { glow: 0.3 }); p.position.set(X(8) + 0.35 + i * 1.3, -0.65, 0); scene.add(p); par.push(p); }
      const lost = chunks[4], at0 = lost.position.clone();
      const frags = [];
      for (let i = 0; i < 16; i++) { const f = V.box(0.28, 0.28, 0.28, PAL.chunks[4], { radius: 0.03, outline: 0.02 }); f.position.copy(at0); f.scale.setScalar(0.01); scene.add(f); frags.push(f); }
      const threads = [];
      chunks.forEach((c, i) => { if (i === 4) return; const r = V.rod(PAL.cyan, 0.03); r.setEnds(new THREE.Vector3(c.position.x, 0.2, 0), new THREE.Vector3(at0.x, 0.2, 0)); r.material.opacity = 0; scene.add(r); threads.push(r); });
      par.forEach(p => { const r = V.rod(PAL.amber, 0.04); r.setEnds(new THREE.Vector3(p.position.x, 0.2, 0), new THREE.Vector3(at0.x, 0.2, 0)); r.material.opacity = 0; scene.add(r); threads.push(r); });
      // shatter
      col(lost.fill.material, PAL.red, 1.5, 0.3); glow(lost.fill.material, PAL.red, 0.9, 1.5, 0.3);
      tl.to(lost.position, { y: -0.5, duration: 0.06, yoyo: true, repeat: 5 }, 1.5);
      tl.to(lost.scale, { x: 0.01, y: 0.01, z: 0.01, duration: 0.18, ease: 'power3.in' }, 1.95);
      frags.forEach(f => {
        const a = rng() * Math.PI * 2, u = rng() * 2 - 1, sp = 1.2 + rng() * 1.6;
        tl.to(f.scale, { x: 1, y: 1, z: 1, duration: 0.08 }, 1.95);
        tl.to(f.position, { x: at0.x + Math.cos(a) * sp * 1.4, z: at0.z + Math.sin(a) * sp * 0.8, duration: 0.9, ease: 'power2.out' }, 1.98);
        tl.to(f.position, { y: at0.y + 1.2 + u, duration: 0.45, ease: 'power2.out' }, 1.98);
        tl.to(f.position, { y: -1.05, duration: 0.5, ease: 'power2.in' }, 2.43);
        tl.to(f.rotation, { x: rng() * 4, y: rng() * 4, duration: 0.9 }, 1.98);
        tl.to(f.scale, { x: 0.01, y: 0.01, z: 0.01, duration: 0.3 }, 3.1 + rng() * 0.3);
      });
      // threads weave, chunk rebuilds in cyan
      threads.forEach((r, i) => tl.to(r.material, { opacity: 0.95, duration: 0.25 }, 3.0 + i * 0.09));
      par.forEach(p => { glow(p.fill.material, PAL.amber, 1.0, 3.5, 0.2); tl.to(p.position, { y: -0.45, duration: 0.3, yoyo: true, repeat: 1, ease: 'sine.inOut' }, 3.5); });
      col(lost.fill.material, PAL.cyan, 4.2, 0.1); glow(lost.fill.material, PAL.cyan, 0.9, 4.2, 0.2);
      tl.to(lost.scale, { x: 1, y: 1, z: 1, duration: 0.6, ease: 'back.out(1.8)' }, 4.2);
      tl.to(lost.rotation, { y: Math.PI * 2, duration: 0.8, ease: 'power2.out' }, 4.2);
      threads.forEach(r => tl.to(r.material, { opacity: 0, duration: 0.4 }, 4.9));
      glow(lost.fill.material, PAL.cyan, 0.2, 5.4, 0.6);
      par.forEach(p => glow(p.fill.material, PAL.amber, 0.45, 5.0, 0.5));
      // camera: push-in on the gap
      st.cx = 0.6; st.cy = 3.8; st.cz = 15.5; st.tx = 0.4; st.ty = -0.2; st.tz = 0;
      tl.to(st, { cx: -0.2, cy: 2.6, cz: 12.0, tx: -0.1, ty: -0.3, duration: T, ease: 'none' }, 0);
