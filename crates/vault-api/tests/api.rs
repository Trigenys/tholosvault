//! API en lecture seule : routeur réel (sans réseau) sur des archives réelles.

use std::fs;

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt as _;
use vault_api::{router, ApiConfig};
use vault_core::{merkle, pack_dir, Archive, PackOptions};

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

/// Répertoire servi : `jeux.vault` (gros fichier de 2,5 Mio, texte, fichier
/// vide, sous-dossier), `notes.txt` (ignoré), `cassee.vault` (illisible).
fn served() -> (TempDir, Vec<u8>) {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(src.join("sous dossier")).unwrap();
    let big = pseudo_random(5 << 19, 3);
    fs::write(src.join("gros.bin"), &big).unwrap();
    fs::write(src.join("sous dossier/é.txt"), "bonjour\n".repeat(1000)).unwrap();
    fs::write(src.join("vide"), b"").unwrap();
    let root = tmp.path().join("servi");
    fs::create_dir_all(&root).unwrap();
    pack_dir(&src, &root.join("jeux.vault"), &PackOptions::default()).unwrap();
    fs::write(root.join("notes.txt"), "pas une archive").unwrap();
    fs::write(root.join("cassee.vault"), "pas une archive non plus").unwrap();
    (tmp, big)
}

fn app(tmp: &TempDir) -> Router {
    router(ApiConfig::new(tmp.path().join("servi")))
}

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    range: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(r) = range {
        req = req.header(header::RANGE, r);
    }
    let resp = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (parts, body) = resp.into_parts();
    let bytes = to_bytes(body, usize::MAX).await.unwrap().to_vec();
    (parts.status, parts.headers, bytes)
}

async fn get_json(app: &Router, uri: &str) -> (StatusCode, Value) {
    let (s, _, b) = send(app, Method::GET, uri, None).await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

fn root_of(tmp: &TempDir, name: &str) -> String {
    let ar = Archive::open(&tmp.path().join("servi").join(name)).unwrap();
    merkle::to_hex(ar.archive_root())
}

#[tokio::test]
async fn catalog_summary_and_listing() {
    let (tmp, big) = served();
    let app = app(&tmp);
    let (s, v) = get_json(&app, "/health").await;
    assert_eq!((s, v["status"].as_str()), (StatusCode::OK, Some("ok")));

    let (s, v) = get_json(&app, "/v1/archives").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["schema"], "tholos-api/1");
    let list = v["archives"].as_array().unwrap();
    // `notes.txt` ignoré ; l'archive illisible est signalée sans masquer l'autre.
    assert_eq!(list.len(), 2, "{v}");
    assert_eq!(list[0]["name"], "cassee.vault");
    assert!(list[0]["error"].as_str().unwrap().contains("tholos verify"));
    assert_eq!(list[1]["name"], "jeux.vault");
    assert_eq!(
        list[1]["archive_root"],
        root_of(&tmp, "jeux.vault").as_str()
    );

    let (s, v) = get_json(&app, "/v1/archives/jeux.vault").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["files"], 3);
    assert_eq!(
        v["logical_bytes"].as_u64().unwrap(),
        big.len() as u64 + 8000
    );

    let (s, v) = get_json(&app, "/v1/archives/jeux.vault/files").await;
    assert_eq!(s, StatusCode::OK);
    let paths: Vec<&str> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["gros.bin", "sous dossier/é.txt", "vide"]);
}

