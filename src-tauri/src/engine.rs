//! The click engine: hotkey thread, clicker thread, stats thread.
use crate::keys::{self, Key};
use crate::pacer::Pacer;
use crate::sys;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const MAX_CPS: u32 = 5000;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub enabled: bool,
    pub trigger_key: String,
    /// "hold" or "toggle"
    pub mode: String,
    pub target_cps: u32,
    pub output_keys: Vec<String>,
    /// false: one key per click, cycling through the list. true: every key on every click.
    pub multi_input: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enabled: true,
            trigger_key: "F6".into(),
            mode: "hold".into(),
            target_cps: 100,
            output_keys: vec![],
            multi_input: false,
        }
    }
}

#[derive(Default)]
pub struct Shared {
    pub cfg: Mutex<Config>,
    pub version: AtomicU64,
    pub active: AtomicBool,
    pub clicks: AtomicU64,
}

impl Shared {
    pub fn set_config(&self, c: Config) {
        *self.cfg.lock().unwrap() = c;
        self.version.fetch_add(1, Ordering::Release);
    }
}

/// What the app tells the UI.
pub trait Notifier: Send + Sync + 'static {
    fn running(&self, on: bool);
    fn stats(&self, cps: f64, cpu: f64);
}

struct Plan {
    cps: f64,
    multi: bool,
    keys: Vec<sys::Resolved>,
}

fn build_plan(cfg: &Config) -> Plan {
    let trigger = keys::parse(&cfg.trigger_key);
    let mut parsed: Vec<Key> = Vec::new();
    for name in &cfg.output_keys {
        if let Some(k) = keys::parse(name) {
            // never output the trigger key itself: GetAsyncKeyState sees injected input,
            // which would make hold-mode re-trigger itself.
            if Some(k) != trigger && !parsed.contains(&k) {
                parsed.push(k);
            }
        }
    }
    Plan {
        cps: cfg.target_cps.clamp(1, MAX_CPS) as f64,
        multi: cfg.multi_input,
        keys: parsed.into_iter().map(sys::resolve).collect(),
    }
}

pub fn spawn(shared: Arc<Shared>, notify: Arc<dyn Notifier>) {
    {
        let (s, n) = (shared.clone(), notify.clone());
        std::thread::Builder::new().name("hotkey".into()).spawn(move || hotkey_loop(s, n)).unwrap();
    }
    {
        let s = shared.clone();
        std::thread::Builder::new().name("clicker".into()).spawn(move || clicker_loop(s)).unwrap();
    }
    std::thread::Builder::new().name("stats".into()).spawn(move || stats_loop(shared, notify)).unwrap();
}

fn hotkey_loop(shared: Arc<Shared>, notify: Arc<dyn Notifier>) {
    let mut ver = u64::MAX;
    let mut cfg = Config::default();
    let mut trigger: Option<u16> = None;
    let mut prev = false;
    let mut toggled = false;
    let mut last_active = false;
    loop {
        std::thread::sleep(Duration::from_millis(2));
        let v = shared.version.load(Ordering::Acquire);
        if v != ver {
            cfg = shared.cfg.lock().unwrap().clone();
            trigger = keys::parse(&cfg.trigger_key).map(keys::poll_vk);
            toggled = false;
            prev = false;
            ver = v;
        }
        let down = trigger.map(sys::key_down).unwrap_or(false);
        let want = if !cfg.enabled {
            false
        } else if cfg.mode == "toggle" {
            if down && !prev {
                toggled = !toggled;
            }
            toggled
        } else {
            down
        };
        prev = down;
        if want != last_active {
            last_active = want;
            shared.active.store(want, Ordering::Release);
            notify.running(want);
        }
    }
}

fn clicker_loop(shared: Arc<Shared>) {
    sys::boost_thread();
    let timer = sys::Timer::new();
    let freq = sys::qpf();
    let mut sender = sys::Sender::new();
    let mut plan = Plan { cps: 1.0, multi: false, keys: vec![] };
    let mut ver = u64::MAX;
    let mut pacer: Option<Pacer> = None;
    let mut rr = 0usize;
    loop {
        let v = shared.version.load(Ordering::Acquire);
        if v != ver {
            plan = build_plan(&shared.cfg.lock().unwrap());
            ver = v;
            pacer = None;
        }
        if !shared.active.load(Ordering::Acquire) || plan.keys.is_empty() {
            pacer = None;
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }

        let iter_start = sys::qpc();
        let p = pacer.get_or_insert_with(|| Pacer::new(iter_start, plan.cps));
        let n = p.due(iter_start, freq);
        if n > 0 {
            for _ in 0..n {
                if plan.multi {
                    sender.push_group(&plan.keys);
                } else {
                    sender.push_group(std::slice::from_ref(&plan.keys[rr % plan.keys.len()]));
                    rr = rr.wrapping_add(1);
                }
            }
            sender.flush();
            shared.clicks.fetch_add(n as u64, Ordering::Relaxed);
        }

        // Next wake: when the next click is due, but never more often than every 1 ms.
        // Above ~1000 CPS that makes each wake-up send a batch of clicks in one syscall,
        // which is what keeps high CPS from flooding the input queue and lagging the game.
        let ms = (freq / 1000).max(1);
        let wake = p.next_due(freq).max(iter_start + ms);
        let period_us = (1_000_000.0 / plan.cps).max(1000.0);
        let spin_us = (period_us / 8.0).clamp(20.0, 250.0) as i64;
        wait_until(&timer, wake, freq, spin_us, &shared, ver);
    }
}

