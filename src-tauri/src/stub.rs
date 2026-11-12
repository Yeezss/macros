//! Non-Windows stand-in so the project builds and the engine can be tested anywhere.
//! It sends nothing; it only counts the events it would have sent.
use crate::keys::Key;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub static EVENTS_SENT: AtomicU64 = AtomicU64::new(0);
pub static KEYS_DOWN: [AtomicBool; 256] = [const { AtomicBool::new(false) }; 256]; // simulated keyboard, for tests

#[derive(Clone, Copy)]
pub struct Resolved(#[allow(dead_code)] Key);

pub fn resolve(k: Key) -> Resolved {
    Resolved(k)
}

pub struct Sender {
    pending: u64,
}
impl Sender {
    pub fn new() -> Self {
        Sender { pending: 0 }
    }
    pub fn push_group(&mut self, keys: &[Resolved]) {
        self.pending += keys.len() as u64 * 2;
    }
    pub fn flush(&mut self) {
        EVENTS_SENT.fetch_add(self.pending, Ordering::Relaxed);
        self.pending = 0;
    }
}

fn epoch() -> Instant {
    static E: OnceLock<Instant> = OnceLock::new();
    *E.get_or_init(Instant::now)
}
pub fn qpc() -> i64 {
    epoch().elapsed().as_nanos() as i64
}
pub fn qpf() -> i64 {
    1_000_000_000
}
pub fn key_down(vk: u16) -> bool {
    KEYS_DOWN[(vk & 0xFF) as usize].load(Ordering::Relaxed)
}
pub fn boost_thread() {}

pub struct Timer;
impl Timer {
    pub fn new() -> Self {
        Timer
    }
    pub fn wait_us(&self, us: u64) {
        std::thread::sleep(Duration::from_micros(us));
    }
}
pub fn process_cpu_100ns() -> u64 {
    0
}
