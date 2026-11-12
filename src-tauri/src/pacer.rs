//! Click scheduling. Pure logic (time is passed in), so it can be unit-tested anywhere.
//!
//! Instead of "sleep 1/cps between clicks" (which drifts and falls apart once the OS timer
//! can't wake you every few hundred microseconds), the pacer keeps an absolute schedule:
//! by time `t` exactly `floor(t * cps) + 1` clicks should have been sent. Each wake-up asks
//! how many clicks are *due* and sends them in one batch, so the average rate is exact no
//! matter how late a wake-up is, and high CPS costs one syscall per millisecond, not one
//! per click.

pub struct Pacer {
    start: i64,
    sent: u64,
    cps: f64,
}

impl Pacer {
    pub fn new(now: i64, cps: f64) -> Self {
        Pacer { start: now, sent: 0, cps: cps.max(0.001) }
    }

    /// Number of clicks to send right now.
    pub fn due(&mut self, now: i64, freq: i64) -> u32 {
        let elapsed = (now - self.start).max(0) as f64 / freq as f64;
        let target = (elapsed * self.cps).floor() as u64 + 1;
        let n = target.saturating_sub(self.sent);
        // If we were stalled (>50ms behind) don't machine-gun a huge burst: resync instead.
        let max_catch = ((self.cps * 0.05) as u64).max(4);
        if n > max_catch {
            self.start = now;
            self.sent = 1;
            return 1;
        }
        self.sent += n;
        n as u32
    }

    /// Timestamp (in ticks) when the next click becomes due.
    pub fn next_due(&self, freq: i64) -> i64 {
        self.start + (self.sent as f64 / self.cps * freq as f64) as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const F: i64 = 10_000_000; // 10 MHz like QPC

    fn simulate(cps: f64, secs: f64, wake_us: i64, jitter_us: i64) -> (u64, u32) {
        let mut p = Pacer::new(0, cps);
        let mut now = 0i64;
        let mut total = 0u64;
        let mut max_batch = 0;
        let mut seed = 12345u64;
        while (now as f64) < secs * F as f64 {
            let n = p.due(now, F);
            total += n as u64;
            max_batch = max_batch.max(n);
            let wake = p.next_due(F).max(now + wake_us * 10);
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let j = ((seed >> 33) as i64 % (jitter_us + 1)) * 10;
            now = wake + j;
        }
        (total, max_batch)
    }

    #[test]
    fn exact_rate_low() {
        let (t, b) = simulate(20.0, 10.0, 1000, 800);
        assert!((t as i64 - 200).abs() <= 2, "{t}");
        assert_eq!(b, 1);
    }
    #[test]
    fn exact_rate_high_with_jitter() {
        for cps in [300.0, 1000.0, 2500.0, 5000.0] {
            let (t, _) = simulate(cps, 5.0, 1000, 600);
            let want = cps * 5.0;
            assert!((t as f64 - want).abs() / want < 0.02, "cps {cps}: {t} vs {want}");
        }
    }
    #[test]
    fn batches_when_fast() {
        let (_, b) = simulate(5000.0, 1.0, 1000, 0);
        assert!(b >= 4 && b <= 8, "{b}");
    }
    #[test]
    fn first_click_immediate() {
        let mut p = Pacer::new(100, 10.0);
        assert_eq!(p.due(100, F), 1);
        assert_eq!(p.due(100, F), 0);
    }
    #[test]
    fn stall_resyncs() {
        let mut p = Pacer::new(0, 100.0);
        assert_eq!(p.due(0, F), 1);
        // 2s stall would be 200 clicks owed; we send 1 and resync
        assert_eq!(p.due(2 * F, F), 1);
        assert_eq!(p.due(2 * F + F / 100, F), 1);
    }
}
