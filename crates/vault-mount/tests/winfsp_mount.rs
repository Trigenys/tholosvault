//! Montage WinFsp réel : exige le pilote WinFsp installé (CI windows).
#![cfg(all(windows, feature = "winfsp"))]

use std::fs;

use tempfile::TempDir;
use vault_core::{pack_dir, PackOptions};
use vault_mount::{mount, MountOptions, VaultFs};

#[test]
#[ignore = "requiert le pilote WinFsp (winfsp.dev)"]
fn mount_lists_and_reads_through_the_kernel() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(src.join("d/e")).unwrap();
    let big: Vec<u8> = (0..2_500_000u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
        .collect();
    fs::write(
        src.join("a.txt"),
        b"alpha
",
    )
    .unwrap();
    fs::write(src.join("d/e/big.bin"), &big).unwrap();
    fs::write(src.join("d/vide"), b"").unwrap();
    let vault = tmp.path().join("t.vault");
    pack_dir(&src, &vault, &PackOptions::default()).unwrap();

    let mnt = tmp.path().join("mnt");
    fs::create_dir(&mnt).unwrap();
    vault_mount::probe().expect("pilote WinFsp requis pour ce test");
    let fs_ = VaultFs::open(&vault).unwrap();
    let handle = mount(fs_, &mnt, &MountOptions::default()).unwrap();
    assert!(handle.is_alive());

    let mut names: Vec<String> = fs::read_dir(&mnt)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["a.txt", "d"]);
    assert_eq!(
        fs::read(mnt.join("a.txt")).unwrap(),
        b"alpha
"
    );
    assert_eq!(fs::read(mnt.join("d/e/big.bin")).unwrap(), big);
    assert_eq!(fs::metadata(mnt.join("d/vide")).unwrap().len(), 0);
    assert!(fs::metadata(mnt.join("d")).unwrap().is_dir());
    // Volume insensible à la casse, casse d'origine préservée.
    assert_eq!(
        fs::read(mnt.join("A.TXT")).unwrap(),
        b"alpha
"
    );
    assert!(fs::read(mnt.join("absent")).is_err());
    assert!(fs::write(mnt.join("a.txt"), b"x").is_err(), "lecture seule");
    assert!(
        fs::write(mnt.join("nouveau.txt"), b"x").is_err(),
        "lecture seule"
    );

    handle.unmount().unwrap();
    // WinFsp supprime le dossier de montage qu'il a pris en charge.
    let empty = fs::read_dir(&mnt)
        .map(|mut d| d.next().is_none())
        .unwrap_or(true);
    assert!(empty, "démonté");
}
