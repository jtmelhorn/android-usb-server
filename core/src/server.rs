// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux/Android driver for the [`Engine`]: one thread, one `poll()` over
//! {wake pipe, listener, USB fd, client sockets}. USB completions are reaped and their replies
//! written to the socket in the same loop iteration — there are no queues between threads and no
//! timed polling on the input path (the 250 ms poll timeout only refreshes the status snapshot).

use std::io::{self, ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::acl::AccessPolicy;
use crate::backend::UsbBackend;
use crate::descriptors::DeviceInfo;
use crate::engine::{ConnId, Engine, UsbEvent};
use crate::{log_info, log_warn};

pub const DEFAULT_PORT: u16 = 3240;
const STATUS_PERIOD: Duration = Duration::from_millis(250);
const READ_CHUNK: usize = 64 * 1024;

pub struct ServerConfig {
    pub port: u16,
    pub policy: AccessPolicy,
}

pub struct ServerHandle {
    thread: Option<JoinHandle<()>>,
    wake_w: RawFd,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
    pub local_port: u16,
}

impl ServerHandle {
    pub fn status(&self) -> String {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().map_or(true, |t| t.is_finished())
    }

    /// Stop the server thread and wait for it (bounded by the URB drain timeout).
    pub fn stop(&mut self) {
        if let Some(t) = self.thread.take() {
            self.stop.store(true, Ordering::SeqCst);
            // SAFETY: writing one byte to our own pipe.
            unsafe { libc::write(self.wake_w, b"x".as_ptr() as *const libc::c_void, 1) };
            let _ = t.join();
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop();
        // SAFETY: closing our own pipe end exactly once.
        unsafe { libc::close(self.wake_w) };
    }
}

fn bind(port: u16) -> io::Result<TcpListener> {
    // Dual-stack first (IPv4-mapped peers are normalised by the ACL); IPv4-only as fallback.
    TcpListener::bind(SocketAddr::from(([0u16; 8], port))).or_else(|_| TcpListener::bind(SocketAddr::from(([0u8; 4], port))))
}

fn set_opt(fd: RawFd, level: libc::c_int, name: libc::c_int, value: libc::c_int) {
    // SAFETY: valid fd and pointer to a c_int of the declared size.
    unsafe {
        libc::setsockopt(
            fd,
            level,
            name,
            &value as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }
}

fn configure_client(s: &TcpStream) -> io::Result<()> {
    s.set_nonblocking(true)?;
    s.set_nodelay(true)?;
    let fd = s.as_raw_fd();
    // Detect a vanished client (PC asleep, Wi-Fi gone) within ~16 s so it can re-attach.
    set_opt(fd, libc::SOL_SOCKET, libc::SO_KEEPALIVE, 1);
    set_opt(fd, libc::IPPROTO_TCP, libc::TCP_KEEPIDLE, 10);
    set_opt(fd, libc::IPPROTO_TCP, libc::TCP_KEEPINTVL, 2);
    set_opt(fd, libc::IPPROTO_TCP, libc::TCP_KEEPCNT, 3);
    set_opt(fd, libc::IPPROTO_TCP, libc::TCP_USER_TIMEOUT, 10_000);
    Ok(())
}

fn make_pipe() -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: valid array of two ints.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((fds[0], fds[1]))
}

/// Raise the server thread's scheduling priority if the platform allows it (best effort).
fn raise_priority() {
    // SAFETY: plain syscalls on the current thread.
    unsafe {
        let tid = libc::gettid();
        if libc::setpriority(libc::PRIO_PROCESS, tid as _, -10) == 0 {
            log_info!("server thread nice set to -10");
        } else {
            log_warn!("could not raise server thread priority (errno {})", io::Error::last_os_error());
        }
    }
}

/// Bind and start serving `dev` through `backend`. Binding happens before returning so
/// "port in use" is reported to the caller.
pub fn start<B: UsbBackend + Send + 'static>(
    backend: B,
    usb_fd: RawFd,
    dev: DeviceInfo,
    cfg: ServerConfig,
) -> io::Result<ServerHandle> {
    // usbdevfs signals "completions ready" as POLLOUT.
    start_inner(backend, usb_fd, libc::POLLOUT, dev, cfg)
}

fn start_inner<B: UsbBackend + Send + 'static>(
    backend: B,
    usb_fd: RawFd,
    usb_events: libc::c_short,
    dev: DeviceInfo,
    cfg: ServerConfig,
) -> io::Result<ServerHandle> {
    let listener = bind(cfg.port)?;
    listener.set_nonblocking(true)?;
    let local_port = listener.local_addr()?.port();
    let (wake_r, wake_w) = make_pipe()?;
    let stop = Arc::new(AtomicBool::new(false));
    let engine = Engine::new(dev, cfg.policy);
    let status = Arc::new(Mutex::new(engine.status_json(true)));
    let (stop2, status2) = (stop.clone(), status.clone());
    log_info!("USB/IP server listening on port {local_port}");
    let thread = std::thread::Builder::new().name("usbip-server".into()).spawn(move || {
        raise_priority();
        let mut d =
            Driver { engine, backend, usb_fd, usb_events, listener, wake_r, stop: stop2, status: status2, conns: Vec::new() };
        d.run();
        // SAFETY: closing our own pipe end once.
        unsafe { libc::close(wake_r) };
    })?;
    Ok(ServerHandle { thread: Some(thread), wake_w, stop, status, local_port })
}

