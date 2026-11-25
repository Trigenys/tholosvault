//! `tholos fetch` depuis une source locale (chemin ou `file://`), de bout en bout.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tholos<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(args: I) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tholos"))
        .args(args)
        .output()
        .expect("lancement de tholos")
}

fn packed(name: &str) -> (PathBuf, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    let src = dir.join("src");
    fs::create_dir_all(src.join("sous")).unwrap();
    let data: Vec<u8> = (0..2_000_000u32).map(|i| (i * 7 % 251) as u8).collect();
    fs::write(src.join("donnees.bin"), &data).unwrap();
    fs::write(src.join("sous/note.txt"), "bonjour\n".repeat(1000)).unwrap();
    let archive = dir.join("source.vault");
    let out = tholos([OsStr::new("pack"), src.as_os_str(), archive.as_os_str()]);
    assert!(out.status.success(), "{out:?}");
    (dir, archive)
}

#[test]
fn fetch_from_path_then_verify() {
    let (dir, archive) = packed("fetch_local_path");
    let dest = dir.join("copie.vault");
    let out = tholos([
        OsStr::new("fetch"),
        archive.as_os_str(),
        OsStr::new("--out"),
        dest.as_os_str(),
        OsStr::new("--quiet"),
    ]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(fs::read(&dest).unwrap(), fs::read(&archive).unwrap());
    let out = tholos([OsStr::new("verify"), dest.as_os_str()]);
    assert!(out.status.success(), "{out:?}");
}

#[test]
fn fetch_from_file_url_with_expected_root() {
    let (dir, archive) = packed("fetch_local_url");
    let list = tholos([OsStr::new("list"), archive.as_os_str()]);
    let text = String::from_utf8(list.stdout).unwrap();
    let root = text
        .lines()
        .find_map(|l| l.strip_prefix("racine "))
        .unwrap()
        .to_string();
    let abs = fs::canonicalize(&archive).unwrap();
    let url = format!(
        "file://{}{}",
        if cfg!(windows) { "/" } else { "" },
        abs.display()
            .to_string()
            .replace('\\', "/")
            .trim_start_matches("//?/")
    );
    let dest = dir.join("copie.vault");
    let out = tholos([
        "fetch",
        &url,
        "--out",
        dest.to_str().unwrap(),
        "--root",
        &root,
        "--quiet",
    ]);
    assert!(out.status.success(), "{url} {out:?}");
    assert_eq!(fs::read(&dest).unwrap(), fs::read(&archive).unwrap());
}

#[test]
fn fetch_refuses_to_overwrite_its_source() {
    let (_dir, archive) = packed("fetch_local_same");
    let before = fs::read(&archive).unwrap();
    let out = tholos([
        OsStr::new("fetch"),
        archive.as_os_str(),
        OsStr::new("--out"),
        archive.as_os_str(),
        OsStr::new("--restart"),
    ]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("fichier source"), "{err}");
    assert_eq!(
        fs::read(&archive).unwrap(),
        before,
        "la source doit rester intacte"
    );
}
