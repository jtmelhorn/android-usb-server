// SPDX-License-Identifier: GPL-3.0-or-later
//! USB/IP server core.
//!
//! Platform-independent pieces (protocol codec, session state machine, access control, statistics)
//! build and test on any host. The usbdevfs backend, the socket event loop and the JNI bridge are
//! only compiled for Linux/Android.

pub mod acl;
pub mod backend;
pub mod descriptors;
pub mod engine;
pub mod errno;
pub mod log;
pub mod proto;
pub mod session;
pub mod stats;
pub mod usbfs_sys;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod server;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod usbfs;

#[cfg(target_os = "android")]
mod jni_bridge;
