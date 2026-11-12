//! Windows backend: SendInput, QueryPerformanceCounter, high-resolution waitable timer.
use crate::keys::{is_extended, Key, Mouse};
use std::ptr::null;
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::System::Threading::{
    CreateWaitableTimerExW, GetCurrentProcess, GetCurrentThread, GetProcessTimes,
    SetThreadPriority, SetWaitableTimer, WaitForSingleObject,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE,
    KEYBDINPUT, MOUSEINPUT,
};

const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
const KEYEVENTF_KEYUP: u32 = 0x0002;
const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
const MOUSEEVENTF_MIDDLEDOWN: u32 = 0x0020;
const MOUSEEVENTF_MIDDLEUP: u32 = 0x0040;
const MOUSEEVENTF_XDOWN: u32 = 0x0080;
const MOUSEEVENTF_XUP: u32 = 0x0100;

/// A key with its press + release INPUT records prebuilt, so the hot loop only copies them.
#[derive(Clone, Copy)]
pub struct Resolved {
    down: INPUT,
    up: INPUT,
}

fn kb(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    }
}

fn mouse(flags: u32, data: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: data, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    }
}

pub fn resolve(k: Key) -> Resolved {
    match k {
        Key::Vk(vk) => {
            let scan = unsafe { MapVirtualKeyW(vk as u32, 0) } as u16;
            let ext = if is_extended(vk) { KEYEVENTF_EXTENDEDKEY } else { 0 };
            Resolved { down: kb(vk, scan, ext), up: kb(vk, scan, ext | KEYEVENTF_KEYUP) }
        }
        Key::Mouse(m) => {
            let (d, u, data) = match m {
                Mouse::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 0),
                Mouse::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, 0),
                Mouse::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, 0),
                Mouse::X1 => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, 1),
                Mouse::X2 => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, 2),
            };
            Resolved { down: mouse(d, data), up: mouse(u, data) }
        }
    }
}

const CAP: usize = 512;

/// Collects input records and sends them with as few SendInput calls as possible.
pub struct Sender {
    buf: Vec<INPUT>,
}

impl Sender {
    pub fn new() -> Self {
        Sender { buf: Vec::with_capacity(CAP) }
    }
    /// One "click": press every key in the group, then release them all.
    pub fn push_group(&mut self, keys: &[Resolved]) {
        if self.buf.len() + keys.len() * 2 > CAP {
            self.flush();
        }
        for k in keys {
            self.buf.push(k.down);
        }
        for k in keys {
            self.buf.push(k.up);
        }
    }
    pub fn flush(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        unsafe {
            SendInput(self.buf.len() as u32, self.buf.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        }
        self.buf.clear();
    }
}

pub fn qpc() -> i64 {
    let mut v = 0i64;
    unsafe { QueryPerformanceCounter(&mut v) };
    v
}

pub fn qpf() -> i64 {
    let mut v = 0i64;
    unsafe { QueryPerformanceFrequency(&mut v) };
    v.max(1)
}

pub fn key_down(vk: u16) -> bool {
    (unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000) != 0
}

pub fn boost_thread() {
    // THREAD_PRIORITY_HIGHEST (2). Not TIME_CRITICAL: we must never starve the game itself.
    unsafe { SetThreadPriority(GetCurrentThread(), 2) };
}

/// High-resolution waitable timer (Windows 10 1803+), ~0.5 ms accuracy vs 15 ms for Sleep().
pub struct Timer(HANDLE);

impl Timer {
    pub fn new() -> Self {
        unsafe {
            // CREATE_WAITABLE_TIMER_HIGH_RESOLUTION = 0x2, TIMER_ALL_ACCESS = 0x1F0003
            let mut h = CreateWaitableTimerExW(null(), null(), 0x2, 0x1F0003);
            if h as usize == 0 {
                h = CreateWaitableTimerExW(null(), null(), 0, 0x1F0003);
            }
            Timer(h)
        }
    }
    pub fn wait_us(&self, us: u64) {
        if self.0 as usize == 0 {
            std::thread::sleep(Duration::from_micros(us));
            return;
        }
        let due: i64 = -((us as i64) * 10); // relative, 100 ns units
        unsafe {
            if SetWaitableTimer(self.0, &due, 0, None, null(), 0) != 0 {
                WaitForSingleObject(self.0, 200);
            } else {
                std::thread::sleep(Duration::from_micros(us));
            }
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if self.0 as usize != 0 {
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Total CPU time (kernel+user) used by this process, in 100 ns units.
pub fn process_cpu_100ns() -> u64 {
    unsafe {
        let mut c: FILETIME = std::mem::zeroed();
        let mut e: FILETIME = std::mem::zeroed();
        let mut k: FILETIME = std::mem::zeroed();
        let mut u: FILETIME = std::mem::zeroed();
        if GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) == 0 {
            return 0;
        }
        let f = |t: &FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
        f(&k) + f(&u)
    }
}
