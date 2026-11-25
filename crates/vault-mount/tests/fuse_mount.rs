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

#[cfg(target_os = "linux")]
mod sys {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_void};
    use std::path::Path;

    extern "C" {
        fn getxattr(
            path: *const c_char,
            name: *const c_char,
            value: *mut c_void,
            size: usize,
        ) -> isize;
        fn listxattr(path: *const c_char, list: *mut c_char, size: usize) -> isize;
    }

    fn cpath(p: &Path) -> CString {
        CString::new(p.as_os_str().as_encoded_bytes()).unwrap()
    }

    /// `getxattr(2)` : `Err(errno)` si l'appel échoue.
    pub fn get(p: &Path, name: &str) -> Result<Vec<u8>, i32> {
        let (path, name) = (cpath(p), CString::new(name).unwrap());
        let mut buf = vec![0u8; 256];
        let n = unsafe {
            getxattr(
                path.as_ptr(),
                name.as_ptr(),
                buf.as_mut_ptr().cast(),
                buf.len(),
            )
        };
        if n < 0 {
            return Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(0));
        }
        buf.truncate(n as usize);
        Ok(buf)
    }

    pub fn list(p: &Path) -> Vec<String> {
        let path = cpath(p);
        let mut buf = vec![0u8; 1024];
        let n = unsafe { listxattr(path.as_ptr(), buf.as_mut_ptr().cast(), buf.len()) };
        assert!(n >= 0, "{}", std::io::Error::last_os_error());
        buf.truncate(n as usize);
        buf.split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8(s.to_vec()).unwrap())
            .collect()
    }
}

#[test]
#[ignore = "requiert /dev/fuse et fusermount3"]
fn mount_exposes_merkle_roots_as_xattrs() {
    use vault_core::merkle::to_hex;
    use vault_core::Archive;

    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(src.join("d")).unwrap();
    fs::write(src.join("d/f.txt"), "contenu\n".repeat(10_000)).unwrap();
    let vault = tmp.path().join("x.vault");
    pack_dir(&src, &vault, &PackOptions::default()).unwrap();
    let ar = Archive::open(&vault).unwrap();
    let file_root = to_hex(&ar.find("d/f.txt").unwrap().root);
    let archive_root = to_hex(ar.archive_root());
    drop(ar);

    let mnt = tmp.path().join("mnt");
    fs::create_dir(&mnt).unwrap();
    vault_mount::probe().expect("pilote FUSE requis pour ce test");
    let handle = mount(
        VaultFs::open(&vault).unwrap(),
        &mnt,
        &MountOptions::default(),
    )
    .unwrap();

    let f = mnt.join("d/f.txt");
    assert_eq!(
        sys::get(&f, "user.tholos.root").unwrap(),
        file_root.as_bytes()
    );
    assert_eq!(
        sys::get(&mnt, "user.tholos.archive_root").unwrap(),
        archive_root.as_bytes()
    );
    assert_eq!(
        sys::list(&f),
        [
            "user.tholos.root",
            "user.tholos.chunks",
            "user.tholos.archive_root"
        ]
    );
    // Attribut absent : ENODATA (61 sous Linux).
    assert_eq!(sys::get(&f, "user.absent"), Err(61));

    handle.unmount().unwrap();
}