struct Driver<B: UsbBackend> {
    engine: Engine,
    backend: B,
    usb_fd: RawFd,
    usb_events: libc::c_short,
    listener: TcpListener,
    wake_r: RawFd,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
    conns: Vec<(ConnId, TcpStream)>,
}

impl<B: UsbBackend> Driver<B> {
    fn publish(&self, json: String) {
        if let Ok(mut s) = self.status.lock() {
            *s = json;
        }
    }

    fn run(&mut self) {
        let mut buf = vec![0u8; READ_CHUNK];
        let mut fds: Vec<libc::pollfd> = Vec::with_capacity(8);
        let mut last_publish = Instant::now();
        let mut last_peer = None;
        let mut device_gone = false;

        while !self.stop.load(Ordering::Relaxed) {
            fds.clear();
            fds.push(libc::pollfd { fd: self.wake_r, events: libc::POLLIN, revents: 0 });
            fds.push(libc::pollfd { fd: self.listener.as_raw_fd(), events: libc::POLLIN, revents: 0 });
            fds.push(libc::pollfd { fd: self.usb_fd, events: self.usb_events, revents: 0 });
            for (id, s) in &self.conns {
                let mut ev = libc::POLLIN;
                if !self.engine.pending_output(*id).is_empty() {
                    ev |= libc::POLLOUT;
                }
                fds.push(libc::pollfd { fd: s.as_raw_fd(), events: ev, revents: 0 });
            }
            // SAFETY: fds is a valid array of initialised pollfd structs.
            let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, STATUS_PERIOD.as_millis() as i32) };
            if n < 0 && io::Error::last_os_error().kind() != ErrorKind::Interrupted {
                log_warn!("poll failed: {}", io::Error::last_os_error());
                break;
            }
            let now = Instant::now();

            if fds[0].revents != 0 {
                let mut sink = [0u8; 16];
                // SAFETY: reading into a local buffer from our own non-blocking pipe.
                while unsafe { libc::read(self.wake_r, sink.as_mut_ptr() as *mut libc::c_void, sink.len()) } > 0 {}
                if self.stop.load(Ordering::Relaxed) {
                    break;
                }
            }

            // 1. USB completions first: freshest input goes out in this iteration.
            if fds[2].revents != 0 && self.engine.on_usb_ready(&mut self.backend, now) == UsbEvent::DeviceGone {
                device_gone = true;
            }

