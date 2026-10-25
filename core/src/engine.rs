// SPDX-License-Identifier: GPL-3.0-or-later
//! Connection manager: admission, the single importing session, eviction, completion routing
//! and output buffering. Pure logic — the platform driver (`server.rs`) owns the sockets and calls
//! in here, which keeps every lifecycle rule testable on any host.

use std::net::IpAddr;
use std::time::Instant;

use crate::acl::{canonical, AccessPolicy};
use crate::backend::UsbBackend;
use crate::descriptors::DeviceInfo;
use crate::proto::{ST_DEV_BUSY, ST_DEV_ERR};
use crate::session::{Flow, Phase, Session};
use crate::stats::{us_between, Stats};
use crate::{log_info, log_warn};

pub type ConnId = u32;

/// A client that stops reading is dropped once this much reply data is queued for it.
pub const MAX_QUEUED_OUTPUT: usize = 1 << 20;
/// Simultaneous TCP connections (one importer plus short-lived device-list queries).
pub const MAX_CONNECTIONS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbEvent {
    Ok,
    DeviceGone,
}

struct Conn {
    id: ConnId,
    peer: IpAddr,
    session: Session,
    out: Vec<u8>,
    out_pos: usize,
    close_after_flush: bool,
    dead: bool,
    /// When the oldest not-yet-sent completion reply was produced (turnaround metric).
    unsent_since: Option<Instant>,
}

impl Conn {
    fn queued(&self) -> usize {
        self.out.len() - self.out_pos
    }
}

pub struct Engine {
    dev: DeviceInfo,
    policy: AccessPolicy,
    conns: Vec<Conn>,
    next_id: ConnId,
    next_generation: u32,
    imported: Option<ConnId>,
    device_gone: bool,
    pub stats: Stats,
}

impl Engine {
    pub fn new(dev: DeviceInfo, policy: AccessPolicy) -> Self {
        Self {
            dev,
            policy,
            conns: Vec::with_capacity(MAX_CONNECTIONS),
            next_id: 1,
            next_generation: 1,
            imported: None,
            device_gone: false,
            stats: Stats::default(),
        }
    }

    pub fn device(&self) -> &DeviceInfo {
        &self.dev
    }

    pub fn connection_count(&self) -> usize {
        self.conns.len()
    }

    pub fn imported_peer(&self) -> Option<IpAddr> {
        let id = self.imported?;
        self.conns.iter().find(|c| c.id == id).map(|c| c.peer)
    }

    /// True while the connection exists and is not being torn down.
    pub fn is_alive(&self, id: ConnId) -> bool {
        self.idx(id).is_some_and(|i| !self.conns[i].dead)
    }

    pub fn is_device_gone(&self) -> bool {
        self.device_gone
    }

    fn idx(&self, id: ConnId) -> Option<usize> {
        self.conns.iter().position(|c| c.id == id)
    }

