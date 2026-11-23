//! Limiteur de débit partagé entre connexions : seau à jetons continu avec
//! dette autorisée (un gros bloc est accepté puis « remboursé » en dormant).

use std::sync::Mutex;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug)]
pub struct RateLimiter {
    bytes_per_sec: f64,
    state: Mutex<Bucket>,
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(bytes_per_sec: u64) -> Self {
        let rate = bytes_per_sec.max(1) as f64;
        Self {
            bytes_per_sec: rate,
            state: Mutex::new(Bucket {
                tokens: rate,
                last: Instant::now(),
            }),
        }
    }

    /// Décompte `n` octets ; dort le temps nécessaire si le seau est à découvert.
    pub async fn acquire(&self, n: usize) {
        let wait = {
            let mut b = self.state.lock().unwrap();
            let now = Instant::now();
            let elapsed = now.duration_since(b.last).as_secs_f64();
            b.last = now;
            b.tokens = (b.tokens + elapsed * self.bytes_per_sec).min(self.bytes_per_sec);
            b.tokens -= n as f64;
            if b.tokens < 0.0 {
                Duration::from_secs_f64(-b.tokens / self.bytes_per_sec)
            } else {
                Duration::ZERO
            }
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
}

/// Analyse `5M`, `512k`, `1G`, `800` (octets/s). Suffixes binaires (×1024).
pub fn parse_rate(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num, mult) = match s.chars().last()? {
        'k' | 'K' => (&s[..s.len() - 1], 1u64 << 10),
        'm' | 'M' => (&s[..s.len() - 1], 1u64 << 20),
        'g' | 'G' => (&s[..s.len() - 1], 1u64 << 30),
        _ => (s, 1),
    };
    let v: f64 = num.trim().parse().ok()?;
    if v <= 0.0 {
        return None;
    }
    Some((v * mult as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rates() {
        assert_eq!(parse_rate("5M"), Some(5 << 20));
        assert_eq!(parse_rate("512k"), Some(512 << 10));
        assert_eq!(parse_rate("1.5G"), Some((1.5 * (1u64 << 30) as f64) as u64));
        assert_eq!(parse_rate("800"), Some(800));
        assert_eq!(parse_rate("0"), None);
        assert_eq!(parse_rate("abc"), None);
    }

    #[tokio::test(start_paused = true)]
    async fn throttles_to_rate() {
        let l = RateLimiter::new(1000);
        let t0 = Instant::now();
        // Capacité initiale 1000 : première prise gratuite, la suivante attend 1 s.
        l.acquire(1000).await;
        l.acquire(1000).await;
        assert!(t0.elapsed() >= Duration::from_millis(999));
    }
}
