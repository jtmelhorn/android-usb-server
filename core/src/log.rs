// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimal logging: Android `liblog` on device, stderr elsewhere.

use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(v: bool) {
    VERBOSE.store(v, Ordering::Relaxed);
}

#[inline]
pub fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

#[derive(Clone, Copy)]
pub enum Level {
    Debug = 3,
    Info = 4,
    Warn = 5,
    Error = 6,
}

#[cfg(target_os = "android")]
mod sys {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }

    pub fn write(level: super::Level, msg: &str) {
        let text = CString::new(msg.replace('\0', "?")).unwrap_or_default();
        // SAFETY: both pointers are valid NUL-terminated strings for the duration of the call.
        unsafe {
            __android_log_write(level as c_int, b"UsbIpCore\0".as_ptr() as *const c_char, text.as_ptr());
        }
    }
}

#[cfg(not(target_os = "android"))]
mod sys {
    pub fn write(level: super::Level, msg: &str) {
        let l = match level {
            super::Level::Debug => "D",
            super::Level::Info => "I",
            super::Level::Warn => "W",
            super::Level::Error => "E",
        };
        eprintln!("{l} UsbIpCore: {msg}");
    }
}

pub fn write(level: Level, msg: &str) {
    sys::write(level, msg);
}

#[macro_export]
macro_rules! log_info { ($($t:tt)*) => { $crate::log::write($crate::log::Level::Info, &format!($($t)*)) } }
#[macro_export]
macro_rules! log_warn { ($($t:tt)*) => { $crate::log::write($crate::log::Level::Warn, &format!($($t)*)) } }
#[macro_export]
macro_rules! log_error { ($($t:tt)*) => { $crate::log::write($crate::log::Level::Error, &format!($($t)*)) } }
/// Verbose per-URB tracing. The format arguments are not evaluated unless verbose logging is on.
#[macro_export]
macro_rules! log_trace {
    ($($t:tt)*) => {
        if $crate::log::verbose() { $crate::log::write($crate::log::Level::Debug, &format!($($t)*)) }
    }
}
