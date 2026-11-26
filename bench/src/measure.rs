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

/// Nombre de tranches pour les percentiles de queue des seuils temporels.
pub const TAIL_ROUNDS: usize = 5;

/// Percentile robuste aux rafales de préemption : les échantillons, pris dans
/// l'ordre de mesure, sont découpés en `rounds` tranches contiguës ; on calcule
/// le percentile de chaque tranche et l'on retient leur médiane. Une rafale
/// d'interruptions de l'hôte (runner partagé) ne contamine qu'une tranche au
/// lieu de faire basculer la queue de toute la série. Avec `rounds` = 1, c'est
/// `percentile`.
pub fn percentile_median_of_rounds(samples: &[u64], p: f64, rounds: usize) -> u64 {
    assert!(!samples.is_empty());
    let rounds = rounds.clamp(1, samples.len());
    let size = samples.len().div_ceil(rounds);
    let per_round: Vec<u64> = samples.chunks(size).map(|c| percentile(c, p)).collect();
    median(&per_round)
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

    #[test]
    fn rounds_isolate_a_preemption_burst() {
        // 1000 mesures de 300 ns dont une rafale contiguë de 30 à 5400 ns :
        // le p99 global bascule sur la rafale, la médiane des tranches non.
        let mut s = vec![300u64; 1000];
        s[400..430].fill(5400);
        assert_eq!(percentile(&s, 99.0), 5400);
        assert_eq!(percentile_median_of_rounds(&s, 99.0, 5), 300);
    }

    #[test]
    fn rounds_keep_a_systematic_tail() {
        // Une queue lente répartie partout (régression réelle) reste visible.
        let s: Vec<u64> = (0..1000)
            .map(|i| if i % 50 == 0 { 5400 } else { 300 })
            .collect();
        assert_eq!(percentile_median_of_rounds(&s, 99.0, 5), 5400);
        assert_eq!(
            percentile_median_of_rounds(&s, 99.0, 1),
            percentile(&s, 99.0)
        );
    }
}
