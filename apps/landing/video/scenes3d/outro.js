      // One archive, one cut, four uses orbiting it.
      const hero = new THREE.Group(); hero.position.set(0, 1.0, 0); scene.add(hero);
      const core = V.box(3.0, 3.0, 3.0, 0x1C2A44, { radius: 0.22 }); hero.add(core);
      const seams = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(3.02, 3.02, 3.02)), new THREE.LineBasicMaterial({ color: PAL.amber, transparent: true, opacity: 0.0 })); hero.add(seams);
      const cuts = [0.7, 1.3, 2.2];
      cuts.forEach(c => { const s = V.box(0.06, 3.1, 3.1, PAL.amber, { glow: 1.0, radius: 0.01, outline: 0 }); s.position.x = -1.5 + c; s.scale.y = 0.01; hero.add(s); s.cut = true; });
      const mk = (geo, color) => { const g = new THREE.Group(); const f = new THREE.Mesh(geo, V.toon(color, { emissive: color, emissiveIntensity: 0.18 })); f.castShadow = true; g.add(f); g.fill = f; return g; };
      const ring = new THREE.Group(); ring.position.y = 1.0; scene.add(ring);
      const sats = [mk(new THREE.TorusGeometry(0.42, 0.16, 12, 32), PAL.chunks[0]), mk(new THREE.SphereGeometry(0.5, 24, 16), PAL.green), mk(new THREE.OctahedronGeometry(0.6), PAL.cyan), mk(new THREE.ConeGeometry(0.5, 0.95, 24), PAL.chunks[2])];
      const spokes = [];
      sats.forEach((s, i) => { const a = i * Math.PI / 2 + 0.6; s.position.set(Math.cos(a) * 4.6, Math.sin(i * 1.7) * 0.6, Math.sin(a) * 4.6); s.scale.setScalar(0.01); ring.add(s); const r = V.rod(PAL.amber, 0.025); r.material.opacity = 0; ring.add(r); spokes.push(r); });
      const upd = () => { sats.forEach((s, i) => { spokes[i].setEnds(new THREE.Vector3(0, 0, 0), s.position); }); };
      upd();
      // hero turns, cuts light up, satellites pop and orbit
      tl.to(hero.rotation, { y: 1.3, duration: T, ease: 'none' }, 0);
      tl.to(hero.position, { y: 1.25, duration: 3.5, yoyo: true, repeat: 1, ease: 'sine.inOut' }, 0);
      tl.to(seams.material, { opacity: 0.9, duration: 0.8 }, 0.6);
      hero.children.filter(c => c.cut).forEach((s, i) => tl.to(s.scale, { y: 1.02, duration: 0.35, ease: 'power3.out' }, 1.0 + i * 0.2));
      sats.forEach((s, i) => { tl.to(s.scale, { x: 1, y: 1, z: 1, duration: 0.45, ease: 'back.out(2)' }, 2.0 + i * 0.22); tl.to(spokes[i].material, { opacity: 0.7, duration: 0.3 }, 2.1 + i * 0.22); tl.to(s.rotation, { y: Math.PI * 2, x: 0.8, duration: T, ease: 'none' }, 0); });
      tl.to(ring.rotation, { y: 1.1, duration: T, ease: 'none', onUpdate: upd }, 0);
      // camera: slow orbit that eases into a hold
      st.cx = 7.5; st.cy = 3.6; st.cz = 9.0; st.tx = 0; st.ty = 1.0; st.tz = 0;
      tl.to(st, { cx: 2.0, cy: 3.0, cz: 11.8, duration: T - 0.6, ease: 'power1.out' }, 0);
