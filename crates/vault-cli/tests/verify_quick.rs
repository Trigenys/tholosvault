//! Non-régression de `tholos verify --quick` : binaire réel, archive saine,
//! dégât vu par le CRC des shards, option `--sample` liée à `--quick`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tholos"))
        .args(args)
        .output()
        .expect("lancement de tholos")
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

fn packed(name: &str, parity: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("verify-quick-{name}"));
    let _ = fs::remove_dir_all(&d);
    let src = d.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("gros.bin"), noise(3 * 1024 * 1024, 5)).unwrap();
    fs::write(src.join("petit.txt"), "petit").unwrap();
    let archive = d.join("a.vault");
    let o = run(&[
        "pack",
        src.to_str().unwrap(),
        archive.to_str().unwrap(),
        "--parity",
        parity,
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    archive
}

fn text(o: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn quick_on_healthy_archive_reports_sample_and_crc() {
    let a = packed("sain", "5");
    let o = run(&["verify", a.to_str().unwrap(), "--quick", "--sample", "2"]);
    let (out, err) = text(&o);
    assert!(o.status.success(), "{err}");
    assert!(
        out.contains("OK (rapide) : 2 morceaux décompressés"),
        "{out}"
    );
    assert!(out.contains("CRC de tous les shards conforme"), "{out}");
}

#[test]
fn quick_without_parity_states_its_limit() {
    let a = packed("sans-parite", "0");
    let o = run(&["verify", a.to_str().unwrap(), "--quick", "--sample", "1"]);
    let (out, err) = text(&o);
    assert!(o.status.success(), "{err}");
    assert!(out.contains("hors échantillon non contrôlé"), "{out}");
}

#[test]
fn quick_sees_damage_through_crc_and_escalates() {
    let a = packed("abime", "5");
    let mut b = fs::read(&a).unwrap();
    b[64 + 1024 * 1024] ^= 0xff;
    fs::write(&a, b).unwrap();
    let o = run(&["verify", a.to_str().unwrap(), "--quick", "--sample", "0"]);
    let (_, err) = text(&o);
    assert!(!o.status.success());
    assert!(err.contains("vérification complète effectuée"), "{err}");
    assert!(err.contains("morceau"), "{err}");
    assert!(err.contains("réparable"), "{err}");
}

#[test]
fn sample_requires_quick() {
    let a = packed("option", "5");
    let o = run(&["verify", a.to_str().unwrap(), "--sample", "3"]);
    assert!(!o.status.success());
    assert!(text(&o).1.contains("--quick"));
}
