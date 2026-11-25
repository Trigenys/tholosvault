//! Attributs étendus du cœur de montage, sans pilote : racines Merkle lues
//! dans l'index, cohérentes avec `vault-core` et stables d'une archive à l'autre.

use std::fs;

use tempfile::TempDir;
use vault_core::merkle::to_hex;
use vault_core::{pack_dir, Archive, PackOptions};
use vault_mount::{FsError, VaultFs, ROOT_INO, XATTR_ARCHIVE_ROOT, XATTR_CHUNKS, XATTR_ROOT};

fn pack(tmp: &TempDir, name: &str, extra: Option<&str>) -> std::path::PathBuf {
    let src = tmp.path().join(format!("src-{name}"));
    fs::create_dir_all(src.join("d")).unwrap();
    fs::write(src.join("d/commun.txt"), "même contenu\n".repeat(20_000)).unwrap();
    fs::write(src.join("vide"), b"").unwrap();
    if let Some(e) = extra {
        fs::write(src.join("en-plus.txt"), e).unwrap();
    }
    let out = tmp.path().join(format!("{name}.vault"));
    pack_dir(&src, &out, &PackOptions::default()).unwrap();
    out
}

#[test]
fn file_and_archive_roots_come_from_the_index() {
    let tmp = TempDir::new().unwrap();
    let path = pack(&tmp, "a", None);
    let ar = Archive::open(&path).unwrap();
    let expected_file = ar.find("d/commun.txt").unwrap().clone();
    let archive_root = to_hex(ar.archive_root());
    let fs_ = VaultFs::open(&path).unwrap();

    let ino = fs_.resolve("d/commun.txt").unwrap().ino;
    assert_eq!(
        fs_.xattr_names(ino).unwrap(),
        [XATTR_ROOT, XATTR_CHUNKS, XATTR_ARCHIVE_ROOT]
    );
    assert_eq!(
        fs_.xattr(ino, XATTR_ROOT).unwrap(),
        Some(to_hex(&expected_file.root))
    );
    assert_eq!(
        fs_.xattr(ino, XATTR_CHUNKS).unwrap(),
        Some(expected_file.chunks.len().to_string())
    );
    assert_eq!(
        fs_.xattr(ino, XATTR_ARCHIVE_ROOT).unwrap(),
        Some(archive_root.clone())
    );

    // Répertoires (racine comprise) : seulement la racine d'archive.
    for dir in [ROOT_INO, fs_.resolve("d").unwrap().ino] {
        assert_eq!(fs_.xattr_names(dir).unwrap(), [XATTR_ARCHIVE_ROOT]);
        assert_eq!(
            fs_.xattr(dir, XATTR_ARCHIVE_ROOT).unwrap(),
            Some(archive_root.clone())
        );
        assert_eq!(fs_.xattr(dir, XATTR_ROOT).unwrap(), None);
    }
    // Attribut inconnu, inœud inconnu.
    assert_eq!(fs_.xattr(ino, "user.autre").unwrap(), None);
    assert!(matches!(
        fs_.xattr(9_999, XATTR_ROOT),
        Err(FsError::NotFound)
    ));
    assert!(matches!(fs_.xattr_names(9_999), Err(FsError::NotFound)));
}

#[test]
fn same_content_same_root_across_archives() {
    let tmp = TempDir::new().unwrap();
    let a = VaultFs::open(&pack(&tmp, "a", None)).unwrap();
    let b = VaultFs::open(&pack(&tmp, "b", Some("nouveau\n"))).unwrap();
    let root_in = |fs_: &VaultFs, p: &str| {
        let ino = fs_.resolve(p).unwrap().ino;
        fs_.xattr(ino, XATTR_ROOT).unwrap().unwrap()
    };
    // Fichier inchangé : même racine ; archives différentes : racines différentes.
    assert_eq!(root_in(&a, "d/commun.txt"), root_in(&b, "d/commun.txt"));
    assert_eq!(root_in(&a, "vide"), root_in(&b, "vide"));
    assert_ne!(
        a.xattr(ROOT_INO, XATTR_ARCHIVE_ROOT).unwrap(),
        b.xattr(ROOT_INO, XATTR_ARCHIVE_ROOT).unwrap()
    );
}
