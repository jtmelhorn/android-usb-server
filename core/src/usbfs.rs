// SPDX-License-Identifier: GPL-3.0-or-later
//! usbdevfs backend: asynchronous URBs on the device fd that Android's `UsbDeviceConnection`
//! opened for us (the same mechanism libusb uses on Android; no root needed).
//!
//! * Transfers are submitted with `USBDEVFS_SUBMITURB` and collected with the non-blocking
//!   `USBDEVFS_REAPURBNDELAY` when `poll()` reports `POLLOUT` on the fd. No thread ever blocks
//!   inside the kernel, so stop and disconnect cannot hang.
//! * URB structs and their buffers live in reusable slots: after warm-up, a transfer does not
//!   allocate.
//! * The fd is borrowed: Kotlin owns the `UsbDeviceConnection` and closes it after `stop()`
//!   returns.

use std::os::raw::{c_int, c_uint, c_void};
use std::os::unix::io::RawFd;
use std::time::{Duration, Instant};

use crate::backend::{Completion, Submit, TransferKind, UsbBackend};
use crate::descriptors::SPEED_UNKNOWN;
use crate::errno;
use crate::usbfs_sys::*;
use crate::{log_error, log_warn};

struct Slot {
    urb: Box<UsbdevfsUrb>,
    buf: Vec<u8>,
    tag: u64,
    in_use: bool,
    is_control: bool,
    is_in: bool,
}

pub struct UsbFs {
    fd: RawFd,
    slots: Vec<Slot>,
    free: Vec<usize>,
    in_flight: usize,
}

// SAFETY: the raw pointers inside the slots only ever point into memory owned by this struct,
// and the struct is used by one thread at a time (moved into the server thread).
unsafe impl Send for UsbFs {}

fn errno_now() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(errno::EIO)
}

impl UsbFs {
    /// Wrap a usbdevfs fd. The caller keeps ownership of the fd and must keep it open until this
    /// value is dropped.
    pub fn new(fd: RawFd) -> Self {
        Self { fd, slots: Vec::with_capacity(16), free: Vec::with_capacity(16), in_flight: 0 }
    }

    pub fn fd(&self) -> RawFd {
        self.fd
    }

    /// Device speed via `USBDEVFS_GET_SPEED` (kernel ≥ 3.3); `SPEED_UNKNOWN` if unsupported.
    pub fn speed(&self) -> u32 {
        // SAFETY: GET_SPEED takes no argument and returns the speed or -1.
        let r = unsafe { libc::ioctl(self.fd, USBDEVFS_GET_SPEED as _, 0) };
        if r < 0 {
            log_warn!("USBDEVFS_GET_SPEED failed (errno {}); guessing speed from descriptors", errno_now());
            SPEED_UNKNOWN
        } else {
            r as u32
        }
    }

    fn alloc_slot(&mut self) -> usize {
        if let Some(i) = self.free.pop() {
            return i;
        }
        self.slots.push(Slot {
            urb: Box::default(),
            buf: Vec::with_capacity(64),
            tag: 0,
            in_use: false,
            is_control: false,
            is_in: false,
        });
        self.slots.len() - 1
    }

    fn release_slot(&mut self, i: usize) {
        self.slots[i].in_use = false;
        self.free.push(i);
    }

    /// Wait (bounded) until the kernel has returned every URB, so no memory it references is freed.
    fn drain(&mut self, limit: Duration) {
        let deadline = Instant::now() + limit;
        while self.in_flight > 0 && Instant::now() < deadline {
            let mut p = libc::pollfd { fd: self.fd, events: libc::POLLOUT, revents: 0 };
            // SAFETY: valid pollfd array of length 1.
            unsafe { libc::poll(&mut p, 1, 20) };
            // On disconnect the kernel kills outstanding URBs and they become reapable, so an
            // error here just means "keep collecting until the deadline".
            let _ = self.reap(&mut |_| {});
        }
    }
}

