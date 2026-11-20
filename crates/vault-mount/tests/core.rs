//! Cœur de vault-mount sans pilote : arbre, attributs, parcours, lecture.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tempfile::TempDir;
use vault_core::{pack_dir, Archive, PackOptions};
use vault_mount::{FsError, NodeKind, VaultFs, ROOT_INO};

fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x & 0xff) as u8
        })
        .collect()
}

/// Arbre : fichier à la racine, sous-répertoires imbriqués, fichier vide, un
/// nom qui précède son homonyme-répertoire dans l'ordre des chemins (`b.txt`
/// avant `b/`), un gros fichier multi-morceaux.
fn fixture(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let files = vec![
        ("a.txt".to_string(), b"alpha\n".to_vec()),
        ("b.txt".to_string(), b"beta\n".to_vec()),
        ("b/inner.txt".to_string(), b"inner\n".to_vec()),
        ("b/deep/leaf.bin".to_string(), pseudo_random(300_000, 7)),
        ("vide".to_string(), Vec::new()),
        (
            "z/gros.bin".to_string(),
            pseudo_random(3 * 1024 * 1024 + 777, 9),
        ),
    ];
    for (rel, data) in &files {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }
    files
}

fn setup() -> (TempDir, PathBuf, Vec<(String, Vec<u8>)>) {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    let files = fixture(&src);
    let dst = tmp.path().join("t.vault");
    pack_dir(&src, &dst, &PackOptions::default()).unwrap();
    (tmp, dst, files)
}

fn names(fs: &VaultFs, ino: u64, offset: u64) -> Vec<(&str, NodeKind)> {
    fs.readdir(ino, offset)
        .unwrap()
        .map(|e| (e.name, e.kind))
        .collect()
}

#[test]
fn tree_exposes_implicit_dirs_sorted() {
    let (_tmp, vault, _) = setup();
    let fs = VaultFs::open(&vault).unwrap();

    let root = fs.getattr(ROOT_INO).unwrap();
    assert_eq!(root.kind, NodeKind::Dir);
    assert_eq!(root.nlink, 2 + 2, "deux sous-répertoires : b et z");

    assert_eq!(
        names(&fs, ROOT_INO, 0),
        vec![
            (".", NodeKind::Dir),
            ("..", NodeKind::Dir),
            ("a.txt", NodeKind::File),
            ("b", NodeKind::Dir),
            ("b.txt", NodeKind::File),
            ("vide", NodeKind::File),
            ("z", NodeKind::Dir),
        ]
    );
    // Reprise à un décalage : les entrées déjà servies sont omises.
    let all = fs.readdir(ROOT_INO, 0).unwrap().collect::<Vec<_>>();
    let resumed = names(&fs, ROOT_INO, all[3].next_offset);
    assert_eq!(resumed.first().map(|e| e.0), Some("b.txt"));

    let b = fs.lookup(ROOT_INO, "b").unwrap();
    assert_eq!(b.kind, NodeKind::Dir);
    assert_eq!(b.nlink, 3);
    assert_eq!(
        names(&fs, b.ino, 2),
        vec![("deep", NodeKind::Dir), ("inner.txt", NodeKind::File)]
    );
    // `..` d'un sous-répertoire pointe vers le parent.
    let deep = fs.lookup(b.ino, "deep").unwrap();
    let dotdot = fs.readdir(deep.ino, 1).unwrap().next().unwrap();
    assert_eq!((dotdot.name, dotdot.ino), ("..", b.ino));

    assert_eq!(
        fs.statfs().files,
        1 + 6 + 3,
        "racine, 6 fichiers, 3 répertoires"
    );
}

