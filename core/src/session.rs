// SPDX-License-Identifier: GPL-3.0-or-later
//! One client connection: handshake (OP_REQ_DEVLIST / OP_REQ_IMPORT), then the URB phase.
//!
//! The session is a pure state machine: bytes from the socket go in through [`Session::feed`],
//! transfer completions through [`Session::on_completion`], and replies are appended to an output
//! buffer the caller writes to the socket. It never blocks and never touches the socket itself.

use std::collections::HashMap;
use std::time::Instant;

use crate::backend::{Completion, Submit, TransferKind, UsbBackend};
use crate::descriptors::{DeviceInfo, EpKind};
use crate::errno::*;
use crate::proto::*;
use crate::stats::Stats;
use crate::{log_trace, log_warn};

/// Upper bound on concurrently pending transfers per session (protects memory; a gamepad driver
/// keeps a handful outstanding).
pub const MAX_PENDING: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Handshake,
    AwaitingImport,
    Imported,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    /// A valid OP_REQ_IMPORT for our busid arrived; the server must call `accept_import` or
    /// `reject_import`, then feed again (with an empty slice) to process any buffered bytes.
    ImportRequested,
    /// Write out what is buffered, then close the connection.
    CloseAfterFlush,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReason {
    Protocol(ProtoError),
    DuplicateSeqnum(u32),
    TooManyPending,
    DeviceGone,
}

impl std::fmt::Display for CloseReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CloseReason::Protocol(e) => write!(f, "protocol error: {e}"),
            CloseReason::DuplicateSeqnum(s) => write!(f, "duplicate seqnum {s}"),
            CloseReason::TooManyPending => write!(f, "too many pending transfers"),
            CloseReason::DeviceGone => write!(f, "USB device disconnected"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    /// Seqnum of the CMD_UNLINK that targets this transfer, once one has arrived.
    unlink_seqnum: Option<u32>,
    is_in: bool,
    interrupt_in: bool,
}

