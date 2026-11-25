//! Non-régression de `tholos extract` : binaire réel, extraction complète et
//! sélective, refus des sélecteurs inconnus, aucun fichier tronqué laissé
//! après un morceau corrompu.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tholos(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tholos"))
        .args(args)
        .output()
        .expect("lancement de tholos")
}

fn run(args: &[&str]) -> Output {
    let a: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
    tholos(&a)
}

fn workdir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("extract-{name}"));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn noise(len: usize, seed: u64) -> Vec<u8> {
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

/// Arbre source + archive sans parité ; renvoie (répertoire, archive).
fn packed(name: &str) -> (PathBuf, PathBuf) {
    let d = workdir(name);
    let src = d.join("src");
    fs::create_dir_all(src.join("docs/sous")).unwrap();
    fs::write(src.join("gros.bin"), noise(2 * 1024 * 1024 + 17, 9)).unwrap();
    fs::write(src.join("docs/a.txt"), "alpha\n".repeat(1000)).unwrap();
    fs::write(src.join("docs/sous/b.txt"), "beta").unwrap();
    fs::write(src.join("docs-hors.txt"), "hors").unwrap();
    let archive = d.join("a.vault");
    let o = run(&[
        "pack",
        src.to_str().unwrap(),
        archive.to_str().unwrap(),
        "--parity",
        "0",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    (d, archive)
}

fn listing(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        for e in fs::read_dir(&p).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                stack.push(e.path());
            } else {
                let rel = e.path().strip_prefix(dir).unwrap().to_path_buf();
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn full_extraction_restores_the_tree() {
    let (d, archive) = packed("full");
    let out = d.join("out");
    let o = run(&["extract", archive.to_str().unwrap(), out.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        listing(&out),
        ["docs-hors.txt", "docs/a.txt", "docs/sous/b.txt", "gros.bin"]
    );
    for rel in ["gros.bin", "docs/a.txt", "docs/sous/b.txt", "docs-hors.txt"] {
        assert_eq!(
            fs::read(out.join(rel)).unwrap(),
            fs::read(d.join("src").join(rel)).unwrap(),
            "{rel}"
        );
    }
    assert!(String::from_utf8_lossy(&o.stdout).contains("4 fichier(s) extrait(s)"));
}

#[test]
fn selective_extraction_takes_files_and_directories_only() {
    let (d, archive) = packed("select");
    let out = d.join("out");
    let o = run(&[
        "extract",
        archive.to_str().unwrap(),
        out.to_str().unwrap(),
        "docs",
        "gros.bin",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    // `docs` ne capture pas `docs-hors.txt` : un répertoire, pas un préfixe de nom.
    assert_eq!(listing(&out), ["docs/a.txt", "docs/sous/b.txt", "gros.bin"]);
}

#[test]
fn unknown_selector_fails_before_writing_anything() {
    let (d, archive) = packed("unknown");
    let out = d.join("out");
    let o = run(&[
        "extract",
        archive.to_str().unwrap(),
        out.to_str().unwrap(),
        "docs",
        "absent",
    ]);
    assert!(!o.status.success());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("absent"), "{err}");
    assert!(!out.exists());
}

#[test]
fn corrupted_chunk_leaves_no_truncated_file() {
    let (d, archive) = packed("corrupt");
    // Corrompre un octet au milieu des frames de gros.bin (en-tête de 64 o,
    // gros.bin est le premier fichier non trivial écrit après `docs*`).
    let mut bytes = fs::read(&archive).unwrap();
    let mid = 64 + 40_000 + 1024 * 1024;
    bytes[mid] ^= 0xff;
    fs::write(&archive, &bytes).unwrap();

    let out = d.join("out");
    let o = run(&["extract", archive.to_str().unwrap(), out.to_str().unwrap()]);
    assert!(!o.status.success());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("gros.bin"), "{err}");
    assert!(err.contains("tholos verify"), "{err}");
    let files = listing(&out);
    assert!(!files.iter().any(|f| f.contains("gros.bin")), "{files:?}");
    assert!(files.contains(&"docs/a.txt".to_string()), "{files:?}");
}
