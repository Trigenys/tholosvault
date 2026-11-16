      // Each chunk compresses alone; one is plucked out and unfolds by itself.
      const chunks = [];
      for (let i = 0; i < 8; i++) {
        const c = V.box(1.6, 1.25, 1.25, PAL.chunks[i]); c.position.set(-6.65 + i * 1.9, -0.55, 0); scene.add(c); chunks.push(c);
      }
      const pad = new THREE.Mesh(new THREE.CylinderGeometry(1.2, 1.2, 0.06, 40), V.toon(PAL.amber, { emissive: PAL.amber, emissiveIntensity: 0.15 }));
      pad.position.set(chunks[3].position.x, -1.17, 0); pad.scale.setScalar(0.01); scene.add(pad);
      // compression: squash with a glow pulse
      chunks.forEach((c, i) => {
        const at = 1.5 + i * 0.09;
        tl.to(c.scale, { x: 0.46, y: 0.92, z: 0.92, duration: 0.5, ease: 'power3.inOut' }, at);
        tl.to(c.position, { y: -0.75, duration: 0.5, ease: 'power3.inOut' }, at);
        glow(c.fill.material, PAL.chunks[i], 0.7, at + 0.3, 0.15); glow(c.fill.material, PAL.chunks[i], 0.0, at + 0.6, 0.4);
      });
      // one chunk lifts and unfolds alone
      const k = chunks[3];
      tl.to(pad.scale, { x: 1, y: 1, z: 1, duration: 0.4, ease: 'back.out(2)' }, 3.9);
      tl.to(k.position, { y: 1.1, duration: 0.6, ease: 'back.out(1.4)' }, 4.0);
      tl.to(k.rotation, { y: 0.35, duration: 0.6, ease: 'power2.out' }, 4.0);
      tl.to(k.scale, { x: 1, y: 1, z: 1, duration: 0.35, ease: 'back.out(2.2)' }, 4.5);
      glow(k.fill.material, PAL.amber, 0.18, 4.5, 0.3); col(k.fill.material, PAL.amber, 4.5, 0.3);
      tl.to(k.position, { y: 1.3, duration: 1.2, yoyo: true, repeat: 3, ease: 'sine.inOut' }, 4.6);
      chunks.forEach((c, i) => { if (i !== 3) col(c.fill.material, new THREE.Color(PAL.chunks[i]).multiplyScalar(0.55).getHex(), 4.5, 0.6); });
      // camera: macro drift then settle on the open chunk
      st.cx = -4.5; st.cy = 2.0; st.cz = 9.5; st.tx = -1.0; st.ty = -0.3; st.tz = 0;
      tl.to(st, { cx: -0.8, cy: 2.6, cz: 7.8, tx: k.position.x + 0.4, ty: 0.3, duration: T, ease: 'none' }, 0);