#[test]
fn attrs_and_errors() {
    let (_tmp, vault, files) = setup();
    let fs = VaultFs::open(&vault).unwrap();

    let gros = fs.resolve("z/gros.bin").unwrap();
    assert_eq!(gros.kind, NodeKind::File);
    assert_eq!(gros.size, files[5].1.len() as u64);
    assert_eq!(gros.nlink, 1);
    assert_ne!(gros.mtime, SystemTime::UNIX_EPOCH);
    assert_eq!(fs.resolve("vide").unwrap().size, 0);
    assert_eq!(fs.resolve("").unwrap().ino, ROOT_INO);

    assert!(matches!(
        fs.lookup(ROOT_INO, "absent"),
        Err(FsError::NotFound)
    ));
    assert!(matches!(fs.lookup(gros.ino, "x"), Err(FsError::NotDir)));
    assert!(matches!(
        fs.readdir(gros.ino, 0).map(|_| ()),
        Err(FsError::NotDir)
    ));
    assert!(matches!(
        fs.read(ROOT_INO, 0, &mut [0; 4]),
        Err(FsError::IsDir)
    ));
    assert!(matches!(fs.getattr(0), Err(FsError::NotFound)));
    assert!(matches!(fs.getattr(u64::MAX), Err(FsError::NotFound)));
}

#[test]
fn reads_match_content_by_pieces() {
    let (_tmp, vault, files) = setup();
    let fs = VaultFs::open(&vault).unwrap();

    for (rel, data) in &files {
        let attr = fs.resolve(rel).unwrap();
        // Requêtes de 128 KiB comme le noyau, puis une lecture au-delà de la fin.
        let mut got = Vec::with_capacity(data.len());
        let mut buf = vec![0u8; 128 * 1024];
        let mut off = 0u64;
        loop {
            let n = fs.read(attr.ino, off, &mut buf).unwrap();
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
            off += n as u64;
        }
        assert_eq!(&got, data, "{rel}");
        assert_eq!(
            fs.read(attr.ino, data.len() as u64 + 5, &mut buf).unwrap(),
            0
        );
    }

    // Lecture non alignée à cheval sur des morceaux.
    let gros = fs.resolve("z/gros.bin").unwrap();
    let data = &files[5].1;
    let mut buf = vec![0u8; 777_777];
    let n = fs.read(gros.ino, 123_456, &mut buf).unwrap();
    assert_eq!(n, buf.len());
    assert_eq!(&buf[..], &data[123_456..123_456 + n]);
}

#[test]
fn corrupt_chunk_is_reported_not_served() {
    let (_tmp, vault, _) = setup();
    let bad_start;
    {
        let ar = Archive::open(&vault).unwrap();
        let gros = ar.find("z/gros.bin").unwrap();
        bad_start = u64::from(ar.chunks()[gros.chunks[0] as usize].len);
        let chunk_offset = ar.chunks()[gros.chunks[1] as usize].offset as usize;
        drop(ar);
        let mut bytes = fs::read(&vault).unwrap();
        for i in 0..8 {
            bytes[chunk_offset + 4 + i] ^= 0xa5;
        }
        fs::write(&vault, bytes).unwrap();
    }
    let fs = VaultFs::open(&vault).unwrap();
    let gros = fs.resolve("z/gros.bin").unwrap();
    let mut buf = vec![0u8; 64 * 1024];
    assert_eq!(
        fs.read(gros.ino, 0, &mut buf).unwrap(),
        buf.len(),
        "morceau 0 intact"
    );
    assert!(matches!(
        fs.read(gros.ino, bad_start, &mut buf),
        Err(FsError::Corrupt(_))
    ));
}

#[test]
fn probe_without_driver_is_explicit() {
    // Sans adaptateur compilé, la sonde dit pourquoi et comment y remédier.
    let probe = vault_mount::probe();
    if cfg!(all(target_os = "linux", feature = "fuse")) {
        // Pilote compilé : la sonde réussit, ou dit précisément ce qui manque.
        if let Err(e) = &probe {
            assert!(e.to_string().contains("fusermount"), "{e}");
        }
    } else if cfg!(all(windows, feature = "winfsp")) {
        if let Err(e) = &probe {
            assert!(e.to_string().contains("winfsp.dev"), "{e}");
        }
    } else {
        let e = probe.expect_err("pas d'adaptateur compilé ici");
        assert!(e.to_string().contains("aucun adaptateur"), "{e}");
    }
}
