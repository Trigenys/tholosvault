//! Liaison différée de winfsp-x64.dll (feature `winfsp`, Windows seulement) :
//! le binaire démarre sans le pilote et `probe()` signale son absence.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(all(windows, feature = "winfsp"))]
    winfsp::build::winfsp_link_delayload();
}
