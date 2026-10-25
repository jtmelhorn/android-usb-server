// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw definitions from `<linux/usbdevice_fs.h>`. Uses only `std::os::raw` types so the layout and
//! ioctl encoding are unit-tested on every host (pointer width decides 32/64-bit layout, exactly as
//! for the Android ABIs).

use std::mem::size_of;
use std::os::raw::{c_int, c_uint, c_void};

pub const URB_TYPE_INTERRUPT: u8 = 1;
pub const URB_TYPE_CONTROL: u8 = 2;
pub const URB_TYPE_BULK: u8 = 3;

pub const URB_SHORT_NOT_OK: c_uint = 0x01;
pub const URB_ZERO_PACKET: c_uint = 0x40;

/// `struct usbdevfs_urb` (without the trailing isochronous descriptor array).
#[repr(C)]
#[derive(Debug)]
pub struct UsbdevfsUrb {
    pub typ: u8,
    pub endpoint: u8,
    pub status: c_int,
    pub flags: c_uint,
    pub buffer: *mut c_void,
    pub buffer_length: c_int,
    pub actual_length: c_int,
    pub start_frame: c_int,
    pub number_of_packets: c_int, // union with stream_id
    pub error_count: c_int,
    pub signr: c_uint,
    pub usercontext: *mut c_void,
}

impl Default for UsbdevfsUrb {
    fn default() -> Self {
        Self {
            typ: 0,
            endpoint: 0,
            status: 0,
            flags: 0,
            buffer: std::ptr::null_mut(),
            buffer_length: 0,
            actual_length: 0,
            start_frame: 0,
            number_of_packets: 0,
            error_count: 0,
            signr: 0,
            usercontext: std::ptr::null_mut(),
        }
    }
}

/// `struct usbdevfs_setinterface`.
#[repr(C)]
pub struct UsbdevfsSetInterface {
    pub interface: c_uint,
    pub altsetting: c_uint,
}

const IOC_NONE: u32 = 0;
const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

const fn ioc(dir: u32, nr: u32, size: usize) -> u32 {
    (dir << 30) | ((size as u32) << 16) | ((b'U' as u32) << 8) | nr
}

pub const USBDEVFS_SETINTERFACE: u32 = ioc(IOC_READ, 4, size_of::<UsbdevfsSetInterface>());
pub const USBDEVFS_SUBMITURB: u32 = ioc(IOC_READ, 10, size_of::<UsbdevfsUrb>());
pub const USBDEVFS_DISCARDURB: u32 = ioc(IOC_NONE, 11, 0);
pub const USBDEVFS_REAPURBNDELAY: u32 = ioc(IOC_WRITE, 13, size_of::<*mut c_void>());
pub const USBDEVFS_CLEAR_HALT: u32 = ioc(IOC_READ, 21, size_of::<c_uint>());
pub const USBDEVFS_GET_SPEED: u32 = ioc(IOC_NONE, 31, 0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_and_ioctl_numbers_match_kernel() {
        // Values as produced by the kernel headers on each ABI.
        if size_of::<usize>() == 8 {
            assert_eq!(size_of::<UsbdevfsUrb>(), 56);
            assert_eq!(USBDEVFS_SUBMITURB, 0x8038_550a);
            assert_eq!(USBDEVFS_REAPURBNDELAY, 0x4008_550d);
        } else {
            assert_eq!(size_of::<UsbdevfsUrb>(), 44);
            assert_eq!(USBDEVFS_SUBMITURB, 0x802c_550a);
            assert_eq!(USBDEVFS_REAPURBNDELAY, 0x4004_550d);
        }
        assert_eq!(USBDEVFS_SETINTERFACE, 0x8008_5504);
        assert_eq!(USBDEVFS_DISCARDURB, 0x0000_550b);
        assert_eq!(USBDEVFS_CLEAR_HALT, 0x8004_5515);
        assert_eq!(USBDEVFS_GET_SPEED, 0x0000_551f);
    }
}