pub struct Session {
    phase: Phase,
    generation: u32,
    rx: Vec<u8>,
    pending: HashMap<u32, Pending>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self::with_generation(0)
    }

    /// `generation` must differ between sessions that can have transfers in flight at the same
    /// time; it forms the high half of every backend tag.
    pub fn with_generation(generation: u32) -> Self {
        Self { phase: Phase::Handshake, generation, rx: Vec::with_capacity(4096), pending: HashMap::with_capacity(64) }
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    #[inline]
    fn tag(&self, seqnum: u32) -> u64 {
        ((self.generation as u64) << 32) | seqnum as u64
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Process bytes received from the client. On `Err`, the caller must call [`Session::abort`].
    pub fn feed(
        &mut self,
        data: &[u8],
        dev: &DeviceInfo,
        be: &mut dyn UsbBackend,
        out: &mut Vec<u8>,
        stats: &mut Stats,
    ) -> Result<Flow, CloseReason> {
        let mut rx = std::mem::take(&mut self.rx);
        rx.extend_from_slice(data);
        let mut off = 0;
        let result = loop {
            match self.phase {
                Phase::Handshake => match parse_op_request(&rx[off..]) {
                    Ok(None) => break Ok(Flow::Continue),
                    Err(e) => break Err(CloseReason::Protocol(e)),
                    Ok(Some((req, n))) => {
                        off += n;
                        match req {
                            OpRequest::DevList => {
                                encode_devlist_reply(out, &[&dev.record]);
                                self.phase = Phase::Done;
                                break Ok(Flow::CloseAfterFlush);
                            }
                            OpRequest::Import { busid } if busid == dev.record.busid => {
                                self.phase = Phase::AwaitingImport;
                                break Ok(Flow::ImportRequested);
                            }
                            OpRequest::Import { busid } => {
                                log_warn!("import of unknown busid '{busid}' refused");
                                encode_import_reply(out, ST_NODEV, None);
                                self.phase = Phase::Done;
                                break Ok(Flow::CloseAfterFlush);
                            }
                        }
                    }
                },
                Phase::AwaitingImport => break Ok(Flow::ImportRequested),
                Phase::Imported => match parse_urb_command(&rx[off..]) {
                    Ok(None) => break Ok(Flow::Continue),
                    Err(e) => break Err(CloseReason::Protocol(e)),
                    Ok(Some((cmd, n))) => {
                        let r = match cmd {
                            UrbCommand::Submit(c, payload) => self.submit(&c, payload, dev, be, out, stats),
                            UrbCommand::Unlink(u) => {
                                self.unlink(&u, be, out, stats);
                                Ok(())
                            }
                        };
                        off += n;
                        if let Err(e) = r {
                            break Err(e);
                        }
                    }
                },
                Phase::Done => break Ok(Flow::CloseAfterFlush),
            }
        };
        rx.drain(..off);
        self.rx = rx;
        result
    }

    pub fn accept_import(&mut self, dev: &DeviceInfo, out: &mut Vec<u8>) {
        debug_assert_eq!(self.phase, Phase::AwaitingImport);
        encode_import_reply(out, ST_OK, Some(&dev.record));
        self.phase = Phase::Imported;
    }

    pub fn reject_import(&mut self, status: u32, out: &mut Vec<u8>) {
        encode_import_reply(out, status, None);
        self.phase = Phase::Done;
    }

    fn submit(
        &mut self,
        cmd: &CmdSubmit,
        payload: &[u8],
        dev: &DeviceInfo,
        be: &mut dyn UsbBackend,
        out: &mut Vec<u8>,
        stats: &mut Stats,
    ) -> Result<(), CloseReason> {
        stats.submits += 1;
        stats.bytes_in += payload.len() as u64;
        if self.pending.contains_key(&cmd.seqnum) {
            return Err(CloseReason::DuplicateSeqnum(cmd.seqnum));
        }
        if self.pending.len() >= MAX_PENDING {
            return Err(CloseReason::TooManyPending);
        }
        if cmd.is_iso() {
            log_warn!("isochronous transfer on ep {} rejected (unsupported)", cmd.ep);
            reply_now(out, stats, cmd.seqnum, -EXDEV);
            return Ok(());
        }

        let (endpoint, kind, mut setup, in_len, out_data) = if cmd.ep == 0 {
            let s = cmd.setup;
            let is_in = s[0] & 0x80 != 0;
            if let Some(status) = self.intercept_control(&s, dev, be) {
                log_trace!("seq {} ep0 {:02x?} intercepted -> {}", cmd.seqnum, s, status);
                reply_now(out, stats, cmd.seqnum, status);
                return Ok(());
            }
            let mut setup = s;
            let w_length = u16::from_le_bytes([s[6], s[7]]) as usize;
            let (in_len, data) = if is_in {
                (w_length.min(cmd.transfer_buffer_length as usize), &[][..])
            } else {
                (0, &payload[..w_length.min(payload.len())])
            };
            let fixed = if is_in { in_len } else { data.len() } as u16;
            setup[6..8].copy_from_slice(&fixed.to_le_bytes());
            (if is_in { 0x80 } else { 0x00 }, TransferKind::Control, setup, in_len, data)
        } else {
            let is_in = cmd.is_in();
            let address = cmd.ep as u8 | if is_in { 0x80 } else { 0 };
            let kind = match dev.endpoint_kind(address) {
                EpKind::Interrupt => TransferKind::Interrupt,
                EpKind::Bulk => TransferKind::Bulk,
                other => {
                    log_warn!("transfer to endpoint {address:#04x} ({other:?}) rejected");
                    reply_now(out, stats, cmd.seqnum, -EINVAL);
                    return Ok(());
                }
            };
            let in_len = if is_in { cmd.transfer_buffer_length as usize } else { 0 };
            (address, kind, [0u8; 8], in_len, payload)
        };
        if kind != TransferKind::Control {
            setup = [0; 8];
        }

        let is_in = endpoint & 0x80 != 0;
        let s = Submit {
            tag: self.tag(cmd.seqnum),
            endpoint,
            kind,
            setup,
            out_data,
            in_len,
            short_not_ok: is_in && cmd.transfer_flags & URB_SHORT_NOT_OK != 0,
            zero_packet: !is_in && cmd.transfer_flags & URB_ZERO_PACKET != 0,
        };
        match be.submit(&s) {
            Ok(()) => {
                log_trace!(
                    "seq {} submit ep {:#04x} {:?} len {} setup {:02x?} out {:02x?}",
                    cmd.seqnum,
                    endpoint,
                    kind,
                    in_len.max(out_data.len()),
                    if kind == TransferKind::Control { &setup[..] } else { &[][..] },
                    &out_data[..out_data.len().min(16)]
                );
                self.pending.insert(
                    cmd.seqnum,
                    Pending { unlink_seqnum: None, is_in, interrupt_in: is_in && kind == TransferKind::Interrupt },
                );
                Ok(())
            }
            Err(e) => {
                reply_now(out, stats, cmd.seqnum, e);
                if e == -ENODEV || e == -ESHUTDOWN {
                    Err(CloseReason::DeviceGone)
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Standard requests that must not be passed to the device as raw control transfers
    /// (mirrors Linux `stub_rx.c: tweak_special_requests`). Returns the reply status if handled.
    fn intercept_control(&self, s: &[u8; 8], dev: &DeviceInfo, be: &mut dyn UsbBackend) -> Option<i32> {
        let w_value = u16::from_le_bytes([s[2], s[3]]);
        let w_index = u16::from_le_bytes([s[4], s[5]]);
        match (s[0], s[1]) {
            // SET_CONFIGURATION: the configuration is fixed while Android holds our claims.
            (0x00, 0x09) => {
                if w_value != dev.record.configuration_value as u16 && w_value != 0 {
                    log_warn!("SET_CONFIGURATION {w_value} ignored (active {})", dev.record.configuration_value);
                }
                Some(0)
            }
            // SET_INTERFACE
            (0x01, 0x0b) => Some(be.set_interface(w_index, w_value).err().unwrap_or(0)),
            // CLEAR_FEATURE(ENDPOINT_HALT)
            (0x02, 0x01) if w_value == 0 => Some(be.clear_halt(w_index as u8).err().unwrap_or(0)),
            // Hub SET_FEATURE(PORT_RESET): acknowledged but not performed; a real reset could make
            // Android re-bind its own gamepad driver to the adapter.
            (0x23, 0x03) if w_value == 4 => {
                log_warn!("port reset requested by client: acknowledged without resetting");
                Some(0)
            }
            _ => None,
        }
    }

    fn unlink(&mut self, u: &CmdUnlink, be: &mut dyn UsbBackend, out: &mut Vec<u8>, stats: &mut Stats) {
        stats.unlinks += 1;
        let tag = self.tag(u.unlink_seqnum);
        match self.pending.get_mut(&u.unlink_seqnum) {
            Some(p) if p.unlink_seqnum.is_none() => {
                p.unlink_seqnum = Some(u.seqnum);
                // Whether or not the kernel still had it, the transfer will be reaped and the
                // RET_UNLINK is sent from `on_completion` (Linux stub semantics: no RET_SUBMIT).
                let was_pending = be.discard(tag);
                log_trace!("seq {} unlink {} (in kernel: {was_pending})", u.seqnum, u.unlink_seqnum);
            }
            _ => {
                // Already completed (its RET_SUBMIT was sent) or unknown.
                encode_ret_unlink(out, u.seqnum, 0);
            }
        }
    }

    /// Handle a completion from the backend. Returns false if the tag is not ours.
    pub fn on_completion(&mut self, c: &Completion<'_>, out: &mut Vec<u8>, stats: &mut Stats, now: Instant) -> bool {
        if (c.tag >> 32) as u32 != self.generation {
            return false;
        }
        let seqnum = c.tag as u32;
        let Some(p) = self.pending.remove(&seqnum) else {
            return false;
        };
        stats.completions += 1;
        if self.phase != Phase::Imported {
            return true; // session aborted; drop silently
        }
        match p.unlink_seqnum {
            Some(useq) => {
                let status = if c.status == -ENOENT || c.status == -ECONNRESET { -ECONNRESET } else { c.status };
                encode_ret_unlink(out, useq, status);
            }
            None => {
                if c.status != 0 {
                    stats.errors += 1;
                    stats.last_error_status = c.status;
                }
                if p.is_in {
                    let data = &c.in_data[..c.actual_length.min(c.in_data.len())];
                    if p.interrupt_in && c.status == 0 && !data.is_empty() {
                        stats.on_interrupt_in_report(now);
                    }
                    stats.bytes_out += data.len() as u64;
                    encode_ret_submit(out, seqnum, c.status, data.len() as u32, data);
                } else {
                    encode_ret_submit(out, seqnum, c.status, c.actual_length as u32, &[]);
                }
            }
        }
        log_trace!(
            "seq {} complete status {} len {} in {:02x?}",
            seqnum,
            c.status,
            c.actual_length,
            &c.in_data[..c.in_data.len().min(c.actual_length).min(24)]
        );
        true
    }

    /// End the session: cancel everything still pending. Completions that arrive afterwards are
    /// consumed by `on_completion` without producing output. Returns the number of discards issued.
    pub fn abort(&mut self, be: &mut dyn UsbBackend) -> usize {
        self.phase = Phase::Done;
        let tags: Vec<u64> = self.pending.keys().map(|&s| self.tag(s)).collect();
        for &t in &tags {
            be.discard(t);
        }
        tags.len()
    }
}

fn reply_now(out: &mut Vec<u8>, stats: &mut Stats, seqnum: u32, status: i32) {
    if status != 0 {
        stats.errors += 1;
        stats.last_error_status = status;
    }
    encode_ret_submit(out, seqnum, status, 0, &[]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::mock::MockBackend;
    use crate::descriptors;

    struct Harness {
        s: Session,
        be: MockBackend,
        dev: DeviceInfo,
        out: Vec<u8>,
        stats: Stats,
        /// seqnums whose RET_SUBMIT carries IN data
        ins: Vec<u32>,
    }

    impl Harness {
        fn new() -> Self {
            let dev = descriptors::parse(&descriptors::tests::xbox360_raw(), "1-1", "/dev/bus/usb/001/005", 1, 5, 0).unwrap();
            Self { s: Session::new(), be: MockBackend::default(), dev, out: Vec::new(), stats: Stats::default(), ins: vec![] }
        }

        fn feed(&mut self, bytes: &[u8]) -> Result<Flow, CloseReason> {
            self.s.feed(bytes, &self.dev, &mut self.be, &mut self.out, &mut self.stats)
        }

        fn imported() -> Self {
            let mut h = Self::new();
            let mut b = Vec::new();
            encode_op_request(&mut b, &OpRequest::Import { busid: "1-1".into() });
            assert_eq!(h.feed(&b).unwrap(), Flow::ImportRequested);
            h.s.accept_import(&h.dev, &mut h.out);
            assert_eq!(h.s.phase(), Phase::Imported);
            assert_eq!(h.out.len(), 8 + DEVICE_RECORD_LEN);
            h.out.clear();
            h
        }

        fn cmd(&mut self, seq: u32, dir: u32, ep: u32, len: u32, setup: [u8; 8], data: &[u8]) -> Result<Flow, CloseReason> {
            if dir == DIR_IN {
                self.ins.push(seq);
            }
            let c = CmdSubmit {
                seqnum: seq,
                devid: 0x10005,
                direction: dir,
                ep,
                transfer_flags: 0,
                transfer_buffer_length: len,
                start_frame: 0,
                number_of_packets: NON_ISO_PACKETS,
                interval: 4,
                setup,
            };
            let mut b = Vec::new();
            encode_cmd_submit(&mut b, &c, data);
            self.feed(&b)
        }

        fn unlink(&mut self, seq: u32, target: u32) {
            let mut b = Vec::new();
            encode_cmd_unlink(&mut b, &CmdUnlink { seqnum: seq, devid: 0x10005, direction: 0, ep: 0, unlink_seqnum: target });
            assert_eq!(self.feed(&b).unwrap(), Flow::Continue);
        }

        fn reap(&mut self) {
            let (s, out, stats) = (&mut self.s, &mut self.out, &mut self.stats);
            let now = Instant::now();
            let _ = self.be.reap(&mut |c| {
                assert!(s.on_completion(&c, out, stats, now));
            });
        }

        fn replies(&mut self) -> Vec<UrbReply> {
            let mut v = Vec::new();
            let mut off = 0;
            let ins = self.ins.clone();
            while let Some((r, n)) = parse_urb_reply(&self.out[off..], |s| ins.contains(&s)).unwrap() {
                v.push(r);
                off += n;
            }
            assert_eq!(off, self.out.len(), "trailing bytes in output");
            self.out.clear();
            v
        }
    }

    const GET_DEVICE_DESC: [u8; 8] = [0x80, 6, 0, 1, 0, 0, 18, 0];

    #[test]
    fn devlist_then_close() {
        let mut h = Harness::new();
        let mut b = Vec::new();
        encode_op_request(&mut b, &OpRequest::DevList);
        assert_eq!(h.feed(&b).unwrap(), Flow::CloseAfterFlush);
        assert_eq!(h.out.len(), 12 + DEVICE_RECORD_LEN + 4 * 4);
        assert_eq!(h.s.phase(), Phase::Done);
        assert!(h.be.submitted.is_empty());
    }

    #[test]
    fn handshake_split_across_reads() {
        let mut h = Harness::new();
        let mut b = Vec::new();
        encode_op_request(&mut b, &OpRequest::Import { busid: "1-1".into() });
        for byte in &b[..b.len() - 1] {
            assert_eq!(h.feed(&[*byte]).unwrap(), Flow::Continue);
        }
        assert_eq!(h.feed(&b[b.len() - 1..]).unwrap(), Flow::ImportRequested);
    }

    #[test]
    fn import_wrong_busid() {
        let mut h = Harness::new();
        let mut b = Vec::new();
        encode_op_request(&mut b, &OpRequest::Import { busid: "2-1".into() });
        assert_eq!(h.feed(&b).unwrap(), Flow::CloseAfterFlush);
        assert_eq!(h.out, vec![0x01, 0x11, 0x00, 0x03, 0, 0, 0, ST_NODEV as u8]);
    }

    #[test]
    fn import_rejected_busy() {
        let mut h = Harness::new();
        let mut b = Vec::new();
        encode_op_request(&mut b, &OpRequest::Import { busid: "1-1".into() });
        assert_eq!(h.feed(&b).unwrap(), Flow::ImportRequested);
        h.s.reject_import(ST_DEV_BUSY, &mut h.out);
        assert_eq!(h.out, vec![0x01, 0x11, 0x00, 0x03, 0, 0, 0, ST_DEV_BUSY as u8]);
        assert_eq!(h.feed(&[]).unwrap(), Flow::CloseAfterFlush);
    }

    #[test]
    fn bad_version_closes() {
        let mut h = Harness::new();
        assert_eq!(h.feed(&[0x01, 0x06, 0x80, 0x05, 0, 0, 0, 0]), Err(CloseReason::Protocol(ProtoError::BadVersion(0x0106))));
    }

    #[test]
    fn control_in_roundtrip() {
        let mut h = Harness::imported();
        h.cmd(2, DIR_IN, 0, 18, GET_DEVICE_DESC, &[]).unwrap();
        let sub = &h.be.submitted[0];
        assert_eq!((sub.endpoint, sub.kind, sub.in_len, sub.setup), (0x80, TransferKind::Control, 18, GET_DEVICE_DESC));
        let desc = descriptors::tests::xbox360_raw()[..18].to_vec();
        h.be.complete(2, 0, &desc, 18);
        h.reap();
        assert_eq!(
            h.replies(),
            vec![UrbReply::Submit { seqnum: 2, status: 0, actual_length: 18, number_of_packets: -1, data: desc }]
        );
        assert_eq!(h.s.pending_count(), 0);
    }

    #[test]
    fn control_in_wlength_clamped_to_buffer() {
        let mut h = Harness::imported();
        h.cmd(2, DIR_IN, 0, 8, [0x80, 6, 0, 1, 0, 0, 64, 0], &[]).unwrap();
        let sub = &h.be.submitted[0];
        assert_eq!(sub.in_len, 8);
        assert_eq!(&sub.setup[6..8], &[8, 0]);
    }

    #[test]
    fn control_out_vendor_request_passes_payload() {
        let mut h = Harness::imported();
        h.cmd(4, DIR_OUT, 0, 2, [0x41, 0x01, 0, 0, 0, 0, 2, 0], &[0xaa, 0xbb]).unwrap();
        assert_eq!(h.be.submitted[0].out_data, vec![0xaa, 0xbb]);
        assert_eq!(h.be.submitted[0].endpoint, 0x00);
        h.be.complete(4, 0, &[], 2);
        h.reap();
        assert_eq!(
            h.replies(),
            vec![UrbReply::Submit { seqnum: 4, status: 0, actual_length: 2, number_of_packets: -1, data: vec![] }]
        );
    }

    #[test]
    fn interrupt_in_and_out() {
        let mut h = Harness::imported();
        h.cmd(6, DIR_IN, 1, 32, [0; 8], &[]).unwrap();
        h.cmd(8, DIR_OUT, 1, 8, [0; 8], &[0, 8, 0, 0x40, 0x40, 0, 0, 0]).unwrap();
        assert_eq!(h.be.submitted[0].endpoint, 0x81);
        assert_eq!(h.be.submitted[0].kind, TransferKind::Interrupt);
        assert_eq!(h.be.submitted[1].endpoint, 0x01);
        let report = [0u8, 20, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        h.be.complete(6, 0, &report, 20);
        h.be.complete(8, 0, &[], 8);
        h.reap();
        let r = h.replies();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0], UrbReply::Submit { seqnum: 6, status: 0, actual_length: 20, number_of_packets: -1, data: report.to_vec() });
        assert_eq!(r[1], UrbReply::Submit { seqnum: 8, status: 0, actual_length: 8, number_of_packets: -1, data: vec![] });
        assert_eq!(h.stats.interrupt_in_reports, 1);
    }

    #[test]
    fn pipelined_commands_in_one_read() {
        let mut h = Harness::imported();
        let mut b = Vec::new();
        for seq in [10u32, 12, 14] {
            let c = CmdSubmit {
                seqnum: seq,
                devid: 0,
                direction: DIR_IN,
                ep: 1,
                transfer_flags: 0,
                transfer_buffer_length: 32,
                start_frame: 0,
                number_of_packets: 0,
                interval: 4,
                setup: [0; 8],
            };
            encode_cmd_submit(&mut b, &c, &[]);
        }
        // deliver with an awkward split
        assert_eq!(h.feed(&b[..70]).unwrap(), Flow::Continue);
        assert_eq!(h.be.submitted.len(), 1);
        assert_eq!(h.feed(&b[70..]).unwrap(), Flow::Continue);
        assert_eq!(h.be.submitted.iter().map(|s| s.tag).collect::<Vec<_>>(), vec![10, 12, 14]);
    }

    #[test]
    fn generation_isolates_tags() {
        let mut s = Session::with_generation(7);
        let dev = descriptors::parse(&descriptors::tests::xbox360_raw(), "1-1", "p", 1, 5, 0).unwrap();
        let mut be = MockBackend::default();
        let (mut out, mut stats) = (Vec::new(), Stats::default());
        let mut b = Vec::new();
        encode_op_request(&mut b, &OpRequest::Import { busid: "1-1".into() });
        s.feed(&b, &dev, &mut be, &mut out, &mut stats).unwrap();
        s.accept_import(&dev, &mut out);
        b.clear();
        let c = CmdSubmit {
            seqnum: 6,
            devid: 0,
            direction: DIR_IN,
            ep: 1,
            transfer_flags: 0,
            transfer_buffer_length: 32,
            start_frame: 0,
            number_of_packets: -1,
            interval: 4,
            setup: [0; 8],
        };
        encode_cmd_submit(&mut b, &c, &[]);
        s.feed(&b, &dev, &mut be, &mut out, &mut stats).unwrap();
        assert_eq!(be.submitted[0].tag, (7u64 << 32) | 6);
        // A completion carrying another generation's tag with the same seqnum is not ours.
        let foreign = Completion { tag: 6, status: 0, actual_length: 0, in_data: &[] };
        assert!(!s.on_completion(&foreign, &mut out, &mut stats, Instant::now()));
        assert_eq!(s.pending_count(), 1);
    }

    #[test]
    fn set_configuration_intercepted() {
        let mut h = Harness::imported();
        h.cmd(2, DIR_OUT, 0, 0, [0x00, 0x09, 1, 0, 0, 0, 0, 0], &[]).unwrap();
        h.cmd(4, DIR_OUT, 0, 0, [0x00, 0x09, 0, 0, 0, 0, 0, 0], &[]).unwrap();
        assert!(h.be.submitted.is_empty());
        let r = h.replies();
        assert!(matches!(r[0], UrbReply::Submit { seqnum: 2, status: 0, .. }));
        assert!(matches!(r[1], UrbReply::Submit { seqnum: 4, status: 0, .. }));
    }

    #[test]
    fn set_interface_and_clear_halt_and_port_reset_intercepted() {
        let mut h = Harness::imported();
        h.cmd(2, DIR_OUT, 0, 0, [0x01, 0x0b, 0, 0, 1, 0, 0, 0], &[]).unwrap();
        h.cmd(4, DIR_OUT, 0, 0, [0x02, 0x01, 0, 0, 0x81, 0, 0, 0], &[]).unwrap();
        h.cmd(6, DIR_OUT, 0, 0, [0x23, 0x03, 4, 0, 1, 0, 0, 0], &[]).unwrap();
        assert!(h.be.submitted.is_empty());
        assert_eq!(h.be.set_interface_calls, vec![(1, 0)]);
        assert_eq!(h.be.clear_halt_calls, vec![0x81]);
        assert_eq!(h.replies().len(), 3);
    }

    #[test]
    fn unknown_and_iso_endpoints_rejected_without_closing() {
        let mut h = Harness::imported();
        h.cmd(2, DIR_IN, 9, 8, [0; 8], &[]).unwrap();
        let c = CmdSubmit {
            seqnum: 4,
            devid: 0,
            direction: DIR_IN,
            ep: 1,
            transfer_flags: 0,
            transfer_buffer_length: 0,
            start_frame: 0,
            number_of_packets: 1,
            interval: 1,
            setup: [0; 8],
        };
        let mut b = Vec::new();
        encode_cmd_submit(&mut b, &c, &[]);
        b.extend_from_slice(&[0u8; 16]);
        assert_eq!(h.feed(&b).unwrap(), Flow::Continue);
        assert!(h.be.submitted.is_empty());
        let r = h.replies();
        assert!(matches!(r[0], UrbReply::Submit { seqnum: 2, status, .. } if status == -EINVAL));
        assert!(matches!(r[1], UrbReply::Submit { seqnum: 4, status, .. } if status == -EXDEV));
        assert_eq!(h.s.phase(), Phase::Imported);
    }

    #[test]
    fn unlink_pending_sends_only_ret_unlink() {
        let mut h = Harness::imported();
        h.cmd(6, DIR_IN, 1, 32, [0; 8], &[]).unwrap();
        h.unlink(7, 6);
        assert_eq!(h.be.discarded, vec![6]);
        assert!(h.out.is_empty(), "RET_UNLINK must wait for the kernel to give the URB back");
        h.reap();
        assert_eq!(h.replies(), vec![UrbReply::Unlink { seqnum: 7, status: -ECONNRESET }]);
        assert_eq!(h.s.pending_count(), 0);
    }

    #[test]
    fn unlink_after_completion_race() {
        // URB completed in the kernel but not yet reaped when CMD_UNLINK arrives.
        let mut h = Harness::imported();
        h.cmd(6, DIR_IN, 1, 32, [0; 8], &[]).unwrap();
        h.be.complete(6, 0, &[1, 2, 3], 3);
        h.unlink(7, 6);
        h.reap();
        assert_eq!(h.replies(), vec![UrbReply::Unlink { seqnum: 7, status: 0 }]);
    }

    #[test]
    fn unlink_of_unknown_seqnum_replies_zero() {
        let mut h = Harness::imported();
        h.unlink(9, 1234);
        assert!(h.be.discarded.is_empty());
        assert_eq!(h.replies(), vec![UrbReply::Unlink { seqnum: 9, status: 0 }]);
    }

    #[test]
    fn duplicate_seqnum_is_protocol_error() {
        let mut h = Harness::imported();
        h.cmd(6, DIR_IN, 1, 32, [0; 8], &[]).unwrap();
        assert_eq!(h.cmd(6, DIR_IN, 1, 32, [0; 8], &[]), Err(CloseReason::DuplicateSeqnum(6)));
    }

    #[test]
    fn malformed_data_mid_session_then_abort_discards_everything() {
        let mut h = Harness::imported();
        for seq in [2u32, 4, 6] {
            h.cmd(seq, DIR_IN, 1, 32, [0; 8], &[]).unwrap();
        }
        let garbage = [0xde; 48];
        assert!(matches!(h.feed(&garbage), Err(CloseReason::Protocol(_))));
        assert_eq!(h.s.abort(&mut h.be), 3);
        let mut discarded = h.be.discarded.clone();
        discarded.sort();
        assert_eq!(discarded, vec![2, 4, 6]);
        h.reap();
        assert!(h.out.is_empty(), "no output after abort");
        assert_eq!(h.s.pending_count(), 0);
        assert_eq!(h.be.in_flight(), 0);
    }

    #[test]
    fn submit_error_replied_and_device_gone_closes() {
        let mut h = Harness::imported();
        h.be.fail_submit = Some(-EPIPE);
        h.cmd(2, DIR_IN, 1, 32, [0; 8], &[]).unwrap();
        h.be.fail_submit = None;
        h.be.gone = true;
        assert_eq!(h.cmd(4, DIR_IN, 1, 32, [0; 8], &[]), Err(CloseReason::DeviceGone));
        let r = h.replies();
        assert!(matches!(r[0], UrbReply::Submit { seqnum: 2, status, .. } if status == -EPIPE));
        assert!(matches!(r[1], UrbReply::Submit { seqnum: 4, status, .. } if status == -ENODEV));
    }

    #[test]
    fn completion_errors_forwarded() {
        let mut h = Harness::imported();
        h.cmd(2, DIR_IN, 1, 32, [0; 8], &[]).unwrap();
        h.be.complete(2, -EPIPE, &[], 0);
        h.reap();
        assert!(matches!(h.replies()[0], UrbReply::Submit { seqnum: 2, status, actual_length: 0, .. } if status == -EPIPE));
        assert_eq!(h.stats.errors, 1);
    }

    #[test]
    fn pending_limit_enforced() {
        let mut h = Harness::imported();
        for i in 0..MAX_PENDING as u32 {
            h.cmd(i * 2 + 2, DIR_IN, 1, 32, [0; 8], &[]).unwrap();
        }
        assert_eq!(h.cmd(99_999, DIR_IN, 1, 32, [0; 8], &[]), Err(CloseReason::TooManyPending));
    }

    #[test]
    fn steady_state_interrupt_loop_reuses_capacity() {
        // After warm-up, a submit/complete cycle must not grow any session-owned buffer.
        let mut h = Harness::imported();
        let mut out_cap = 0;
        let mut rx_cap = 0;
        for i in 0..2000u32 {
            let seq = 2 * i + 2;
            h.cmd(seq, DIR_IN, 1, 32, [0; 8], &[]).unwrap();
            h.be.complete(seq as u64, 0, &[0u8; 20], 20);
            h.reap();
            h.out.clear();
            h.be.submitted.clear();
            h.ins.clear();
            if i == 100 {
                out_cap = h.out.capacity();
                rx_cap = h.s.rx.capacity();
            }
        }
        assert_eq!(h.out.capacity(), out_cap);
        assert_eq!(h.s.rx.capacity(), rx_cap);
        assert_eq!(h.stats.interrupt_in_reports, 2000);
    }
}
