      // Content-defined chunking: blades cut where the data decides; an insert only reshapes the first chunk.
      const widths = [1.6, 1.1, 2.0, 0.9, 1.4, 1.2, 1.8, 1.0, 1.5, 1.3, 1.7, 1.1];
      const row = new THREE.Group(); scene.add(row);
      const chunks = []; let x = -9.4;
      widths.forEach((w, i) => {
        const c = V.box(w, 1.25, 1.25, PAL.chunks[i % 8]); c.position.set(x + w / 2, -0.55, 0); row.add(c); chunks.push(c); x += w + 0.22;
      });
      const blades = []; let bx = -9.4;
      widths.forEach((w, i) => {
        bx += w + 0.11;
        if (i === widths.length - 1) return;
        const b = V.box(0.07, 2.6, 1.7, 0xEEF2F7, { glow: 0.9, radius: 0.02, outline: 0 }); b.position.set(bx, 4.2, 0); row.add(b); blades.push(b); bx += 0.11;
      });
      const insert = V.box(0.6, 1.25, 1.25, PAL.amber, { glow: 0.7 }); insert.position.set(-13, -0.55, 0); row.add(insert);
      // row drifts left as the camera tracks it
      tl.fromTo(row.position, { x: 1.2 }, { x: -1.0, duration: T, ease: 'none' }, 0);
      // blades drop at the cut points, then lift
      blades.forEach((b, i) => {
        const at = 0.8 + i * 0.14;
        tl.to(b.position, { y: -0.55, duration: 0.22, ease: 'power3.in' }, at);
        tl.to(b.scale, { x: 1.8, duration: 0.08, yoyo: true, repeat: 1 }, at + 0.2);
        tl.to(b.position, { y: 3.6, duration: 0.5, ease: 'power2.out' }, at + 0.55);
        tl.to(b.scale, { y: 0.01, duration: 0.3 }, at + 0.8);
      });
      chunks.forEach((c, i) => tl.to(c.position, { y: -0.3, duration: 0.25, yoyo: true, repeat: 1, ease: 'sine.inOut' }, 0.95 + i * 0.14));
      // second half: ten bytes added at the front
      tl.to(insert.position, { x: -9.4 - 0.3 - 0.1, duration: 0.8, ease: 'power3.out' }, 5.0);
      tl.to(insert.position, { x: -9.4 + 0.3, duration: 0.35, ease: 'power2.in' }, 5.8);
      tl.to(insert.scale, { x: 0.01, duration: 0.2 }, 6.05);
      tl.to(chunks[0].scale, { x: 1.38, duration: 0.45, ease: 'back.out(1.6)' }, 6.1);
      tl.to(chunks[0].position, { x: chunks[0].position.x - 0.3, duration: 0.45, ease: 'back.out(1.6)' }, 6.1);
      glow(chunks[0].fill.material, PAL.amber, 0.4, 6.1, 0.3);
      col(chunks[0].fill.material, PAL.amber, 6.1, 0.3);
      chunks.slice(1).forEach((c, i) => { glow(c.fill.material, PAL.green, 0.8, 6.7 + i * 0.07, 0.2); glow(c.fill.material, PAL.green, 0.0, 7.4 + i * 0.07, 0.4); });
      // camera: lateral tracking shot
      st.cx = -4.5; st.cy = 3.4; st.cz = 11.5; st.tx = -4.0; st.ty = -0.2; st.tz = 0;
      tl.to(st, { cx: 1.0, cy: 2.6, cz: 9.0, tx: 1.5, ty: -0.3, duration: T, ease: 'none' }, 0);