            // 2. Client input.
            for (k, (id, stream)) in self.conns.iter_mut().enumerate() {
                let re = fds.get(3 + k).map_or(0, |p| p.revents);
                if re == 0 {
                    continue;
                }
                if re & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                    loop {
                        match stream.read(&mut buf) {
                            Ok(0) => {
                                self.engine.on_peer_closed(*id, &mut self.backend);
                                break;
                            }
                            Ok(n) => self.engine.on_data(*id, &buf[..n], &mut self.backend),
                            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                            Err(_) => {
                                self.engine.on_peer_closed(*id, &mut self.backend);
                                break;
                            }
                        }
                        if !self.engine.is_alive(*id) {
                            break;
                        }
                    }
                }
            }

            // 3. New connections.
            if fds[1].revents != 0 {
                loop {
                    match self.listener.accept() {
                        Ok((stream, addr)) => match self.engine.admit(addr.ip()) {
                            Some(id) if configure_client(&stream).is_ok() => self.conns.push((id, stream)),
                            Some(id) => self.engine.on_peer_closed(id, &mut self.backend),
                            None => drop(stream),
                        },
                        Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                        Err(e) => {
                            log_warn!("accept failed: {e}");
                            break;
                        }
                    }
                }
            }

            // 4. Write replies.
            for (id, stream) in &mut self.conns {
                loop {
                    let out = self.engine.pending_output(*id);
                    if out.is_empty() {
                        break;
                    }
                    match stream.write(out) {
                        Ok(n) => self.engine.advance_output(*id, n, Instant::now()),
                        Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                        Err(_) => {
                            self.engine.on_peer_closed(*id, &mut self.backend);
                            break;
                        }
                    }
                }
            }

            // 5. Drop finished connections.
            for id in self.engine.take_closable(&mut self.backend) {
                if let Some(pos) = self.conns.iter().position(|(c, _)| *c == id) {
                    let (_, s) = self.conns.swap_remove(pos);
                    let _ = s.shutdown(Shutdown::Both);
                }
            }

            if device_gone {
                break;
            }
            let peer = self.engine.imported_peer();
            if peer != last_peer || now.duration_since(last_publish) >= STATUS_PERIOD {
                last_peer = peer;
                last_publish = now;
                self.publish(self.engine.status_json(true));
            }
        }

        // Teardown: cancel everything and wait (bounded) for the kernel to return the URBs.
        for id in self.engine.shutdown(&mut self.backend) {
            if let Some(pos) = self.conns.iter().position(|(c, _)| *c == id) {
                let (_, s) = self.conns.swap_remove(pos);
                let _ = s.shutdown(Shutdown::Both);
            }
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        while self.backend.in_flight() > 0 && Instant::now() < deadline {
            let mut p = libc::pollfd { fd: self.usb_fd, events: self.usb_events, revents: 0 };
            // SAFETY: valid single pollfd.
            unsafe { libc::poll(&mut p, 1, 20) };
            self.engine.on_usb_ready(&mut self.backend, Instant::now());
        }
        let json = if device_gone { self.engine.status_json(true) } else { self.engine.status_json(false) };
        self.publish(json);
        log_info!("USB/IP server stopped{}", if device_gone { " (device disconnected)" } else { "" });
    }
}

