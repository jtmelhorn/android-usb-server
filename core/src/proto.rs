// SPDX-License-Identifier: GPL-3.0-or-later
//! USB/IP 1.1.1 wire format, per Linux `Documentation/usb/usbip_protocol.rst`.
//!
//! All integers are big-endian. Parsers are incremental: they return `Ok(None)` when more bytes
//! are needed, and never panic on arbitrary input.

pub const USBIP_VERSION: u16 = 0x0111;

pub const OP_REQ_DEVLIST: u16 = 0x8005;
pub const OP_REP_DEVLIST: u16 = 0x0005;
pub const OP_REQ_IMPORT: u16 = 0x8003;
pub const OP_REP_IMPORT: u16 = 0x0003;

/// `usbip_op_common.status` values (`usbip_common.h`).
pub const ST_OK: u32 = 0;
pub const ST_NA: u32 = 1;
pub const ST_DEV_BUSY: u32 = 2;
pub const ST_DEV_ERR: u32 = 3;
pub const ST_NODEV: u32 = 4;

pub const CMD_SUBMIT: u32 = 1;
pub const CMD_UNLINK: u32 = 2;
pub const RET_SUBMIT: u32 = 3;
pub const RET_UNLINK: u32 = 4;

pub const DIR_OUT: u32 = 0;
pub const DIR_IN: u32 = 1;

pub const OP_HEADER_LEN: usize = 8;
pub const BUSID_LEN: usize = 32;
pub const PATH_LEN: usize = 256;
pub const DEVICE_RECORD_LEN: usize = 312;
pub const URB_HEADER_LEN: usize = 48;
pub const ISO_DESC_LEN: usize = 16;

/// `number_of_packets` value meaning "not isochronous".
pub const NON_ISO_PACKETS: i32 = -1;
/// Upper bound accepted for a single transfer. A gamepad never needs more than a few KiB; the
/// limit stops a malicious client from making the server allocate unbounded memory.
pub const MAX_TRANSFER_LEN: usize = 1 << 20;
pub const MAX_ISO_PACKETS: i32 = 1024;

/// Linux URB transfer flags carried in CMD_SUBMIT.
pub const URB_SHORT_NOT_OK: u32 = 0x0001;
pub const URB_ZERO_PACKET: u32 = 0x0040;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtoError {
    BadVersion(u16),
    UnknownOp(u16),
    UnknownCommand(u32),
    BadDirection(u32),
    BadEndpoint(u32),
    BadLength(i32),
    BadPacketCount(i32),
}

impl std::fmt::Display for ProtoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtoError::BadVersion(v) => write!(f, "unsupported USB/IP version {v:#06x}"),
            ProtoError::UnknownOp(c) => write!(f, "unknown operation {c:#06x}"),
            ProtoError::UnknownCommand(c) => write!(f, "unknown URB command {c}"),
            ProtoError::BadDirection(d) => write!(f, "invalid direction {d}"),
            ProtoError::BadEndpoint(e) => write!(f, "invalid endpoint {e}"),
            ProtoError::BadLength(l) => write!(f, "invalid transfer length {l}"),
            ProtoError::BadPacketCount(n) => write!(f, "invalid number_of_packets {n}"),
        }
    }
}

#[inline]
fn be16(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}
#[inline]
fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
#[inline]
fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}
#[inline]
fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn put_fixed_str(out: &mut Vec<u8>, s: &str, len: usize) {
    let bytes = s.as_bytes();
    let n = bytes.len().min(len - 1); // always NUL-terminated
    out.extend_from_slice(&bytes[..n]);
    out.resize(out.len() + (len - n), 0);
}

/// Interface triple listed after a device record in OP_REP_DEVLIST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceClass {
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
}

/// `struct usbip_usb_device`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRecord {
    pub path: String,
    pub busid: String,
    pub busnum: u32,
    pub devnum: u32,
    pub speed: u32,
    pub id_vendor: u16,
    pub id_product: u16,
    pub bcd_device: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub configuration_value: u8,
    pub num_configurations: u8,
    pub interfaces: Vec<InterfaceClass>,
}