impl UsbBackend for UsbFs {
    fn submit(&mut self, s: &Submit<'_>) -> Result<(), i32> {
        let is_in = s.is_in();
        let (typ, data_len) = match s.kind {
            TransferKind::Control => (URB_TYPE_CONTROL, 8 + if is_in { s.in_len } else { s.out_data.len() }),
            TransferKind::Interrupt => (URB_TYPE_INTERRUPT, if is_in { s.in_len } else { s.out_data.len() }),
            TransferKind::Bulk => (URB_TYPE_BULK, if is_in { s.in_len } else { s.out_data.len() }),
        };
        let idx = self.alloc_slot();
        let fd = self.fd;
        let slot = &mut self.slots[idx];
        slot.buf.clear();
        slot.buf.resize(data_len, 0);
        match s.kind {
            TransferKind::Control => {
                slot.buf[..8].copy_from_slice(&s.setup);
                if !is_in {
                    slot.buf[8..].copy_from_slice(s.out_data);
                }
            }
            _ if !is_in => slot.buf.copy_from_slice(s.out_data),
            _ => {}
        }
        let mut flags: c_uint = 0;
        if s.short_not_ok {
            flags |= URB_SHORT_NOT_OK;
        }
        if s.zero_packet && s.kind == TransferKind::Bulk {
            flags |= URB_ZERO_PACKET;
        }
        *slot.urb = UsbdevfsUrb {
            typ,
            endpoint: s.endpoint,
            flags,
            buffer: slot.buf.as_mut_ptr() as *mut c_void,
            buffer_length: data_len as c_int,
            usercontext: idx as *mut c_void,
            ..Default::default()
        };
        slot.tag = s.tag;
        slot.is_control = s.kind == TransferKind::Control;
        slot.is_in = is_in;
        // SAFETY: the URB and its buffer stay alive and unmoved (boxed / not resized) until the
        // kernel hands the URB back through REAPURBNDELAY.
        let r = unsafe { libc::ioctl(fd, USBDEVFS_SUBMITURB as _, &mut *slot.urb as *mut UsbdevfsUrb) };
        if r < 0 {
            let e = errno_now();
            self.free.push(idx);
            return Err(-e);
        }
        slot.in_use = true;
        self.in_flight += 1;
        Ok(())
    }

    fn discard(&mut self, tag: u64) -> bool {
        let fd = self.fd;
        match self.slots.iter_mut().find(|s| s.in_use && s.tag == tag) {
            Some(slot) => {
                // SAFETY: the pointer identifies a URB we submitted and have not reaped.
                let r = unsafe { libc::ioctl(fd, USBDEVFS_DISCARDURB as _, &mut *slot.urb as *mut UsbdevfsUrb) };
                r == 0
            }
            None => false,
        }
    }

    fn set_interface(&mut self, interface: u16, alt: u16) -> Result<(), i32> {
        let mut si = UsbdevfsSetInterface { interface: interface as c_uint, altsetting: alt as c_uint };
        // SAFETY: valid pointer to a correctly laid out struct.
        let r = unsafe { libc::ioctl(self.fd, USBDEVFS_SETINTERFACE as _, &mut si as *mut UsbdevfsSetInterface) };
        if r < 0 {
            Err(-errno_now())
        } else {
            Ok(())
        }
    }

    fn clear_halt(&mut self, endpoint: u8) -> Result<(), i32> {
        let mut ep: c_uint = endpoint as c_uint;
        // SAFETY: valid pointer to an unsigned int.
        let r = unsafe { libc::ioctl(self.fd, USBDEVFS_CLEAR_HALT as _, &mut ep as *mut c_uint) };
        if r < 0 {
            Err(-errno_now())
        } else {
            Ok(())
        }
    }

    fn reap(&mut self, f: &mut dyn FnMut(Completion<'_>)) -> Result<(), i32> {
        loop {
            let mut p: *mut UsbdevfsUrb = std::ptr::null_mut();
            // SAFETY: REAPURBNDELAY writes one pointer into `p`.
            let r = unsafe { libc::ioctl(self.fd, USBDEVFS_REAPURBNDELAY as _, &mut p as *mut *mut UsbdevfsUrb) };
            if r < 0 {
                let e = errno_now();
                if e == libc::EINTR {
                    continue;
                }
                if e == libc::EAGAIN {
                    return Ok(());
                }
                return Err(-e);
            }
            if p.is_null() {
                continue;
            }
            // SAFETY: the kernel returns a pointer we passed to SUBMITURB.
            let (idx, status, actual) = unsafe { ((*p).usercontext as usize, (*p).status, (*p).actual_length) };
            if idx >= self.slots.len() || !self.slots[idx].in_use {
                log_error!("reaped unknown URB (context {idx}); ignoring");
                continue;
            }
            self.in_flight -= 1;
            let slot = &self.slots[idx];
            let actual = actual.max(0) as usize;
            let in_data: &[u8] = if !slot.is_in {
                &[]
            } else if slot.is_control {
                let end = (8 + actual).min(slot.buf.len());
                &slot.buf[8.min(end)..end]
            } else {
                &slot.buf[..actual.min(slot.buf.len())]
            };
            f(Completion { tag: slot.tag, status, actual_length: actual, in_data });
            self.release_slot(idx);
        }
    }

    fn in_flight(&self) -> usize {
        self.in_flight
    }
}

impl Drop for UsbFs {
    fn drop(&mut self) {
        if self.in_flight == 0 {
            return;
        }
        let tags: Vec<u64> = self.slots.iter().filter(|s| s.in_use).map(|s| s.tag).collect();
        for t in tags {
            self.discard(t);
        }
        self.drain(Duration::from_secs(1));
        if self.in_flight > 0 {
            // The kernel still references these buffers; leaking is the only safe option.
            log_error!("{} URB(s) not returned by the kernel; leaking their buffers", self.in_flight);
            std::mem::forget(std::mem::take(&mut self.slots));
        }
    }
}