    /// Decide whether a new TCP peer may talk to us. `None` means close it immediately.
    pub fn admit(&mut self, peer: IpAddr) -> Option<ConnId> {
        let peer = canonical(peer);
        if !self.policy.admits(peer) {
            self.stats.rejected_peers += 1;
            log_warn!("rejected connection from {peer} (not allowed by local-network policy)");
            return None;
        }
        if self.conns.len() >= MAX_CONNECTIONS {
            self.stats.rejected_peers += 1;
            log_warn!("rejected connection from {peer}: too many connections");
            return None;
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.conns.push(Conn {
            id,
            peer,
            session: Session::with_generation(generation),
            out: Vec::with_capacity(4096),
            out_pos: 0,
            close_after_flush: false,
            dead: false,
            unsent_since: None,
        });
        Some(id)
    }

    /// Bytes arrived from a client.
    pub fn on_data(&mut self, id: ConnId, data: &[u8], be: &mut dyn UsbBackend) {
        let Some(i) = self.idx(id) else { return };
        let mut input = data;
        loop {
            let c = &mut self.conns[i];
            if c.dead || c.close_after_flush {
                return;
            }
            let r = c.session.feed(input, &self.dev, be, &mut c.out, &mut self.stats);
            input = &[];
            match r {
                Ok(Flow::Continue) => break,
                Ok(Flow::CloseAfterFlush) => {
                    c.close_after_flush = true;
                    break;
                }
                Ok(Flow::ImportRequested) => {
                    self.decide_import(i, be);
                    // loop: feed again to process anything pipelined after the request
                }
                Err(reason) => {
                    log_warn!("closing session with {}: {reason}", c.peer);
                    if reason == crate::session::CloseReason::DeviceGone {
                        self.device_gone = true;
                    }
                    self.kill(i, be);
                    break;
                }
            }
        }
        self.enforce_output_limit(i, be);
    }

    fn decide_import(&mut self, i: usize, be: &mut dyn UsbBackend) {
        let peer = self.conns[i].peer;
        if self.device_gone {
            let c = &mut self.conns[i];
            c.session.reject_import(ST_DEV_ERR, &mut c.out);
            return;
        }
        if let Some(cur) = self.imported {
            match self.idx(cur) {
                Some(j) if self.conns[j].peer == peer => {
                    // Same machine re-attaching (e.g. after a network blip it has not noticed
                    // yet on our side): the newer connection wins.
                    log_info!("client {peer} re-imported; replacing its previous session");
                    self.kill(j, be);
                }
                Some(_) => {
                    log_warn!("import from {peer} refused: device in use");
                    let c = &mut self.conns[i];
                    c.session.reject_import(ST_DEV_BUSY, &mut c.out);
                    return;
                }
                None => self.imported = None,
            }
        }
        let c = &mut self.conns[i];
        c.session.accept_import(&self.dev, &mut c.out);
        self.imported = Some(c.id);
        self.stats.sessions += 1;
        self.stats.reset_report_clock();
        log_info!("client {peer} attached {}", self.dev.record.busid);
    }

    /// Abort a connection's session (discarding its transfers) and mark it for closing.
    fn kill(&mut self, i: usize, be: &mut dyn UsbBackend) {
        let c = &mut self.conns[i];
        if c.session.phase() == Phase::Imported || c.session.pending_count() > 0 {
            let n = c.session.abort(be);
            if n > 0 {
                log_info!("discarded {n} pending transfer(s) of {}", c.peer);
            }
        } else {
            c.session.abort(be);
        }
        c.dead = true;
        if self.imported == Some(c.id) {
            self.imported = None;
        }
    }

    fn enforce_output_limit(&mut self, i: usize, be: &mut dyn UsbBackend) {
        if i < self.conns.len() && self.conns[i].queued() > MAX_QUEUED_OUTPUT {
            log_warn!("client {} is not reading replies; closing", self.conns[i].peer);
            self.kill(i, be);
        }
    }

    /// The USB fd is readable/hung up: route completions to the importing session.
    pub fn on_usb_ready(&mut self, be: &mut dyn UsbBackend, now: Instant) -> UsbEvent {
        let target = self.imported.and_then(|id| self.idx(id));
        let stats = &mut self.stats;
        let result = match target {
            Some(i) => {
                let c = &mut self.conns[i];
                let before = c.out.len();
                let r = be.reap(&mut |comp| {
                    if !c.session.on_completion(&comp, &mut c.out, stats, now) {
                        stats.completions += 1; // stale transfer of an earlier session
                    }
                });
                if c.out.len() > before && c.unsent_since.is_none() {
                    c.unsent_since = Some(now);
                }
                r
            }
            None => be.reap(&mut |_| stats.completions += 1),
        };
        if let Some(i) = target {
            self.enforce_output_limit(i, be);
        }
        match result {
            Ok(()) => UsbEvent::Ok,
            Err(e) => {
                log_warn!("USB device error {e}: treating as disconnected");
                self.device_gone = true;
                if let Some(i) = self.imported.and_then(|id| self.idx(id)) {
                    self.kill(i, be);
                }
                UsbEvent::DeviceGone
            }
        }
    }

    /// Bytes waiting to be written to the client's socket.
    pub fn pending_output(&self, id: ConnId) -> &[u8] {
        match self.idx(id) {
            Some(i) if !self.conns[i].dead => &self.conns[i].out[self.conns[i].out_pos..],
            _ => &[],
        }
    }

    /// The driver wrote `n` bytes of `pending_output`.
    pub fn advance_output(&mut self, id: ConnId, n: usize, now: Instant) {
        let Some(i) = self.idx(id) else { return };
        let c = &mut self.conns[i];
        c.out_pos = (c.out_pos + n).min(c.out.len());
        if c.out_pos == c.out.len() {
            c.out.clear(); // keeps capacity
            c.out_pos = 0;
            if let Some(t) = c.unsent_since.take() {
                self.stats.turnaround.record(us_between(t, now));
            }
        }
    }

    /// The peer closed the connection or the socket failed.
    pub fn on_peer_closed(&mut self, id: ConnId, be: &mut dyn UsbBackend) {
        if let Some(i) = self.idx(id) {
            if self.imported == Some(id) {
                log_info!("client {} detached", self.conns[i].peer);
            }
            self.kill(i, be);
        }
    }

    /// Remove connections that are finished. Returns their ids so the driver can drop sockets.
    pub fn take_closable(&mut self, be: &mut dyn UsbBackend) -> Vec<ConnId> {
        let mut closed = Vec::new();
        let mut i = 0;
        while i < self.conns.len() {
            let c = &self.conns[i];
            if c.dead || (c.close_after_flush && c.queued() == 0) {
                if !c.dead {
                    self.kill(i, be);
                }
                closed.push(self.conns.remove(i).id);
            } else {
                i += 1;
            }
        }
        closed
    }

    /// Stop everything: abort all sessions and forget all connections.
    pub fn shutdown(&mut self, be: &mut dyn UsbBackend) -> Vec<ConnId> {
        for i in 0..self.conns.len() {
            self.kill(i, be);
        }
        self.conns.drain(..).map(|c| c.id).collect()
    }

    pub fn status_json(&self, running: bool) -> String {
        let (state, client) = if !running {
            ("stopped", String::new())
        } else if self.device_gone {
            ("device_gone", String::new())
        } else if let Some(p) = self.imported_peer() {
            ("attached", p.to_string())
        } else {
            ("listening", String::new())
        };
        self.stats.to_json(state, &client, "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::mock::MockBackend;
    use crate::descriptors;
    use crate::errno::*;
    use crate::proto::*;

    const PC: &str = "192.168.15.133";
    const OTHER: &str = "192.168.15.50";

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn engine() -> Engine {
        let dev = descriptors::parse(&descriptors::tests::xbox360_raw(), "1-1", "/dev/bus/usb/001/005", 1, 5, 0).unwrap();
        Engine::new(dev, AccessPolicy::default())
    }

    fn import_bytes() -> Vec<u8> {
        let mut b = Vec::new();
        encode_op_request(&mut b, &OpRequest::Import { busid: "1-1".into() });
        b
    }

    fn submit_bytes(seq: u32, ep: u32) -> Vec<u8> {
        let mut b = Vec::new();
        let c = CmdSubmit {
            seqnum: seq,
            devid: 0x10005,
            direction: DIR_IN,
            ep,
            transfer_flags: 0,
            transfer_buffer_length: 32,
            start_frame: 0,
            number_of_packets: -1,
            interval: 4,
            setup: [0; 8],
        };
        encode_cmd_submit(&mut b, &c, &[]);
        b
    }

    /// Admit + import; returns id with the import reply already consumed.
    fn attach(e: &mut Engine, be: &mut MockBackend, peer: &str) -> ConnId {
        let id = e.admit(ip(peer)).unwrap();
        e.on_data(id, &import_bytes(), be);
        let out = e.pending_output(id).to_vec();
        assert_eq!(&out[..8], &[0x01, 0x11, 0x00, 0x03, 0, 0, 0, 0], "import accepted");
        e.advance_output(id, out.len(), Instant::now());
        id
    }

    fn flush(e: &mut Engine, id: ConnId) -> Vec<u8> {
        let out = e.pending_output(id).to_vec();
        e.advance_output(id, out.len(), Instant::now());
        out
    }

    #[test]
    fn public_peer_rejected_before_any_protocol() {
        let mut e = engine();
        assert_eq!(e.admit(ip("8.8.8.8")), None);
        assert_eq!(e.admit(ip("100.64.1.1")), None);
        assert_eq!(e.stats.rejected_peers, 2);
        assert_eq!(e.connection_count(), 0);
    }

    #[test]
    fn allowed_client_only() {
        let dev = descriptors::parse(&descriptors::tests::xbox360_raw(), "1-1", "p", 1, 5, 0).unwrap();
        let mut e = Engine::new(dev, AccessPolicy { allowed_client: Some(ip(PC)) });
        assert!(e.admit(ip(OTHER)).is_none());
        assert!(e.admit(ip(PC)).is_some());
    }

    #[test]
    fn connection_limit() {
        let mut e = engine();
        for _ in 0..MAX_CONNECTIONS {
            assert!(e.admit(ip(PC)).is_some());
        }
        assert!(e.admit(ip(PC)).is_none());
    }

    #[test]
    fn devlist_while_attached_does_not_disturb_session() {
        let mut e = engine();
        let mut be = MockBackend::default();
        let a = attach(&mut e, &mut be, PC);
        e.on_data(a, &submit_bytes(2, 1), &mut be);

        let q = e.admit(ip(PC)).unwrap();
        let mut b = Vec::new();
        encode_op_request(&mut b, &OpRequest::DevList);
        e.on_data(q, &b, &mut be);
        let reply = flush(&mut e, q);
        assert_eq!(&reply[..12], &[0x01, 0x11, 0x00, 0x05, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(e.take_closable(&mut be), vec![q]);
        assert_eq!(e.imported_peer(), Some(ip(PC)));
        assert!(be.discarded.is_empty());
    }

    #[test]
    fn second_importer_from_other_machine_is_busy() {
        let mut e = engine();
        let mut be = MockBackend::default();
        attach(&mut e, &mut be, PC);
        let b = e.admit(ip(OTHER)).unwrap();
        e.on_data(b, &import_bytes(), &mut be);
        assert_eq!(flush(&mut e, b), vec![0x01, 0x11, 0x00, 0x03, 0, 0, 0, ST_DEV_BUSY as u8]);
        assert_eq!(e.take_closable(&mut be), vec![b]);
        assert_eq!(e.imported_peer(), Some(ip(PC)));
    }

    #[test]
    fn same_machine_reimport_evicts_old_session_and_ignores_its_stale_transfers() {
        let mut e = engine();
        let mut be = MockBackend::default();
        let old = attach(&mut e, &mut be, PC);
        e.on_data(old, &submit_bytes(6, 1), &mut be);
        let old_tag = be.submitted[0].tag;

        let new = attach(&mut e, &mut be, PC);
        assert_eq!(be.discarded, vec![old_tag]);
        assert_eq!(e.take_closable(&mut be), vec![old]);

        // New session reuses seqnum 6; the old discarded URB completes afterwards.
        e.on_data(new, &submit_bytes(6, 1), &mut be);
        let new_tag = be.submitted[1].tag;
        assert_ne!(old_tag, new_tag);
        be.complete(new_tag, 0, &[1, 2, 3], 3);
        assert_eq!(e.on_usb_ready(&mut be, Instant::now()), UsbEvent::Ok);
        let out = flush(&mut e, new);
        // exactly one RET_SUBMIT for the new session (the stale ECONNRESET completion is dropped)
        let (r, n) = parse_urb_reply(&out, |_| true).unwrap().unwrap();
        assert_eq!(n, out.len());
        assert_eq!(r, UrbReply::Submit { seqnum: 6, status: 0, actual_length: 3, number_of_packets: -1, data: vec![1, 2, 3] });
        assert_eq!(be.in_flight(), 0);
    }

    #[test]
    fn client_disconnect_with_pending_transfers_cleans_up() {
        let mut e = engine();
        let mut be = MockBackend::default();
        let a = attach(&mut e, &mut be, PC);
        for s in [2, 4, 6, 8] {
            e.on_data(a, &submit_bytes(s, 1), &mut be);
        }
        assert_eq!(be.in_flight(), 4);
        e.on_peer_closed(a, &mut be);
        assert_eq!(be.discarded.len(), 4);
        assert_eq!(e.take_closable(&mut be), vec![a]);
        assert_eq!(e.on_usb_ready(&mut be, Instant::now()), UsbEvent::Ok);
        assert_eq!(be.in_flight(), 0);
        assert_eq!(e.imported_peer(), None);
        // device is free again
        attach(&mut e, &mut be, OTHER);
    }

    #[test]
    fn malformed_client_data_closes_session() {
        let mut e = engine();
        let mut be = MockBackend::default();
        let a = attach(&mut e, &mut be, PC);
        e.on_data(a, &submit_bytes(2, 1), &mut be);
        e.on_data(a, &[0xff; 48], &mut be);
        assert_eq!(e.pending_output(a), &[] as &[u8]);
        assert_eq!(e.take_closable(&mut be), vec![a]);
        assert_eq!(be.discarded.len(), 1);
        assert_eq!(e.imported_peer(), None);
    }

    #[test]
    fn device_gone_during_session() {
        let mut e = engine();
        let mut be = MockBackend::default();
        let a = attach(&mut e, &mut be, PC);
        e.on_data(a, &submit_bytes(2, 1), &mut be);
        let tag = be.submitted[0].tag;
        be.complete(tag, -ESHUTDOWN, &[], 0);
        be.gone = true;
        assert_eq!(e.on_usb_ready(&mut be, Instant::now()), UsbEvent::DeviceGone);
        assert!(e.is_device_gone());
        assert_eq!(e.take_closable(&mut be), vec![a]);
        // a later import is refused with a device error instead of hanging
        let b = e.admit(ip(PC)).unwrap();
        e.on_data(b, &import_bytes(), &mut be);
        assert_eq!(flush(&mut e, b), vec![0x01, 0x11, 0x00, 0x03, 0, 0, 0, ST_DEV_ERR as u8]);
        assert!(e.status_json(true).contains("\"state\":\"device_gone\""));
    }

    #[test]
    fn rapid_reconnect_100_times_no_leaks() {
        let mut e = engine();
        let mut be = MockBackend::default();
        for round in 0..100u32 {
            let peer = if round % 2 == 0 { PC } else { OTHER };
            let a = attach(&mut e, &mut be, peer);
            for s in 0..3 {
                e.on_data(a, &submit_bytes(2 + 2 * s, 1), &mut be);
            }
            // complete one, leave two pending
            let t = be.pending[0];
            be.complete(t, 0, &[0; 20], 20);
            e.on_usb_ready(&mut be, Instant::now());
            flush(&mut e, a);
            e.on_peer_closed(a, &mut be);
            assert_eq!(e.take_closable(&mut be), vec![a]);
            e.on_usb_ready(&mut be, Instant::now());
            assert_eq!(be.in_flight(), 0, "round {round}");
            assert_eq!(e.connection_count(), 0);
        }
        assert_eq!(e.stats.sessions, 100);
        assert_eq!(e.stats.interrupt_in_reports, 100);
    }

    #[test]
    fn client_not_reading_is_dropped() {
        let mut e = engine();
        let mut be = MockBackend::default();
        let a = attach(&mut e, &mut be, PC);
        let mut seq = 2u32;
        while e.is_alive(a) {
            e.on_data(a, &submit_bytes(seq, 1), &mut be);
            let t = *be.pending.last().unwrap();
            be.complete(t, 0, &[0; 32], 32);
            e.on_usb_ready(&mut be, Instant::now());
            seq += 2;
        }
        assert!(seq as usize / 2 * 80 > MAX_QUEUED_OUTPUT);
        assert_eq!(e.take_closable(&mut be), vec![a]);
    }

    #[test]
    fn shutdown_aborts_everything() {
        let mut e = engine();
        let mut be = MockBackend::default();
        let a = attach(&mut e, &mut be, PC);
        e.on_data(a, &submit_bytes(2, 1), &mut be);
        let q = e.admit(ip(OTHER)).unwrap();
        let mut ids = e.shutdown(&mut be);
        ids.sort();
        assert_eq!(ids, vec![a, q]);
        assert_eq!(be.discarded.len(), 1);
        e.on_usb_ready(&mut be, Instant::now());
        assert_eq!(be.in_flight(), 0);
        assert!(e.status_json(false).contains("\"state\":\"stopped\""));
    }

    #[test]
    fn turnaround_recorded_on_flush() {
        let mut e = engine();
        let mut be = MockBackend::default();
        let a = attach(&mut e, &mut be, PC);
        e.on_data(a, &submit_bytes(2, 1), &mut be);
        let t = be.pending[0];
        be.complete(t, 0, &[0; 20], 20);
        let t0 = Instant::now();
        e.on_usb_ready(&mut be, t0);
        let n = e.pending_output(a).len();
        e.advance_output(a, n / 2, t0 + std::time::Duration::from_micros(30));
        assert_eq!(e.stats.turnaround.summary().n, 0);
        e.advance_output(a, n - n / 2, t0 + std::time::Duration::from_micros(80));
        let s = e.stats.turnaround.summary();
        assert_eq!((s.n, s.min), (1, 80));
    }
}