/// Sleep on the OS timer for the bulk of the wait, spin for the last few microseconds.
/// Wakes early (in <=20 ms slices) if the clicker was stopped or the config changed.
fn wait_until(timer: &sys::Timer, wake: i64, freq: i64, spin_us: i64, shared: &Shared, ver: u64) {
    loop {
        let rem = wake - sys::qpc();
        if rem <= 0 {
            return;
        }
        if !shared.active.load(Ordering::Relaxed) || shared.version.load(Ordering::Relaxed) != ver {
            return;
        }
        let rem_us = rem * 1_000_000 / freq;
        if rem_us > spin_us + 50 {
            timer.wait_us((rem_us - spin_us).min(20_000) as u64);
        } else {
            std::hint::spin_loop();
        }
    }
}

fn stats_loop(shared: Arc<Shared>, notify: Arc<dyn Notifier>) {
    let ncpu = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
    let mut last_clicks = 0u64;
    let mut last_cpu = sys::process_cpu_100ns();
    let mut last_t = std::time::Instant::now();
    let mut idle_sent = false;
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let dt = last_t.elapsed().as_secs_f64().max(0.001);
        last_t = std::time::Instant::now();
        let c = shared.clicks.load(Ordering::Relaxed);
        let cps = (c - last_clicks) as f64 / dt;
        last_clicks = c;
        let cpu_now = sys::process_cpu_100ns();
        let cpu = ((cpu_now - last_cpu) as f64 / 1e7) / dt / ncpu * 100.0;
        last_cpu = cpu_now;
        let busy = cps > 0.0;
        if busy || !idle_sent {
            notify.stats(cps, cpu.clamp(0.0, 100.0));
        }
        idle_sent = !busy;
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering::Relaxed;

    struct Quiet;
    impl Notifier for Quiet {
        fn running(&self, _: bool) {}
        fn stats(&self, _: f64, _: f64) {}
    }

    fn measure(cps: u32, multi: bool, keys: &[&str]) -> (f64, u64) {
        let shared = Arc::new(Shared::default());
        shared.set_config(Config {
            enabled: true,
            trigger_key: "F6".into(),
            mode: "hold".into(),
            target_cps: cps,
            output_keys: keys.iter().map(|s| s.to_string()).collect(),
            multi_input: multi,
        });
        spawn(shared.clone(), Arc::new(Quiet));
        std::thread::sleep(Duration::from_millis(100));
        sys::EVENTS_SENT.store(0, Relaxed);
        let c0 = shared.clicks.load(Relaxed);
        sys::KEYS_DOWN[0x75].store(true, Relaxed); // hold F6
        let t = std::time::Instant::now();
        std::thread::sleep(Duration::from_secs(3));
        let el = t.elapsed().as_secs_f64();
        let clicks = shared.clicks.load(Relaxed) - c0;
        sys::KEYS_DOWN[0x75].store(false, Relaxed); // release
        std::thread::sleep(Duration::from_millis(100));
        let after = shared.clicks.load(Relaxed);
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(after, shared.clicks.load(Relaxed), "clicker must stop when the key is released");
        (clicks as f64 / el, sys::EVENTS_SENT.load(Relaxed))
    }

    // Engine threads are process-global singletons per test, so run scenarios sequentially.
    #[test]
    fn rates_and_modes() {
        let (r, ev) = measure(50, false, &["A", "B"]);
        eprintln!("50cps -> {r:.1} (events {ev})");
        assert!((r - 50.0).abs() < 3.0);
        assert_eq!(ev as f64 / 2.0 / (r * 3.0), 1.0f64.max(ev as f64 / 2.0 / (r * 3.0)).min(1.05));
        let (r, _) = measure(2000, false, &["MouseLeft"]);
        eprintln!("2000cps -> {r:.0}");
        assert!((r - 2000.0).abs() / 2000.0 < 0.04);
        let (r, ev) = measure(5000, true, &["A", "MouseRight"]);
        eprintln!("5000cps multi -> {r:.0} (events {ev})");
        assert!((r - 5000.0).abs() / 5000.0 < 0.04);
        assert!(ev as f64 / (r * 3.0) > 3.9, "multi mode: 4 events per click");
    }

    #[test]
    fn trigger_key_is_filtered_from_outputs() {
        let cfg = Config { trigger_key: "F6".into(), output_keys: vec!["F6".into(), "A".into(), "A".into()], ..Config::default() };
        assert_eq!(build_plan(&cfg).keys.len(), 1);
    }
}
