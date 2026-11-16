//! Générateur xorshift64* maison : la reproductibilité du corpus ne doit pas
//! dépendre de l'algorithme d'une dépendance externe.

#[derive(Debug, Clone)]
pub struct Xorshift(u64);

impl Xorshift {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Entier uniforme dans `0..n` (`n > 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    /// Entier dans `lo..=hi` à distribution log-uniforme.
    pub fn log_uniform(&mut self, lo: u64, hi: u64) -> u64 {
        let u = self.unit();
        let v = (lo as f64).ln() + u * ((hi as f64).ln() - (lo as f64).ln());
        (v.exp().round() as u64).clamp(lo, hi)
    }

    /// Réel dans `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Rang dans `0..n` à distribution de type Zipf (exposant ~1) : le rang 0
    /// est le plus fréquent.
    pub fn zipf(&mut self, n: usize) -> usize {
        let u = self.unit();
        (((n as f64 + 1.0).powf(u)) as usize)
            .saturating_sub(1)
            .min(n - 1)
    }

    pub fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let bytes = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Xorshift::new(42);
        let mut b = Xorshift::new(42);
        let mut c = Xorshift::new(43);
        let sa: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let sb: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        let sc: Vec<u64> = (0..8).map(|_| c.next_u64()).collect();
        assert_eq!(sa, sb);
        assert_ne!(sa, sc);
    }

    /// Valeur dorée : un changement d'algorithme change le corpus et doit être
    /// assumé explicitement.
    #[test]
    fn golden_value() {
        let mut r = Xorshift::new(1);
        assert_eq!(r.next_u64(), GOLDEN);
    }

    const GOLDEN: u64 = 973_819_730_272_012_410;
}