impl DeviceRecord {
    pub fn devid(&self) -> u32 {
        (self.busnum << 16) | (self.devnum & 0xffff)
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        let start = out.len();
        put_fixed_str(out, &self.path, PATH_LEN);
        put_fixed_str(out, &self.busid, BUSID_LEN);
        put32(out, self.busnum);
        put32(out, self.devnum);
        put32(out, self.speed);
        put16(out, self.id_vendor);
        put16(out, self.id_product);
        put16(out, self.bcd_device);
        out.push(self.device_class);
        out.push(self.device_subclass);
        out.push(self.device_protocol);
        out.push(self.configuration_value);
        out.push(self.num_configurations);
        out.push(self.interfaces.len().min(255) as u8);
        debug_assert_eq!(out.len() - start, DEVICE_RECORD_LEN);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpRequest {
    DevList,
    Import { busid: String },
}

/// Parse an OP_REQ_* message. Returns the request and the number of bytes consumed.
pub fn parse_op_request(buf: &[u8]) -> Result<Option<(OpRequest, usize)>, ProtoError> {
    if buf.len() < OP_HEADER_LEN {
        return Ok(None);
    }
    let version = be16(buf, 0);
    let code = be16(buf, 2);
    if version != USBIP_VERSION {
        return Err(ProtoError::BadVersion(version));
    }
    match code {
        OP_REQ_DEVLIST => Ok(Some((OpRequest::DevList, OP_HEADER_LEN))),
        OP_REQ_IMPORT => {
            let total = OP_HEADER_LEN + BUSID_LEN;
            if buf.len() < total {
                return Ok(None);
            }
            let raw = &buf[OP_HEADER_LEN..total];
            let end = raw.iter().position(|&b| b == 0).unwrap_or(BUSID_LEN);
            let busid = String::from_utf8_lossy(&raw[..end]).into_owned();
            Ok(Some((OpRequest::Import { busid }, total)))
        }
        other => Err(ProtoError::UnknownOp(other)),
    }
}

fn put_op_header(out: &mut Vec<u8>, code: u16, status: u32) {
    put16(out, USBIP_VERSION);
    put16(out, code);
    put32(out, status);
}

pub fn encode_devlist_reply(out: &mut Vec<u8>, devices: &[&DeviceRecord]) {
    put_op_header(out, OP_REP_DEVLIST, ST_OK);
    put32(out, devices.len() as u32);
    for d in devices {
        d.encode(out);
        for i in &d.interfaces {
            out.extend_from_slice(&[i.class, i.subclass, i.protocol, 0]);
        }
    }
}

pub fn encode_import_reply(out: &mut Vec<u8>, status: u32, device: Option<&DeviceRecord>) {
    put_op_header(out, OP_REP_IMPORT, status);
    if status == ST_OK {
        if let Some(d) = device {
            d.encode(out);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CmdSubmit {
    pub seqnum: u32,
    pub devid: u32,
    pub direction: u32,
    pub ep: u32,
    pub transfer_flags: u32,
    pub transfer_buffer_length: u32,
    pub start_frame: i32,
    pub number_of_packets: i32,
    pub interval: i32,
    pub setup: [u8; 8],
}

impl CmdSubmit {
    pub fn is_iso(&self) -> bool {
        self.number_of_packets != 0 && self.number_of_packets != NON_ISO_PACKETS
    }
    pub fn is_in(&self) -> bool {
        self.direction == DIR_IN
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CmdUnlink {
    pub seqnum: u32,
    pub devid: u32,
    pub direction: u32,
    pub ep: u32,
    pub unlink_seqnum: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UrbCommand<'a> {
    /// The slice is the OUT payload (empty for IN transfers).
    Submit(CmdSubmit, &'a [u8]),
    Unlink(CmdUnlink),
}

/// Parse one CMD_SUBMIT / CMD_UNLINK. Returns the command and bytes consumed (header + payload,
/// including any isochronous descriptors, which the session rejects).
pub fn parse_urb_command(buf: &[u8]) -> Result<Option<(UrbCommand<'_>, usize)>, ProtoError> {
    if buf.len() < URB_HEADER_LEN {
        return Ok(None);
    }
    let command = be32(buf, 0);
    let seqnum = be32(buf, 4);
    let devid = be32(buf, 8);
    let direction = be32(buf, 12);
    let ep = be32(buf, 16);
    match command {
        CMD_SUBMIT => {
            if direction != DIR_OUT && direction != DIR_IN {
                return Err(ProtoError::BadDirection(direction));
            }
            if ep > 15 {
                return Err(ProtoError::BadEndpoint(ep));
            }
            let len = be32(buf, 24) as i32;
            if len < 0 || len as usize > MAX_TRANSFER_LEN {
                return Err(ProtoError::BadLength(len));
            }
            let npk = be32(buf, 32) as i32;
            if npk < NON_ISO_PACKETS || npk > MAX_ISO_PACKETS {
                return Err(ProtoError::BadPacketCount(npk));
            }
            let mut setup = [0u8; 8];
            setup.copy_from_slice(&buf[40..48]);
            let cmd = CmdSubmit {
                seqnum,
                devid,
                direction,
                ep,
                transfer_flags: be32(buf, 20),
                transfer_buffer_length: len as u32,
                start_frame: be32(buf, 28) as i32,
                number_of_packets: npk,
                interval: be32(buf, 36) as i32,
                setup,
            };
            let out_len = if direction == DIR_OUT { len as usize } else { 0 };
            let iso_len = if cmd.is_iso() { npk as usize * ISO_DESC_LEN } else { 0 };
            let total = URB_HEADER_LEN + out_len + iso_len;
            if buf.len() < total {
                return Ok(None);
            }
            let data = &buf[URB_HEADER_LEN..URB_HEADER_LEN + out_len];
            Ok(Some((UrbCommand::Submit(cmd, data), total)))
        }
        CMD_UNLINK => Ok(Some((
            UrbCommand::Unlink(CmdUnlink { seqnum, devid, direction, ep, unlink_seqnum: be32(buf, 20) }),
            URB_HEADER_LEN,
        ))),
        other => Err(ProtoError::UnknownCommand(other)),
    }
}

/// RET_SUBMIT header followed by `in_data` (empty for OUT transfers). `devid`, `direction` and
/// `ep` are zero as in the Linux stub; clients identify the request by `seqnum`.
pub fn encode_ret_submit(out: &mut Vec<u8>, seqnum: u32, status: i32, actual_length: u32, in_data: &[u8]) {
    put32(out, RET_SUBMIT);
    put32(out, seqnum);
    put32(out, 0);
    put32(out, 0);
    put32(out, 0);
    put32(out, status as u32);
    put32(out, actual_length);
    put32(out, 0); // start_frame
    put32(out, NON_ISO_PACKETS as u32);
    put32(out, 0); // error_count
    out.extend_from_slice(&[0u8; 8]);
    out.extend_from_slice(in_data);
}

pub fn encode_ret_unlink(out: &mut Vec<u8>, seqnum: u32, status: i32) {
    put32(out, RET_UNLINK);
    put32(out, seqnum);
    put32(out, 0);
    put32(out, 0);
    put32(out, 0);
    put32(out, status as u32);
    out.extend_from_slice(&[0u8; 24]);
}

// ---- Client-side encoders (used by tests and diagnostic tools) ----

pub fn encode_op_request(out: &mut Vec<u8>, req: &OpRequest) {
    match req {
        OpRequest::DevList => put_op_header(out, OP_REQ_DEVLIST, 0),
        OpRequest::Import { busid } => {
            put_op_header(out, OP_REQ_IMPORT, 0);
            put_fixed_str(out, busid, BUSID_LEN);
        }
    }
}

pub fn encode_cmd_submit(out: &mut Vec<u8>, cmd: &CmdSubmit, out_data: &[u8]) {
    put32(out, CMD_SUBMIT);
    put32(out, cmd.seqnum);
    put32(out, cmd.devid);
    put32(out, cmd.direction);
    put32(out, cmd.ep);
    put32(out, cmd.transfer_flags);
    put32(out, cmd.transfer_buffer_length);
    put32(out, cmd.start_frame as u32);
    put32(out, cmd.number_of_packets as u32);
    put32(out, cmd.interval as u32);
    out.extend_from_slice(&cmd.setup);
    out.extend_from_slice(out_data);
}

pub fn encode_cmd_unlink(out: &mut Vec<u8>, cmd: &CmdUnlink) {
    put32(out, CMD_UNLINK);
    put32(out, cmd.seqnum);
    put32(out, cmd.devid);
    put32(out, cmd.direction);
    put32(out, cmd.ep);
    put32(out, cmd.unlink_seqnum);
    out.extend_from_slice(&[0u8; 24]);
}

/// Decoded RET_SUBMIT / RET_UNLINK (client side; tests only need these fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrbReply {
    Submit { seqnum: u32, status: i32, actual_length: u32, number_of_packets: i32, data: Vec<u8> },
    Unlink { seqnum: u32, status: i32 },
}

/// Parse a server reply. `expect_in_data(seqnum)` says whether RET_SUBMIT for that seqnum carries
/// IN data (the direction is not on the wire in replies).
pub fn parse_urb_reply(
    buf: &[u8],
    expect_in_data: impl Fn(u32) -> bool,
) -> Result<Option<(UrbReply, usize)>, ProtoError> {
    if buf.len() < URB_HEADER_LEN {
        return Ok(None);
    }
    let command = be32(buf, 0);
    let seqnum = be32(buf, 4);
    let status = be32(buf, 20) as i32;
    match command {
        RET_SUBMIT => {
            let actual = be32(buf, 24);
            if actual as usize > MAX_TRANSFER_LEN {
                return Err(ProtoError::BadLength(actual as i32));
            }
            let data_len = if expect_in_data(seqnum) { actual as usize } else { 0 };
            if buf.len() < URB_HEADER_LEN + data_len {
                return Ok(None);
            }
            Ok(Some((
                UrbReply::Submit {
                    seqnum,
                    status,
                    actual_length: actual,
                    number_of_packets: be32(buf, 32) as i32,
                    data: buf[URB_HEADER_LEN..URB_HEADER_LEN + data_len].to_vec(),
                },
                URB_HEADER_LEN + data_len,
            )))
        }
        RET_UNLINK => Ok(Some((UrbReply::Unlink { seqnum, status }, URB_HEADER_LEN))),
        other => Err(ProtoError::UnknownCommand(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample_device() -> DeviceRecord {
        DeviceRecord {
            path: "/dev/bus/usb/001/002".into(),
            busid: "1-1".into(),
            busnum: 1,
            devnum: 2,
            speed: 2,
            id_vendor: 0x045e,
            id_product: 0x028e,
            bcd_device: 0x0114,
            device_class: 0xff,
            device_subclass: 0xff,
            device_protocol: 0xff,
            configuration_value: 1,
            num_configurations: 1,
            interfaces: vec![InterfaceClass { class: 0xff, subclass: 0x5d, protocol: 0x01 }],
        }
    }

    #[test]
    fn device_record_layout() {
        let mut out = Vec::new();
        sample_device().encode(&mut out);
        assert_eq!(out.len(), DEVICE_RECORD_LEN);
        assert_eq!(&out[..20], b"/dev/bus/usb/001/002");
        assert_eq!(out[20], 0);
        assert_eq!(&out[256..259], b"1-1");
        assert_eq!(&out[288..292], &1u32.to_be_bytes()); // busnum
        assert_eq!(&out[292..296], &2u32.to_be_bytes()); // devnum
        assert_eq!(&out[296..300], &2u32.to_be_bytes()); // speed
        assert_eq!(&out[300..306], &[0x04, 0x5e, 0x02, 0x8e, 0x01, 0x14]);
        assert_eq!(&out[306..312], &[0xff, 0xff, 0xff, 1, 1, 1]);
    }

    #[test]
    fn long_strings_truncated_and_terminated() {
        let mut d = sample_device();
        d.path = "x".repeat(400);
        d.busid = "y".repeat(40);
        let mut out = Vec::new();
        d.encode(&mut out);
        assert_eq!(out.len(), DEVICE_RECORD_LEN);
        assert_eq!(out[255], 0);
        assert_eq!(out[287], 0);
    }

    #[test]
    fn devlist_reply_bytes() {
        let d = sample_device();
        let mut out = Vec::new();
        encode_devlist_reply(&mut out, &[&d]);
        assert_eq!(&out[..12], &[0x01, 0x11, 0x00, 0x05, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(out.len(), 12 + DEVICE_RECORD_LEN + 4);
        assert_eq!(&out[out.len() - 4..], &[0xff, 0x5d, 0x01, 0x00]);
    }

    #[test]
    fn import_reply_bytes() {
        let d = sample_device();
        let mut ok = Vec::new();
        encode_import_reply(&mut ok, ST_OK, Some(&d));
        assert_eq!(&ok[..8], &[0x01, 0x11, 0x00, 0x03, 0, 0, 0, 0]);
        assert_eq!(ok.len(), 8 + DEVICE_RECORD_LEN);
        let mut err = Vec::new();
        encode_import_reply(&mut err, ST_NODEV, Some(&d));
        assert_eq!(err, vec![0x01, 0x11, 0x00, 0x03, 0, 0, 0, 4]);
    }

    #[test]
    fn op_request_parse() {
        assert_eq!(parse_op_request(&[0x01, 0x11, 0x80]).unwrap(), None);
        let (r, n) = parse_op_request(&[0x01, 0x11, 0x80, 0x05, 0, 0, 0, 0]).unwrap().unwrap();
        assert_eq!((r, n), (OpRequest::DevList, 8));

        let mut buf = Vec::new();
        encode_op_request(&mut buf, &OpRequest::Import { busid: "1-1".into() });
        assert_eq!(buf.len(), 40);
        assert_eq!(parse_op_request(&buf[..39]).unwrap(), None);
        let (r, n) = parse_op_request(&buf).unwrap().unwrap();
        assert_eq!(r, OpRequest::Import { busid: "1-1".into() });
        assert_eq!(n, 40);
    }

    #[test]
    fn op_request_errors() {
        assert_eq!(parse_op_request(&[0x01, 0x10, 0x80, 0x05, 0, 0, 0, 0]), Err(ProtoError::BadVersion(0x0110)));
        assert_eq!(parse_op_request(&[0x01, 0x11, 0x80, 0x99, 0, 0, 0, 0]), Err(ProtoError::UnknownOp(0x8099)));
        // busid without NUL fills all 32 bytes
        let mut buf = vec![0x01, 0x11, 0x80, 0x03, 0, 0, 0, 0];
        buf.extend_from_slice(&[b'a'; 32]);
        let (r, _) = parse_op_request(&buf).unwrap().unwrap();
        assert_eq!(r, OpRequest::Import { busid: "a".repeat(32) });
    }

    fn submit(seq: u32, dir: u32, ep: u32, len: u32) -> CmdSubmit {
        CmdSubmit {
            seqnum: seq,
            devid: 0x0001_0002,
            direction: dir,
            ep,
            transfer_flags: 0,
            transfer_buffer_length: len,
            start_frame: 0,
            number_of_packets: NON_ISO_PACKETS,
            interval: 0,
            setup: [0x80, 6, 0, 1, 0, 0, 18, 0],
        }
    }

    #[test]
    fn cmd_submit_spec_vector() {
        // Hand-built from usbip_protocol.rst field order.
        let bytes: Vec<u8> = [
            &[0, 0, 0, 1][..],   // command
            &[0, 0, 0, 7],       // seqnum
            &[0, 1, 0, 2],       // devid
            &[0, 0, 0, 1],       // direction IN
            &[0, 0, 0, 0],       // ep
            &[0, 0, 2, 0],       // transfer_flags (URB_DIR_IN)
            &[0, 0, 0, 18],      // length
            &[0, 0, 0, 0],       // start_frame
            &[0xff, 0xff, 0xff, 0xff], // number_of_packets
            &[0, 0, 0, 0],       // interval
            &[0x80, 6, 0, 1, 0, 0, 18, 0],
        ]
        .concat();
        let (cmd, n) = parse_urb_command(&bytes).unwrap().unwrap();
        assert_eq!(n, 48);
        match cmd {
            UrbCommand::Submit(c, data) => {
                assert_eq!(c.seqnum, 7);
                assert_eq!(c.devid, 0x0001_0002);
                assert!(c.is_in());
                assert!(!c.is_iso());
                assert_eq!(c.transfer_flags, 0x200);
                assert_eq!(c.transfer_buffer_length, 18);
                assert_eq!(c.setup, [0x80, 6, 0, 1, 0, 0, 18, 0]);
                assert!(data.is_empty());
            }
            _ => panic!("expected submit"),
        }
        let mut re = Vec::new();
        if let UrbCommand::Submit(c, d) = parse_urb_command(&bytes).unwrap().unwrap().0 {
            encode_cmd_submit(&mut re, &c, d);
        }
        assert_eq!(re, bytes);
    }

    #[test]
    fn cmd_submit_out_payload_incremental() {
        let mut buf = Vec::new();
        encode_cmd_submit(&mut buf, &submit(3, DIR_OUT, 1, 3), &[9, 8, 7]);
        for cut in 0..buf.len() {
            assert_eq!(parse_urb_command(&buf[..cut]).unwrap(), None, "cut {cut}");
        }
        let (cmd, n) = parse_urb_command(&buf).unwrap().unwrap();
        assert_eq!(n, 51);
        assert!(matches!(cmd, UrbCommand::Submit(_, d) if d == [9, 8, 7]));
    }

    #[test]
    fn cmd_submit_iso_consumes_descriptors() {
        let mut c = submit(3, DIR_IN, 2, 0);
        c.number_of_packets = 2;
        let mut buf = Vec::new();
        encode_cmd_submit(&mut buf, &c, &[]);
        buf.extend_from_slice(&[0u8; 32]);
        assert_eq!(parse_urb_command(&buf[..buf.len() - 1]).unwrap(), None);
        let (cmd, n) = parse_urb_command(&buf).unwrap().unwrap();
        assert_eq!(n, 80);
        assert!(matches!(cmd, UrbCommand::Submit(c, _) if c.is_iso()));
    }

    #[test]
    fn cmd_submit_zero_packets_is_non_iso() {
        let mut c = submit(3, DIR_IN, 1, 32);
        c.number_of_packets = 0;
        let mut buf = Vec::new();
        encode_cmd_submit(&mut buf, &c, &[]);
        let (cmd, _) = parse_urb_command(&buf).unwrap().unwrap();
        assert!(matches!(cmd, UrbCommand::Submit(c, _) if !c.is_iso()));
    }

    #[test]
    fn cmd_submit_rejects_bad_fields() {
        let enc = |c: CmdSubmit| {
            let mut b = Vec::new();
            encode_cmd_submit(&mut b, &c, &[]);
            b
        };
        assert_eq!(parse_urb_command(&enc(submit(1, 2, 0, 0))), Err(ProtoError::BadDirection(2)));
        assert_eq!(parse_urb_command(&enc(submit(1, 1, 16, 0))), Err(ProtoError::BadEndpoint(16)));
        assert_eq!(parse_urb_command(&enc(submit(1, 1, 1, 0x8000_0000))), Err(ProtoError::BadLength(i32::MIN)));
        assert_eq!(
            parse_urb_command(&enc(submit(1, 1, 1, MAX_TRANSFER_LEN as u32 + 1))),
            Err(ProtoError::BadLength(MAX_TRANSFER_LEN as i32 + 1))
        );
        let mut c = submit(1, 1, 1, 8);
        c.number_of_packets = -2;
        assert_eq!(parse_urb_command(&enc(c)), Err(ProtoError::BadPacketCount(-2)));
        c.number_of_packets = 1025;
        assert_eq!(parse_urb_command(&enc(c)), Err(ProtoError::BadPacketCount(1025)));
        let mut b = enc(submit(1, 1, 1, 8));
        b[3] = 9;
        assert_eq!(parse_urb_command(&b), Err(ProtoError::UnknownCommand(9)));
    }

    #[test]
    fn unlink_roundtrip() {
        let u = CmdUnlink { seqnum: 10, devid: 0x10002, direction: 0, ep: 0, unlink_seqnum: 9 };
        let mut buf = Vec::new();
        encode_cmd_unlink(&mut buf, &u);
        assert_eq!(buf.len(), 48);
        let (cmd, n) = parse_urb_command(&buf).unwrap().unwrap();
        assert_eq!(n, 48);
        assert_eq!(cmd, UrbCommand::Unlink(u));
    }

    #[test]
    fn ret_submit_bytes() {
        let mut out = Vec::new();
        encode_ret_submit(&mut out, 7, 0, 3, &[1, 2, 3]);
        assert_eq!(out.len(), 51);
        assert_eq!(&out[0..8], &[0, 0, 0, 3, 0, 0, 0, 7]);
        assert_eq!(&out[8..20], &[0u8; 12]);
        assert_eq!(&out[20..28], &[0, 0, 0, 0, 0, 0, 0, 3]);
        assert_eq!(&out[32..36], &[0xff; 4]);
        assert_eq!(&out[48..], &[1, 2, 3]);
        let (r, n) = parse_urb_reply(&out, |_| true).unwrap().unwrap();
        assert_eq!(n, 51);
        assert_eq!(r, UrbReply::Submit { seqnum: 7, status: 0, actual_length: 3, number_of_packets: -1, data: vec![1, 2, 3] });
    }

    #[test]
    fn ret_unlink_bytes() {
        let mut out = Vec::new();
        encode_ret_unlink(&mut out, 11, -104);
        assert_eq!(out.len(), 48);
        assert_eq!(&out[0..8], &[0, 0, 0, 4, 0, 0, 0, 11]);
        assert_eq!(&out[20..24], &(-104i32).to_be_bytes());
    }

    /// Deterministic xorshift so the "fuzz" run is reproducible without extra crates.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn random_input_never_panics() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let mut buf = Vec::with_capacity(256);
        for _ in 0..200_000 {
            buf.clear();
            let len = (rng.next() % 200) as usize;
            for _ in 0..len {
                buf.push(rng.next() as u8);
            }
            // Bias some inputs towards valid prefixes so deeper paths are exercised.
            match rng.next() % 4 {
                0 if buf.len() >= 4 => buf[..4].copy_from_slice(&[0x01, 0x11, 0x80, 0x03]),
                1 if buf.len() >= 4 => buf[..4].copy_from_slice(&[0, 0, 0, 1]),
                2 if buf.len() >= 4 => buf[..4].copy_from_slice(&[0, 0, 0, 2]),
                _ => {}
            }
            let _ = parse_op_request(&buf);
            if let Ok(Some((_, n))) = parse_urb_command(&buf) {
                assert!(n <= buf.len());
            }
            let _ = parse_urb_reply(&buf, |s| s & 1 == 1);
        }
    }
}