#[tokio::test]
async fn file_content_ranges_and_head() {
    let (tmp, big) = served();
    let app = app(&tmp);
    let uri = "/v1/archives/jeux.vault/files/gros.bin";

    let (s, h, b) = send(&app, Method::GET, uri, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b, big, "contenu complet (plusieurs blocs de 1 Mio)");
    assert_eq!(h[header::ACCEPT_RANGES], "bytes");
    assert!(h[header::ETAG].to_str().unwrap().starts_with('"'));

    let (s, h, b) = send(&app, Method::GET, uri, Some("bytes=1048570-1048589")).await;
    assert_eq!(s, StatusCode::PARTIAL_CONTENT);
    assert_eq!(b, &big[1048570..1048590]);
    assert_eq!(
        h[header::CONTENT_RANGE].to_str().unwrap(),
        format!("bytes 1048570-1048589/{}", big.len())
    );

    let (s, _, b) = send(&app, Method::GET, uri, Some("bytes=-10")).await;
    assert_eq!(s, StatusCode::PARTIAL_CONTENT);
    assert_eq!(b, &big[big.len() - 10..]);

    let (s, h, _) = send(&app, Method::GET, uri, Some("bytes=99999999-")).await;
    assert_eq!(s, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        h[header::CONTENT_RANGE].to_str().unwrap(),
        format!("bytes */{}", big.len())
    );

    // Plusieurs plages : ignorées (RFC 9110), fichier entier.
    let (s, _, b) = send(&app, Method::GET, uri, Some("bytes=0-1,5-6")).await;
    assert_eq!((s, b.len()), (StatusCode::OK, big.len()));

    let (s, h, b) = send(&app, Method::HEAD, uri, None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(b.is_empty());
    assert_eq!(h[header::CONTENT_LENGTH], big.len().to_string().as_str());

    // Chemin à espace et accent, encodé en pourcentage.
    let (s, _, b) = send(
        &app,
        Method::GET,
        "/v1/archives/jeux.vault/files/sous%20dossier/%C3%A9.txt",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b, "bonjour\n".repeat(1000).as_bytes());

    let (s, _, b) = send(
        &app,
        Method::GET,
        "/v1/archives/jeux.vault/files/vide",
        None,
    )
    .await;
    assert_eq!((s, b.len()), (StatusCode::OK, 0));
}

#[tokio::test]
async fn names_outside_the_root_and_missing_items_are_refused() {
    let (tmp, _) = served();
    fs::write(tmp.path().join("dehors.vault"), "x").unwrap();
    let app = app(&tmp);
    for (uri, want) in [
        ("/v1/archives/..%2Fdehors.vault", StatusCode::BAD_REQUEST),
        ("/v1/archives/notes.txt", StatusCode::BAD_REQUEST),
        ("/v1/archives/absente.vault", StatusCode::NOT_FOUND),
        (
            "/v1/archives/jeux.vault/files/absent",
            StatusCode::NOT_FOUND,
        ),
        (
            "/v1/archives/cassee.vault",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        ("/inconnue", StatusCode::NOT_FOUND),
    ] {
        let (s, v) = get_json(&app, uri).await;
        assert_eq!(s, want, "{uri} : {v}");
        assert!(v["error"].as_str().is_some_and(|m| !m.is_empty()), "{uri}");
    }
}

#[tokio::test]
async fn verify_reports_damage_and_corrupt_content_is_refused() {
    let (tmp, _) = served();
    let app = app(&tmp);
    let (s, v) = get_json(&app, "/v1/archives/jeux.vault/verify").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (v["ok"].as_bool(), v["mode"].as_str()),
        (Some(true), Some("full"))
    );
    let (_, v) = get_json(&app, "/v1/archives/jeux.vault/verify?quick=true&sample=2").await;
    assert_eq!(
        (v["ok"].as_bool(), v["mode"].as_str()),
        (Some(true), Some("quick"))
    );
    assert_eq!(v["chunks_checked"], 2);

    // Un octet abîmé dans le premier morceau de gros.bin, posé par
    // remplacement (renommage) pendant que le service tourne.
    let path = tmp.path().join("servi/jeux.vault");
    let off = {
        let ar = Archive::open(&path).unwrap();
        let first = ar.find("gros.bin").unwrap().chunks[0];
        ar.chunks()[first as usize].offset + 10
    };
    let mut bytes = fs::read(&path).unwrap();
    bytes[off as usize] ^= 0xff;
    let tmp_file = path.with_extension("tmp");
    fs::write(&tmp_file, &bytes).unwrap();
    fs::rename(&tmp_file, &path).unwrap();

    let (s, v) = get_json(&app, "/v1/archives/jeux.vault/verify").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["ok"], false, "{v}");
    assert_eq!(v["repairable"], true);
    assert_eq!(v["bad_chunks"].as_array().unwrap().len(), 1);

    let (s, v) = get_json(&app, "/v1/archives/jeux.vault/files/gros.bin").await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["error"].as_str().unwrap().contains("tholos repair"));
}

#[tokio::test]
async fn a_replaced_archive_is_seen_at_the_next_request() {
    let (tmp, _) = served();
    let app = app(&tmp);
    let before = root_of(&tmp, "jeux.vault");
    let (_, v) = get_json(&app, "/v1/archives/jeux.vault").await;
    assert_eq!(v["archive_root"], before.as_str());

    let src = tmp.path().join("src");
    fs::write(src.join("ajout.txt"), "nouvelle version").unwrap();
    pack_dir(
        &src,
        &tmp.path().join("servi/jeux.vault"),
        &PackOptions::default(),
    )
    .unwrap();
    let after = root_of(&tmp, "jeux.vault");
    assert_ne!(before, after);
    let (_, v) = get_json(&app, "/v1/archives/jeux.vault").await;
    assert_eq!(v["archive_root"], after.as_str());
    assert_eq!(v["files"], 4);
}
