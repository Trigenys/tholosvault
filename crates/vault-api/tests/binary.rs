//! Bout en bout : binaire `tholos-api` réel, port TCP éphémère, requête HTTP brute.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};

use vault_core::{pack_dir, PackOptions};

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn http_get(addr: &str, path: &str) -> String {
    let mut s = TcpStream::connect(addr).unwrap();
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

#[test]
fn serves_a_directory_over_tcp() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("a.txt"), "contenu servi\n").unwrap();
    let root = tmp.path().join("servi");
    std::fs::create_dir_all(&root).unwrap();
    pack_dir(&src, &root.join("a.vault"), &PackOptions::default()).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_tholos-api"))
        .arg(&root)
        .args(["--listen", "127.0.0.1:0"])
        .stderr(Stdio::piped())
        .spawn()
        .expect("lancement de tholos-api");
    let stderr = child.stderr.take().unwrap();
    let _server = Server(child);
    let mut line = String::new();
    BufReader::new(stderr).read_line(&mut line).unwrap();
    let addr = line
        .split("http://")
        .nth(1)
        .and_then(|r| r.split_whitespace().next())
        .unwrap_or_else(|| panic!("adresse absente de : {line}"))
        .to_string();

    let r = http_get(&addr, "/v1/archives/a.vault/files/a.txt");
    assert!(r.starts_with("HTTP/1.1 200"), "{r}");
    assert!(r.ends_with("contenu servi\n"), "{r}");
    let r = http_get(&addr, "/v1/archives");
    assert!(r.contains("\"a.vault\""), "{r}");
}

#[test]
fn refuses_a_missing_directory() {
    let o = Command::new(env!("CARGO_BIN_EXE_tholos-api"))
        .arg("/nexiste/pas")
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("n'est pas un répertoire"));
}
