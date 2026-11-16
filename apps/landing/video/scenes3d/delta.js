      // Two versions side by side: only the three changed chunks travel.
      const changed = [1, 4, 6], left = [], right = [], movers = [];
      const pos = (cx, i) => new THREE.Vector3(cx - 2.0 + (i % 4) * 1.35, 1.55 - Math.floor(i / 4) * 1.35, 0);
      const base = V.box(6.0, 0.25, 2.2, 0x16233C); base.position.set(-3.8, -1.1, 0); scene.add(base);
      const base2 = base.clone(); base2.position.x = 3.8; scene.add(base2);
      for (let i = 0; i < 8; i++) {
        const a = V.box(1.1, 1.1, 0.6, PAL.chunks[i]); a.position.copy(pos(-3.8, i)); scene.add(a); left.push(a);
        const isNew = changed.includes(i);
        const b = V.box(1.1, 1.1, 0.6, isNew ? PAL.amber : PAL.chunks[i], { glow: isNew ? 0.3 : 0 }); b.position.copy(pos(3.8, i)); scene.add(b); right.push(b);
        if (isNew) { b.scale.setScalar(0.01); const m = V.box(1.1, 1.1, 0.6, PAL.amber, { glow: 0.35 }); m.position.copy(pos(-3.8, i)); m.scale.setScalar(0.01); scene.add(m); movers.push({ m, i }); }
        else { const g = new THREE.Mesh(new THREE.BoxGeometry(1.14, 1.14, 0.64), new THREE.MeshBasicMaterial({ color: PAL.ink, transparent: true, opacity: 0.0 })); g.position.copy(b.position); scene.add(g); b.dim = g; }
      }
      // mark the changed chunks on the left
      changed.forEach((i, k) => { const a = left[i]; col(a.fill.material, PAL.amber, 1.0 + k * 0.1, 0.3); glow(a.fill.material, PAL.amber, 0.35, 1.0 + k * 0.1, 0.3); tl.to(a.position, { z: 0.35, duration: 0.3, ease: 'back.out(2)' }, 1.0 + k * 0.1); });
      // copies fly across in an arc, land, and the right side fills in
      movers.forEach(({ m, i }, k) => {
        const from = pos(-3.8, i), to = pos(3.8, i), at = 1.9 + k * 0.35;
        tl.to(m.scale, { x: 1, y: 1, z: 1, duration: 0.15 }, at);
        tl.to(m.position, { x: to.x, duration: 1.0, ease: 'power1.inOut' }, at);
        tl.to(m.position, { y: from.y + 2.4, duration: 0.5, ease: 'power2.out' }, at);
        tl.to(m.position, { y: to.y, duration: 0.5, ease: 'power2.in' }, at + 0.5);
        tl.to(m.rotation, { y: Math.PI * 2, duration: 1.0, ease: 'power1.inOut' }, at);
        tl.to(m.scale, { x: 0.01, y: 0.01, z: 0.01, duration: 0.01 }, at + 1.0);
        tl.to(right[i].scale, { x: 1, y: 1, z: 1, duration: 0.4, ease: 'back.out(2.5)' }, at + 1.0);
        glow(left[i].fill.material, PAL.amber, 0.0, at + 1.0, 0.4); tl.to(left[i].position, { z: 0, duration: 0.3 }, at + 1.0);
      });
      right.forEach(b => { if (b.dim) { tl.to(b.dim.material, { opacity: 0.45, duration: 0.5 }, 1.0); tl.to(b.dim.material, { opacity: 0.0, duration: 0.5 }, 4.3); } });
      // camera: lateral move, left to right
      st.cx = -2.5; st.cy = 2.6; st.cz = 11.5; st.tx = -1.2; st.ty = 0.8; st.tz = 0;
      tl.to(st, { cx: 2.2, cy: 2.4, cz: 10.8, tx: 1.2, ty: 0.7, duration: T, ease: 'none' }, 0);
