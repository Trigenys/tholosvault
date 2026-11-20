//! Liaison différée de winfsp-x64.dll (feature `winfsp`, Windows seulement).
//! `cargo:rustc-link-arg` ne se propage pas depuis vault-mount : chaque binaire
//! qui l'embarque doit l'émettre lui-même, sinon il ne démarre pas sans le pilote.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(all(windows, feature = "winfsp"))]
    winfsp::build::winfsp_link_delayload();
}
