//! Montage FUSE réel : exige /dev/fuse et fusermount3 (CI ubuntu).
#![cfg(all(target_os = "linux", feature = "fuse"))]

use std::fs;

use tempfile::TempDir;
use vault_core::{pack_dir, PackOptions};
use vault_mount::{mount, MountOptions, VaultFs};

#[test]
#[ignore = "requiert /dev/fuse et fusermount3"]
fn mount_lists_and_reads_through_the_kernel() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(src.join("d/e")).unwrap();
    let big: Vec<u8> = (0..2_500_000u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
        .collect();
    fs::write(src.join("a.txt"), b"alpha\n").unwrap();
    fs::write(src.join("d/e/big.bin"), &big).unwrap();
    fs::write(src.join("d/vide"), b"").unwrap();
    let vault = tmp.path().join("t.vault");
    pack_dir(&src, &vault, &PackOptions::default()).unwrap();

    let mnt = tmp.path().join("mnt");
    fs::create_dir(&mnt).unwrap();
    vault_mount::probe().expect("pilote FUSE requis pour ce test");
    let fs_ = VaultFs::open(&vault).unwrap();
    let handle = mount(fs_, &mnt, &MountOptions::default()).unwrap();
    assert!(handle.is_alive());

    let mut names: Vec<String> = fs::read_dir(&mnt)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["a.txt", "d"]);
    assert_eq!(fs::read(mnt.join("a.txt")).unwrap(), b"alpha\n");
    assert_eq!(fs::read(mnt.join("d/e/big.bin")).unwrap(), big);
    assert_eq!(fs::metadata(mnt.join("d/vide")).unwrap().len(), 0);
    assert!(fs::metadata(mnt.join("d")).unwrap().is_dir());
    assert!(fs::read(mnt.join("absent")).is_err());
    assert!(fs::write(mnt.join("a.txt"), b"x").is_err(), "lecture seule");

    handle.unmount().unwrap();
    assert!(fs::read_dir(&mnt).unwrap().next().is_none(), "démonté");
}
