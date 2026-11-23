//! Chronométrage, percentiles et calibration de l'hôte.

use std::hint::black_box;
use std::time::Instant;

/// Nanosecondes écoulées pour `f`, et son résultat.
pub fn time_ns<T>(f: impl FnOnce() -> T) -> (u64, T) {
    let t = Instant::now();
    let r = f();
    (t.elapsed().as_nanos() as u64, r)
}

/// `iters` mesures indépendantes de `f`, en ns.
pub fn samples_ns<T>(iters: usize, mut f: impl FnMut() -> T) -> Vec<u64> {
    (0..iters)
        .map(|_| {
            let (ns, r) = time_ns(&mut f);
            black_box(r);
            ns
        })
        .collect()
}

/// Percentile au rang le plus proche sur un tableau non trié.
pub fn percentile(samples: &[u64], p: f64) -> u64 {
    assert!(!samples.is_empty());
    let mut s = samples.to_vec();
    s.sort_unstable();
    let rank = ((p / 100.0) * s.len() as f64).ceil() as usize;
    s[rank.clamp(1, s.len()) - 1]
}

pub fn median_f64(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut v = samples.to_vec();
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

pub fn median(samples: &[u64]) -> u64 {
    percentile(samples, 50.0)
}

/// Médiane de `n` répétitions de `f`.
pub fn median_of<T>(n: usize, f: impl FnMut() -> T) -> u64 {
    median(&samples_ns(n, f))
}

/// Référence CPU de l'hôte : BLAKE3 d'1 MiB en mémoire (médiane de 31 mesures).
/// Les seuils temporels s'expriment en ratio de cette valeur, pas en secondes.
pub fn calib_blake3_ns_per_mib() -> u64 {
    let buf = vec![0xA5u8; 1 << 20];
    median_of(31, || blake3::hash(black_box(&buf)))
}

/// Secondes par gigaoctet (10^9 octets) pour `ns` sur `bytes`.
pub fn s_per_gb(ns: u64, bytes: u64) -> f64 {
    ns as f64 / 1e9 / (bytes as f64 / 1e9)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentiles() {
        let s = [5, 1, 4, 2, 3];
        assert_eq!(percentile(&s, 50.0), 3);
        assert_eq!(percentile(&s, 99.0), 5);
        assert_eq!(percentile(&s, 0.0), 1);
        assert_eq!(percentile(&[7], 99.0), 7);
    }
}
