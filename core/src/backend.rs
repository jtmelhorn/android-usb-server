// SPDX-License-Identifier: GPL-3.0-or-later
//! Abstraction over the USB device so the session logic can be tested without hardware.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferKind {
    Control,
    Interrupt,
    Bulk,
}

/// A transfer to start. `tag` identifies the transfer until it completes: the session generation in
/// the high 32 bits and the USB/IP seqnum in the low 32 bits, so transfers of an aborted session
/// can never be confused with a new session's seqnums.
#[derive(Debug)]
pub struct Submit<'a> {
    pub tag: u64,
    /// Endpoint address including the direction bit (0x80 = IN). 0x00/0x80 for control.
    pub endpoint: u8,
    pub kind: TransferKind,
    /// Setup packet for control transfers.
    pub setup: [u8; 8],
    /// Data to send (OUT transfers / control writes).
    pub out_data: &'a [u8],
    /// Bytes requested (IN transfers / control reads).
    pub in_len: usize,
    pub short_not_ok: bool,
    pub zero_packet: bool,
}

impl Submit<'_> {
    pub fn is_in(&self) -> bool {
        self.endpoint & 0x80 != 0
    }
}

/// A finished transfer. `in_data` is valid only for the duration of the callback.
#[derive(Debug)]
pub struct Completion<'a> {
    pub tag: u64,
    /// 0 or a negative Linux errno.
    pub status: i32,
    pub actual_length: usize,
    pub in_data: &'a [u8],
}

pub trait UsbBackend {
    /// Start a transfer. `Err` carries a negative Linux errno; the transfer was not started.
    fn submit(&mut self, s: &Submit<'_>) -> Result<(), i32>;
    /// Request cancellation. Returns true if the transfer was still pending in the kernel; it will
    /// then be reported through `reap` (usually with `-ECONNRESET`/`-ENOENT`).
    fn discard(&mut self, tag: u64) -> bool;
    fn set_interface(&mut self, interface: u16, alt: u16) -> Result<(), i32>;
    fn clear_halt(&mut self, endpoint: u8) -> Result<(), i32>;
    /// Deliver every completion that is ready, without blocking. `Err(-ENODEV)` means the device
    /// is gone (completions already delivered are still valid).
    fn reap(&mut self, f: &mut dyn FnMut(Completion<'_>)) -> Result<(), i32>;
    /// Number of transfers handed to the kernel and not yet reaped.
    fn in_flight(&self) -> usize;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Submitted {
        pub tag: u64,
        pub endpoint: u8,
        pub kind: TransferKind,
        pub setup: [u8; 8],
        pub out_data: Vec<u8>,
        pub in_len: usize,
    }

    /// Records calls; completions are scripted by the test.
    #[derive(Default)]
    pub struct MockBackend {
        pub submitted: Vec<Submitted>,
        pub pending: Vec<u64>,
        pub ready: VecDeque<(u64, i32, Vec<u8>, usize)>,
        pub discarded: Vec<u64>,
        pub set_interface_calls: Vec<(u16, u16)>,
        pub clear_halt_calls: Vec<u8>,
        pub fail_submit: Option<i32>,
        pub gone: bool,
    }

    impl MockBackend {
        /// Complete a pending IN/OUT transfer with data (IN) or a written length (OUT).
        pub fn complete(&mut self, tag: u64, status: i32, data: &[u8], actual: usize) {
            assert!(self.pending.contains(&tag), "tag {tag} not pending");
            self.ready.push_back((tag, status, data.to_vec(), actual));
        }
    }

    impl UsbBackend for MockBackend {
        fn submit(&mut self, s: &Submit<'_>) -> Result<(), i32> {
            if self.gone {
                return Err(-crate::errno::ENODEV);
            }
            if let Some(e) = self.fail_submit {
                return Err(e);
            }
            self.submitted.push(Submitted {
                tag: s.tag,
                endpoint: s.endpoint,
                kind: s.kind,
                setup: s.setup,
                out_data: s.out_data.to_vec(),
                in_len: s.in_len,
            });
            self.pending.push(s.tag);
            Ok(())
        }

        fn discard(&mut self, tag: u64) -> bool {
            self.discarded.push(tag);
            let still_pending = self.pending.contains(&tag) && !self.ready.iter().any(|r| r.0 == tag);
            if still_pending {
                self.ready.push_back((tag, -crate::errno::ECONNRESET, vec![], 0));
            }
            still_pending
        }

        fn set_interface(&mut self, interface: u16, alt: u16) -> Result<(), i32> {
            self.set_interface_calls.push((interface, alt));
            Ok(())
        }

        fn clear_halt(&mut self, endpoint: u8) -> Result<(), i32> {
            self.clear_halt_calls.push(endpoint);
            Ok(())
        }

        fn reap(&mut self, f: &mut dyn FnMut(Completion<'_>)) -> Result<(), i32> {
            while let Some((tag, status, data, actual)) = self.ready.pop_front() {
                self.pending.retain(|&t| t != tag);
                f(Completion { tag, status, actual_length: actual, in_data: &data });
            }
            if self.gone {
                Err(-crate::errno::ENODEV)
            } else {
                Ok(())
            }
        }

        fn in_flight(&self) -> usize {
            self.pending.len()
        }
    }
}