/// Socket-level tests of the driver with a scripted backend. Linux/Android only; on the Shield run
/// the cross-compiled test binary via adb (see docs/testing.md).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Completion, Submit};
    use crate::descriptors;
    use crate::errno::ECONNRESET;
    use crate::proto::*;
    use std::collections::VecDeque;
    use std::net::Ipv4Addr;

    /// Interrupt-IN on 0x81 completes immediately with a 20-byte report; 0x84 never completes
    /// unless discarded. Readiness is signalled through a pipe (POLLIN on its read end).
    struct PipeMock {
        r: RawFd,
        w: RawFd,
        pending: Vec<u64>,
        ready: VecDeque<(u64, i32, Vec<u8>)>,
    }

    impl PipeMock {
        fn new() -> Self {
            let (r, w) = make_pipe().unwrap();
            Self { r, w, pending: vec![], ready: VecDeque::new() }
        }
        fn signal(&self) {
            unsafe { libc::write(self.w, b"x".as_ptr() as *const libc::c_void, 1) };
        }
    }

    impl UsbBackend for PipeMock {
        fn submit(&mut self, s: &Submit<'_>) -> Result<(), i32> {
            self.pending.push(s.tag);
            if s.endpoint == 0x81 {
                let mut report = vec![0u8; 20];
                report[1] = 20;
                report[2] = (s.tag & 0xff) as u8;
                self.ready.push_back((s.tag, 0, report));
                self.signal();
            }
            Ok(())
        }
        fn discard(&mut self, tag: u64) -> bool {
            if self.pending.contains(&tag) && !self.ready.iter().any(|r| r.0 == tag) {
                self.ready.push_back((tag, -ECONNRESET, vec![]));
                self.signal();
                true
            } else {
                false
            }
        }
        fn set_interface(&mut self, _: u16, _: u16) -> Result<(), i32> {
            Ok(())
        }
        fn clear_halt(&mut self, _: u8) -> Result<(), i32> {
            Ok(())
        }
        fn reap(&mut self, f: &mut dyn FnMut(Completion<'_>)) -> Result<(), i32> {
            let mut sink = [0u8; 64];
            while unsafe { libc::read(self.r, sink.as_mut_ptr() as *mut libc::c_void, sink.len()) } > 0 {}
            while let Some((tag, status, data)) = self.ready.pop_front() {
                self.pending.retain(|&t| t != tag);
                f(Completion { tag, status, actual_length: data.len(), in_data: &data });
            }
            Ok(())
        }
        fn in_flight(&self) -> usize {
            self.pending.len()
        }
    }

    fn start_mock() -> ServerHandle {
        let mock = PipeMock::new();
        let fd = mock.r;
        let dev = descriptors::parse(&descriptors::tests::xbox360_raw(), "1-1", "/dev/bus/usb/001/005", 1, 5, 2).unwrap();
        start_inner(mock, fd, libc::POLLIN, dev, ServerConfig { port: 0, policy: AccessPolicy::default() }).unwrap()
    }

    fn connect(port: u16) -> TcpStream {
        let s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.set_nodelay(true).unwrap();
        s
    }

    fn import(s: &mut TcpStream) {
        let mut b = Vec::new();
        encode_op_request(&mut b, &OpRequest::Import { busid: "1-1".into() });
        s.write_all(&b).unwrap();
        let mut reply = vec![0u8; 8 + DEVICE_RECORD_LEN];
        s.read_exact(&mut reply).unwrap();
        assert_eq!(&reply[..8], &[0x01, 0x11, 0x00, 0x03, 0, 0, 0, 0]);
    }

    fn submit_in(s: &mut TcpStream, seq: u32, ep: u32) {
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
        let mut b = Vec::new();
        encode_cmd_submit(&mut b, &c, &[]);
        s.write_all(&b).unwrap();
    }

    fn read_reply(s: &mut TcpStream, with_data: bool) -> UrbReply {
        let mut hdr = [0u8; 48];
        s.read_exact(&mut hdr).unwrap();
        let len = u32::from_be_bytes([hdr[24], hdr[25], hdr[26], hdr[27]]) as usize;
        let mut all = hdr.to_vec();
        if with_data && u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) == RET_SUBMIT {
            let mut d = vec![0u8; len];
            s.read_exact(&mut d).unwrap();
            all.extend_from_slice(&d);
        }
        parse_urb_reply(&all, |_| with_data).unwrap().unwrap().0
    }

    #[test]
    fn devlist_import_stream_unlink_reconnect_stop() {
        let mut h = start_mock();
        let port = h.local_port;

        // device list on its own connection; the server closes it afterwards
        let mut q = connect(port);
        let mut b = Vec::new();
        encode_op_request(&mut b, &OpRequest::DevList);
        q.write_all(&b).unwrap();
        let mut list = Vec::new();
        q.read_to_end(&mut list).unwrap();
        assert_eq!(list.len(), 12 + DEVICE_RECORD_LEN + 16);

        // 500 interrupt reports through a real socket
        let mut c = connect(port);
        import(&mut c);
        for i in 0..500u32 {
            submit_in(&mut c, 2 * i + 2, 1);
            match read_reply(&mut c, true) {
                UrbReply::Submit { seqnum, status: 0, data, .. } => {
                    assert_eq!(seqnum, 2 * i + 2);
                    assert_eq!(data.len(), 20);
                }
                other => panic!("unexpected {other:?}"),
            }
        }

        // unlink a transfer that never completes on its own
        submit_in(&mut c, 5000, 4);
        let mut b = Vec::new();
        encode_cmd_unlink(&mut b, &CmdUnlink { seqnum: 5001, devid: 0x10005, direction: 0, ep: 4, unlink_seqnum: 5000 });
        c.write_all(&b).unwrap();
        assert_eq!(read_reply(&mut c, false), UrbReply::Unlink { seqnum: 5001, status: -ECONNRESET });

        // client vanishes with a transfer pending; reconnect and attach again
        submit_in(&mut c, 6000, 4);
        drop(c);
        std::thread::sleep(Duration::from_millis(100));
        let mut c2 = connect(port);
        import(&mut c2);
        submit_in(&mut c2, 2, 1);
        assert!(matches!(read_reply(&mut c2, true), UrbReply::Submit { seqnum: 2, status: 0, .. }));
        std::thread::sleep(Duration::from_millis(300));
        assert!(h.status().contains("\"state\":\"attached\""), "{}", h.status());

        // stop while attached with a pending transfer: bounded, and the client sees EOF
        submit_in(&mut c2, 4, 4);
        std::thread::sleep(Duration::from_millis(50));
        let t0 = Instant::now();
        h.stop();
        assert!(t0.elapsed() < Duration::from_secs(2));
        let mut rest = Vec::new();
        let _ = c2.read_to_end(&mut rest);
        assert!(h.status().contains("\"state\":\"stopped\""));
    }

    #[test]
    fn port_in_use_reported() {
        let h = start_mock();
        let mock = PipeMock::new();
        let fd = mock.r;
        let dev = descriptors::parse(&descriptors::tests::xbox360_raw(), "1-1", "p", 1, 5, 2).unwrap();
        let r = start_inner(mock, fd, libc::POLLIN, dev, ServerConfig { port: h.local_port, policy: AccessPolicy::default() });
        assert!(r.is_err());
    }
}
