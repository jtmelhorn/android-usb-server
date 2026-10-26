// SPDX-License-Identifier: GPL-3.0-or-later
//! JNI entry points for `io.github.jtmelhorn.usbipbridge.NativeBridge`.

use std::net::IpAddr;
use std::panic::{catch_unwind, AssertUnwindSafe};

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_TRUE};
use jni::JNIEnv;

use crate::acl::AccessPolicy;
use crate::descriptors;
use crate::server::{self, ServerConfig, ServerHandle};
use crate::usbfs::UsbFs;
use crate::{log_error, log_info};

/// Stable busid for the single exported device; it does not change across re-enumeration, so
/// Windows-side `usbip attach -b 1-1` keeps working.
pub const BUSID: &str = "1-1";

fn throw(env: &mut JNIEnv, msg: &str) {
    log_error!("{msg}");
    let _ = env.throw_new("java/io/IOException", msg);
}

#[allow(clippy::too_many_arguments)]
fn start(
    env: &mut JNIEnv,
    fd: jint,
    raw: &JByteArray,
    bus_num: jint,
    dev_num: jint,
    path: &JString,
    port: jint,
    allowed_client: &JString,
    verbose: jboolean,
) -> Result<jlong, String> {
    crate::log::set_verbose(verbose == JNI_TRUE);
    let raw = env.convert_byte_array(raw).map_err(|e| format!("descriptors: {e}"))?;
    let path: String = env.get_string(path).map_err(|e| format!("path: {e}"))?.into();
    let allowed: Option<IpAddr> = if allowed_client.is_null() {
        None
    } else {
        let s: String = env.get_string(allowed_client).map_err(|e| format!("allowed client: {e}"))?.into();
        let s = s.trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s.parse().map_err(|_| format!("invalid allowed client address '{s}'"))?)
        }
    };
    if !(1..=65535).contains(&port) {
        return Err(format!("invalid port {port}"));
    }

    let usb = UsbFs::new(fd);
    let speed = usb.speed();
    let dev = descriptors::parse(&raw, BUSID, &path, bus_num as u32, dev_num as u32, speed)?;
    log_info!(
        "exporting {:04x}:{:04x} as busid {BUSID} (speed {}, {} interface(s))",
        dev.record.id_vendor,
        dev.record.id_product,
        dev.record.speed,
        dev.record.interfaces.len()
    );
    let cfg = ServerConfig { port: port as u16, policy: AccessPolicy { allowed_client: allowed } };
    let handle = server::start(usb, fd, dev, cfg).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            format!("port {port} is already in use")
        } else {
            format!("cannot start server: {e}")
        }
    })?;
    Ok(Box::into_raw(Box::new(handle)) as jlong)
}

#[no_mangle]
pub extern "system" fn Java_io_github_jtmelhorn_usbipbridge_NativeBridge_nativeStart<'l>(
    mut env: JNIEnv<'l>,
    _cls: JClass<'l>,
    fd: jint,
    raw: JByteArray<'l>,
    bus_num: jint,
    dev_num: jint,
    path: JString<'l>,
    port: jint,
    allowed_client: JString<'l>,
    verbose: jboolean,
) -> jlong {
    let r = catch_unwind(AssertUnwindSafe(|| {
        start(&mut env, fd, &raw, bus_num, dev_num, &path, port, &allowed_client, verbose)
    }));
    match r {
        Ok(Ok(h)) => h,
        Ok(Err(msg)) => {
            throw(&mut env, &msg);
            0
        }
        Err(_) => {
            throw(&mut env, "internal error (panic) while starting");
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_jtmelhorn_usbipbridge_NativeBridge_nativeStop<'l>(
    _env: JNIEnv<'l>,
    _cls: JClass<'l>,
    handle: jlong,
) {
    if handle == 0 {
        return;
    }
    let _ = catch_unwind(|| {
        // SAFETY: `handle` came from Box::into_raw in nativeStart and Kotlin calls stop once.
        let mut h = unsafe { Box::from_raw(handle as *mut ServerHandle) };
        h.stop();
    });
}

#[no_mangle]
pub extern "system" fn Java_io_github_jtmelhorn_usbipbridge_NativeBridge_nativeStatus<'l>(
    env: JNIEnv<'l>,
    _cls: JClass<'l>,
    handle: jlong,
) -> jstring {
    let json = if handle == 0 {
        "{\"state\":\"stopped\"}".to_string()
    } else {
        catch_unwind(|| {
            // SAFETY: handle is live until nativeStop; we only borrow it.
            let h = unsafe { &*(handle as *const ServerHandle) };
            h.status()
        })
        .unwrap_or_else(|_| "{\"state\":\"error\",\"detail\":\"status panic\"}".to_string())
    };
    env.new_string(json).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

/// `USBDEVFS_RESET` on a device fd: a USB port reset, after which the device re-enumerates much
/// as if it had been unplugged and plugged back in. Returns 0 or a negative errno.
#[no_mangle]
pub extern "system" fn Java_io_github_jtmelhorn_usbipbridge_NativeBridge_nativeResetDevice<'l>(
    _env: JNIEnv<'l>,
    _cls: JClass<'l>,
    fd: jint,
) -> jint {
    // SAFETY: plain ioctl without argument on a caller-owned fd.
    let r = unsafe { libc::ioctl(fd, crate::usbfs_sys::USBDEVFS_RESET as _, 0) };
    if r < 0 {
        -std::io::Error::last_os_error().raw_os_error().unwrap_or(crate::errno::EIO)
    } else {
        0
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_jtmelhorn_usbipbridge_NativeBridge_nativeVersion<'l>(
    env: JNIEnv<'l>,
    _cls: JClass<'l>,
) -> jstring {
    env.new_string(concat!("usbipcore ", env!("CARGO_PKG_VERSION")))
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
