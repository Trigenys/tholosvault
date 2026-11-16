      // Fingerprints rise from the chunks and merge into a tree; a corrupted byte lights the path to the root.
      const chunks = [], leaves = [], l1 = [], l2 = [], rods = {};
      const X = i => -5.6 + i * 1.6;
      for (let i = 0; i < 8; i++) {
        const c = V.box(1.3, 1.1, 1.1, PAL.chunks[i]); c.position.set(X(i), -0.65, 0); scene.add(c); chunks.push(c);
        const o = V.orb(0.2, PAL.chunks[i], 0.7); o.position.set(X(i), 0.9, 0); o.scale.setScalar(0.01); scene.add(o); leaves.push(o);
      }
      const P1 = i => (X(i * 2) + X(i * 2 + 1)) / 2, P2 = i => (P1(i * 2) + P1(i * 2 + 1)) / 2;
      for (let i = 0; i < 4; i++) { const o = V.orb(0.24, 0x93A1BC, 0.5); o.position.set(P1(i), 2.5, 0); o.scale.setScalar(0.01); scene.add(o); l1.push(o); }
      for (let i = 0; i < 2; i++) { const o = V.orb(0.28, 0xC8D2E4, 0.5); o.position.set(P2(i), 4.0, 0); o.scale.setScalar(0.01); scene.add(o); l2.push(o); }
      const root = V.orb(0.42, 0xEEF2F7, 0.5); root.position.set((P2(0) + P2(1)) / 2, 5.5, 0); root.scale.setScalar(0.01); scene.add(root);
      const link = (name, a, b) => { const r = V.rod(0x3B4D70, 0.035); r.setEnds(a.position, b.position); r.material.opacity = 0; scene.add(r); rods[name] = r; return r; };
      for (let i = 0; i < 8; i++) link('l' + i, leaves[i], l1[Math.floor(i / 2)]);
      for (let i = 0; i < 4; i++) link('m' + i, l1[i], l2[Math.floor(i / 2)]);
      for (let i = 0; i < 2; i++) link('t' + i, l2[i], root);
      const pop = (o, at) => tl.to(o.scale, { x: 1, y: 1, z: 1, duration: 0.35, ease: 'back.out(2)' }, at);
      const show = (r, at) => tl.to(r.material, { opacity: 1, duration: 0.3 }, at);
      leaves.forEach((o, i) => pop(o, 1.0 + i * 0.07));
      for (let i = 0; i < 8; i++) show(rods['l' + i], 1.9 + i * 0.05);
      l1.forEach((o, i) => pop(o, 2.2 + i * 0.08));
      for (let i = 0; i < 4; i++) show(rods['m' + i], 2.7 + i * 0.06);
      l2.forEach((o, i) => pop(o, 3.0 + i * 0.1));
      for (let i = 0; i < 2; i++) show(rods['t' + i], 3.4 + i * 0.08);
      pop(root, 3.7);
      tl.to(root.scale, { x: 1.25, y: 1.25, z: 1.25, duration: 0.25, yoyo: true, repeat: 1 }, 4.05);
      // one damaged byte: chunk 4 and its path turn red
      const red = (o, at) => { col(o.fill.material, PAL.red, at, 0.25); glow(o.fill.material, PAL.red, 0.9, at, 0.25); };
      const redRod = (r, at) => { col(r.material, PAL.red, at, 0.25); tl.to(r.scale, { x: 2.2, z: 2.2, duration: 0.25 }, at); };
      red(chunks[4], 4.8); tl.to(chunks[4].position, { y: -0.45, duration: 0.07, yoyo: true, repeat: 5 }, 4.8);
      red(leaves[4], 5.05); redRod(rods['l4'], 5.25); red(l1[2], 5.45); redRod(rods['m2'], 5.65); red(l2[1], 5.85); redRod(rods['t1'], 6.05); red(root, 6.25);
      // camera: tilt up along the tree
      st.cx = 0.5; st.cy = 2.0; st.cz = 13.5; st.tx = 0; st.ty = 0.6; st.tz = 0;
      tl.to(st, { cx: -0.6, cy: 4.6, cz: 12.2, tx: 0.2, ty: 2.6, duration: T, ease: 'none' }, 0);
