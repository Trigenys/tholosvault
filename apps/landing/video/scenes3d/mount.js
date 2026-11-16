      // Plug the archive like a drive: a cable grows, the screen lights up with files.
      const crate = V.box(3.6, 2.6, 2.8, 0x24365A); crate.position.set(-5.2, 0.1, 0); scene.add(crate);
      const lid = V.box(3.8, 0.35, 3.0, 0x3A4F7E); lid.position.set(-5.2, 1.55, 0); scene.add(lid);
      const port = V.box(0.5, 0.3, 0.2, PAL.amber, { glow: 0.8 }); port.position.set(-3.4, 0.0, 0.6); scene.add(port);
      const laptop = new THREE.Group(); laptop.position.set(3.4, -1.2, 0.4); laptop.rotation.y = -0.28; scene.add(laptop);
      const base = V.box(5.6, 0.22, 3.4, 0x1A2744); base.position.set(0, 0.11, 0); laptop.add(base);
      const screen = V.box(5.5, 3.4, 0.22, 0x16233C); screen.position.set(0, 2.0, -1.6); screen.rotation.x = -0.12; laptop.add(screen);
      const panel = new THREE.Mesh(new THREE.PlaneGeometry(5.1, 3.0), new THREE.MeshBasicMaterial({ color: 0x0E1830 }));
      panel.position.set(0, 2.0, -1.47); panel.rotation.x = -0.12; laptop.add(panel);
      const tiles = [];
      for (let i = 0; i < 12; i++) {
        const t = V.box(1.05, 0.72, 0.08, PAL.chunks[i % 8], { radius: 0.04, glow: 0.25 });
        const cx = -1.9 + (i % 4) * 1.27, cy = 0.95 - Math.floor(i / 4) * 0.95;
        t.position.set(cx, 2.0 + cy, -1.4 + cy * 0.12); t.rotation.x = -0.12; t.scale.setScalar(0.01);
        laptop.add(t); tiles.push(t);
      }
      // cable (tube revealed through drawRange)
      const curve = new THREE.CatmullRomCurve3([new THREE.Vector3(-3.2, 0.0, 0.7), new THREE.Vector3(-1.8, -0.95, 1.9), new THREE.Vector3(0.4, -1.05, 2.0), new THREE.Vector3(1.6, -0.6, 1.5), new THREE.Vector3(1.9, -0.3, 1.2)]);
      const tube = new THREE.Mesh(new THREE.TubeGeometry(curve, 72, 0.075, 8, false), new THREE.MeshBasicMaterial({ color: PAL.amber }));
      const total = tube.geometry.index.count; tube.geometry.setDrawRange(0, 0); scene.add(tube);
      const plug = V.orb(0.16, PAL.amber, 1.0); plug.position.copy(curve.getPoint(0)); plug.scale.setScalar(0.01); scene.add(plug);
      const prog = { p: 0 };
      tl.to(prog, { p: 1, duration: 1.4, ease: 'power2.inOut', onUpdate: () => { tube.geometry.setDrawRange(0, Math.floor(total * prog.p)); plug.position.copy(curve.getPoint(prog.p)); } }, 0.7);
      tl.to(plug.scale, { x: 1, y: 1, z: 1, duration: 0.3, ease: 'back.out(2)' }, 0.7);
      tl.to(plug.scale, { x: 1.6, y: 1.6, z: 1.6, duration: 0.18, yoyo: true, repeat: 1 }, 2.1);
      // screen wakes, tiles pop in a grid
      col(panel.material, 0x1C2E55, 2.2, 0.4);
      tiles.forEach((t, i) => tl.to(t.scale, { x: 1, y: 1, z: 1, duration: 0.35, ease: 'back.out(2.2)' }, 2.35 + i * 0.045));
      tiles.forEach((t, i) => tl.to(t.position, { y: t.position.y + 0.05, duration: 0.6, yoyo: true, repeat: 1, ease: 'sine.inOut' }, 3.4 + i * 0.03));
      // camera: slow pull-back
      st.cx = 2.4; st.cy = 3.4; st.cz = 9.5; st.tx = 0.6; st.ty = 0.6; st.tz = 0;
      tl.to(st, { cx: -0.4, cy: 3.0, cz: 13.2, tx: -0.4, ty: 0.3, duration: T, ease: 'none' }, 0);
