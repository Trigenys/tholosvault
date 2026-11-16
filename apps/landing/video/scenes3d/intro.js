      // A sealed crate bursts, files pile up, the disk fills red.
      const crate = V.box(4.4, 3.0, 3.2, 0x24365A); crate.position.set(-2.6, 0.3, 0); scene.add(crate);
      const lid = V.box(4.7, 0.4, 3.5, 0x3A4F7E); lid.position.set(-2.6, 2.0, 0); scene.add(lid);
      const latch = V.box(0.6, 0.6, 0.2, PAL.amber, { glow: 0.5 }); latch.position.set(-2.6, 0.8, 1.7); scene.add(latch);
      const files = [];
      for (let i = 0; i < 40; i++) {
        const f = V.box(0.9, 1.2, 0.07, PAL.chunks[i % 8], { radius: 0.03 });
        f.position.set(-2.6, 0.6, 0); f.scale.setScalar(0.01);
        scene.add(f); files.push(f);
      }
      const disk = new THREE.Group();
      const dBody = new THREE.Mesh(new THREE.CylinderGeometry(1.15, 1.15, 0.5, 40), V.toon(0x2A3A5E, { emissive: 0x000000, emissiveIntensity: 0 }));
      dBody.castShadow = true; disk.add(dBody);
      const dLine = new THREE.Mesh(new THREE.CylinderGeometry(1.19, 1.19, 0.54, 40), new THREE.MeshBasicMaterial({ color: PAL.ink, side: THREE.BackSide })); disk.add(dLine);
      disk.position.set(5.2, -0.95, -1.2); scene.add(disk);
      const bar = V.box(5.2, 0.28, 0.3, 0x16233C); bar.position.set(1.4, 3.1, -2.4); scene.add(bar);
      const fill = V.box(5.0, 0.18, 0.2, 0x4C7BD9, { glow: 0.5 }); fill.position.x = 2.5;
      const pivot = new THREE.Group(); pivot.position.set(1.4 - 2.5, 3.1, -2.3); pivot.scale.x = 0.01; pivot.add(fill); scene.add(pivot);

      // camera
      st.cx = -1.0; st.cy = 5.0; st.cz = 17.5; st.tx = 0.8; st.ty = 0.4; st.tz = 0;
      tl.to(st, { cx: 1.4, cy: 3.2, cz: 12.5, tx: 1.6, ty: 0.2, duration: T, ease: 'none' }, 0);

      // lid flies, files burst into a pile
      tl.to(latch.scale, { x: 0.01, y: 0.01, z: 0.01, duration: 0.25, ease: 'power2.in' }, 0.9);
      tl.to(lid.position, { y: 2.6, x: -5.2, z: -2.6, duration: 0.7, ease: 'power3.out' }, 1.0);
      tl.to(lid.rotation, { z: -0.55, x: 0.2, duration: 0.7, ease: 'power3.out' }, 1.0);
      files.forEach((f, i) => {
        const r1 = rng(), r2 = rng(), r3 = rng();
        const tx = -0.6 + r1 * 6.2, tz = -1.6 + r2 * 3.4, ty = -1.2 + 0.6 + Math.floor(i / 8) * 0.09 + r3 * 0.1;
        const at = 1.15 + i * 0.07;
        tl.to(f.scale, { x: 1, y: 1, z: 1, duration: 0.2 }, at);
        tl.to(f.position, { y: 2.6 + r3 * 1.2, duration: 0.3, ease: 'power2.out' }, at);
        tl.to(f.position, { x: tx, z: tz, duration: 0.65, ease: 'power1.out' }, at);
        tl.to(f.position, { y: ty, duration: 0.4, ease: 'power2.in' }, at + 0.3);
        tl.to(f.rotation, { x: -Math.PI / 2 + (r1 - 0.5) * 0.3, z: (r2 - 0.5) * 1.6, duration: 0.7 }, at);
      });
      // progress bar crawls, disk turns red
      tl.to(pivot.scale, { x: 1, duration: 5.2, ease: 'power1.in' }, 1.6);
      col(fill.fill.material, PAL.red, 5.4, 1.0); glow(fill.fill.material, PAL.red, 0.9, 5.4, 1.0);
      col(dBody.material, PAL.red, 4.6, 1.8); glow(dBody.material, PAL.red, 0.8, 4.6, 1.8);
      tl.to(disk.position, { y: -0.9, duration: 0.08, yoyo: true, repeat: 7 }, 6.2);
