// SPDX-License-Identifier: GPL-3.0-or-later
//! Parse the raw descriptor blob returned by `UsbDeviceConnection.getRawDescriptors()`
//! (device descriptor followed by the configuration descriptor sets).

use crate::proto::{DeviceRecord, InterfaceClass};

pub const SPEED_UNKNOWN: u32 = 0;
pub const SPEED_LOW: u32 = 1;
pub const SPEED_FULL: u32 = 2;
pub const SPEED_HIGH: u32 = 3;
pub const SPEED_SUPER: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpKind {
    None,
    Interrupt,
    Bulk,
    Isochronous,
}

/// Index into `DeviceInfo::endpoints`: endpoint number plus 16 for IN.
#[inline]
pub fn ep_index(address: u8) -> usize {
    (address & 0x0f) as usize | if address & 0x80 != 0 { 16 } else { 0 }
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub record: DeviceRecord,
    pub endpoints: [EpKind; 32],
    pub max_packet0: u8,
    pub bcd_usb: u16,
}

impl DeviceInfo {
    pub fn endpoint_kind(&self, address: u8) -> EpKind {
        self.endpoints[ep_index(address)]
    }
}

/// Build the exported device description.
///
/// `speed` is a `USBDEVFS_GET_SPEED` value or `SPEED_UNKNOWN`, in which case it is guessed from
/// `bcdUSB` (USB 3.x → super speed, otherwise full speed, which is what gamepads use).
pub fn parse(
    raw: &[u8],
    busid: &str,
    path: &str,
    busnum: u32,
    devnum: u32,
    speed: u32,
) -> Result<DeviceInfo, String> {
    if raw.len() < 18 || raw[0] < 18 || raw[1] != 1 {
        return Err("raw descriptors do not start with a device descriptor".into());
    }
    let le16 = |at: usize| u16::from_le_bytes([raw[at], raw[at + 1]]);
    let bcd_usb = le16(2);
    let speed = if speed != SPEED_UNKNOWN {
        speed
    } else if bcd_usb >= 0x0300 {
        SPEED_SUPER
    } else {
        SPEED_FULL
    };

    let mut endpoints = [EpKind::None; 32];
    let mut interfaces: Vec<(u8, InterfaceClass)> = Vec::new();
    let mut configuration_value = 0u8;

    // Walk the first configuration only (gamepads have exactly one).
    let mut pos = raw[0] as usize;
    let mut in_first_config = false;
    while pos + 2 <= raw.len() {
        let len = raw[pos] as usize;
        let typ = raw[pos + 1];
        if len < 2 || pos + len > raw.len() {
            break; // truncated / malformed tail: keep what was parsed
        }
        let d = &raw[pos..pos + len];
        match typ {
            2 if len >= 9 => {
                if in_first_config {
                    break;
                }
                in_first_config = true;
                configuration_value = d[5];
            }
            4 if len >= 9 && in_first_config => {
                let number = d[2];
                let alt = d[3];
                if alt == 0 && !interfaces.iter().any(|(n, _)| *n == number) {
                    interfaces.push((number, InterfaceClass { class: d[5], subclass: d[6], protocol: d[7] }));
                }
            }
            5 if len >= 7 && in_first_config => {
                let kind = match d[3] & 0x03 {
                    1 => EpKind::Isochronous,
                    2 => EpKind::Bulk,
                    3 => EpKind::Interrupt,
                    _ => EpKind::None,
                };
                endpoints[ep_index(d[2])] = kind;
            }
            _ => {}
        }
        pos += len;
    }
    if !in_first_config {
        return Err("no configuration descriptor found".into());
    }
    interfaces.sort_by_key(|(n, _)| *n);

    Ok(DeviceInfo {
        record: DeviceRecord {
            path: path.to_string(),
            busid: busid.to_string(),
            busnum,
            devnum,
            speed,
            id_vendor: le16(8),
            id_product: le16(10),
            bcd_device: le16(12),
            device_class: raw[4],
            device_subclass: raw[5],
            device_protocol: raw[6],
            configuration_value,
            num_configurations: raw[17],
            interfaces: interfaces.into_iter().map(|(_, c)| c).collect(),
        },
        endpoints,
        max_packet0: raw[7],
        bcd_usb,
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Descriptors of a wired Xbox 360 controller (045e:028e), the class the 8BitDo adapter is
    /// expected to present in XInput mode. Interface 0: interrupt IN 0x81 / OUT 0x01.
    pub fn xbox360_raw() -> Vec<u8> {
        let mut v = vec![
            18, 1, 0x00, 0x02, 0xff, 0xff, 0xff, 8, 0x5e, 0x04, 0x8e, 0x02, 0x14, 0x01, 1, 2, 3, 1,
        ];
        let config_body: Vec<u8> = [
            // interface 0: class ff/5d/01, 2 endpoints
            &[9u8, 4, 0, 0, 2, 0xff, 0x5d, 0x01, 0][..],
            &[17, 0x21, 0x00, 0x01, 0x01, 0x25, 0x81, 0x14, 0, 0, 0, 0, 0x13, 0x01, 0x08, 0, 0],
            &[7, 5, 0x81, 3, 32, 0, 4],
            &[7, 5, 0x01, 3, 32, 0, 8],
            // interface 1: headset (interrupt) — present on real pads
            &[9, 4, 1, 0, 4, 0xff, 0x5d, 0x03, 0],
            &[7, 5, 0x82, 3, 32, 0, 2],
            &[7, 5, 0x02, 3, 32, 0, 4],
            &[7, 5, 0x83, 3, 32, 0, 64],
            &[7, 5, 0x03, 3, 32, 0, 16],
            // interface 2
            &[9, 4, 2, 0, 1, 0xff, 0x5d, 0x02, 0],
            &[7, 5, 0x84, 3, 32, 0, 16],
            // interface 3: security, no endpoints
            &[9, 4, 3, 0, 0, 0xff, 0xfd, 0x13, 4],
        ]
        .concat();
        let total = 9 + config_body.len();
        v.extend_from_slice(&[9, 2, total as u8, (total >> 8) as u8, 4, 1, 0, 0xa0, 0xfa]);
        v.extend_from_slice(&config_body);
        v
    }

    #[test]
    fn parses_xbox360_layout() {
        let info = parse(&xbox360_raw(), "1-1", "/dev/bus/usb/001/005", 1, 5, SPEED_UNKNOWN).unwrap();
        let r = &info.record;
        assert_eq!((r.id_vendor, r.id_product, r.bcd_device), (0x045e, 0x028e, 0x0114));
        assert_eq!(r.speed, SPEED_FULL);
        assert_eq!(r.configuration_value, 1);
        assert_eq!(r.num_configurations, 1);
        assert_eq!(r.interfaces.len(), 4);
        assert_eq!(r.interfaces[0], InterfaceClass { class: 0xff, subclass: 0x5d, protocol: 0x01 });
        assert_eq!(r.interfaces[3], InterfaceClass { class: 0xff, subclass: 0xfd, protocol: 0x13 });
        assert_eq!(info.endpoint_kind(0x81), EpKind::Interrupt);
        assert_eq!(info.endpoint_kind(0x01), EpKind::Interrupt);
        assert_eq!(info.endpoint_kind(0x84), EpKind::Interrupt);
        assert_eq!(info.endpoint_kind(0x85), EpKind::None);
        assert_eq!(r.devid(), 0x0001_0005);
    }

    #[test]
    fn explicit_speed_wins() {
        let info = parse(&xbox360_raw(), "1-1", "p", 1, 5, SPEED_HIGH).unwrap();
        assert_eq!(info.record.speed, SPEED_HIGH);
    }

    #[test]
    fn rejects_garbage_and_survives_truncation() {
        assert!(parse(&[], "1-1", "p", 1, 1, 0).is_err());
        assert!(parse(&[9, 2, 0, 0, 0, 0, 0, 0, 0], "1-1", "p", 1, 1, 0).is_err());
        let raw = xbox360_raw();
        for cut in 18..raw.len() {
            let _ = parse(&raw[..cut], "1-1", "p", 1, 1, 0); // must not panic
        }
        // zero-length descriptor inside config must not loop forever
        let mut bad = raw[..27].to_vec();
        bad.extend_from_slice(&[0, 0, 0]);
        assert!(parse(&bad, "1-1", "p", 1, 1, 0).is_ok());
    }
}
