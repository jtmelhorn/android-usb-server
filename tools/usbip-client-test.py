#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Minimal USB/IP 1.1.1 client for testing the Shield server from any PC, without drivers.

It lists and imports the exported device, performs the control transfers a host does during
enumeration (device/config/string descriptors, SET_CONFIGURATION, the XInput init request), sends
the player-1 LED command, queues interrupt-IN transfers for --seconds and reports what arrives,
then exercises CMD_UNLINK on a pending transfer.

Usage: python tools/usbip-client-test.py <shield-ip> [--busid 1-1] [--seconds 10]
"""
import argparse
import socket
import struct
import sys
import time

VERSION = 0x0111
CMD_SUBMIT, CMD_UNLINK, RET_SUBMIT, RET_UNLINK = 1, 2, 3, 4
DIR_OUT, DIR_IN = 0, 1


def recv_exact(s, n):
    buf = b""
    while len(buf) < n:
        chunk = s.recv(n - len(buf))
        if not chunk:
            raise ConnectionError(f"server closed connection ({len(buf)}/{n} bytes)")
        buf += chunk
    return buf


def parse_device(rec):
    path, busid = rec[:256].split(b"\0")[0].decode(), rec[256:288].split(b"\0")[0].decode()
    busnum, devnum, speed, vid, pid, bcd = struct.unpack(">IIIHHH", rec[288:306])
    cls, sub, proto, cfg, ncfg, nif = struct.unpack(">BBBBBB", rec[306:312])
    return dict(path=path, busid=busid, busnum=busnum, devnum=devnum, speed=speed, vid=vid, pid=pid,
                bcd=bcd, cls=cls, sub=sub, proto=proto, cfg=cfg, ncfg=ncfg, nif=nif)


def devlist(host, port):
    with socket.create_connection((host, port), timeout=5) as s:
        s.sendall(struct.pack(">HHI", VERSION, 0x8005, 0))
        ver, code, status, ndev = struct.unpack(">HHII", recv_exact(s, 12))
        devs = []
        for _ in range(ndev):
            d = parse_device(recv_exact(s, 312))
            d["interfaces"] = [recv_exact(s, 4)[:3] for _ in range(d["nif"])]
            devs.append(d)
        return ver, code, status, devs


class Client:
    def __init__(self, host, port, busid):
        self.s = socket.create_connection((host, port), timeout=5)
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.s.sendall(struct.pack(">HHI", VERSION, 0x8003, 0) + busid.encode().ljust(32, b"\0"))
        ver, code, status = struct.unpack(">HHI", recv_exact(self.s, 8))
        if status != 0:
            raise RuntimeError(f"import refused: status {status}")
        self.dev = parse_device(recv_exact(self.s, 312))
        self.devid = (self.dev["busnum"] << 16) | self.dev["devnum"]
        self.seq = 0
        self.in_seqs = set()

    def next_seq(self, direction):
        # usbip-win2 style: low bit carries the direction
        self.seq += 2
        return self.seq | (1 if direction == DIR_IN else 0)

    def submit(self, direction, ep, length, setup=b"\0" * 8, data=b""):
        seq = self.next_seq(direction)
        if direction == DIR_IN:
            self.in_seqs.add(seq)
        hdr = struct.pack(">IIIII", CMD_SUBMIT, seq, self.devid, direction, ep)
        hdr += struct.pack(">IiiiI", 0, length, 0, -1, 0) + setup
        self.s.sendall(hdr + data)
        return seq

    def unlink(self, target):
        seq = self.next_seq(DIR_OUT)
        self.s.sendall(struct.pack(">IIIII", CMD_UNLINK, seq, self.devid, 0, 0) + struct.pack(">I", target) + b"\0" * 24)
        return seq

    def reply(self, timeout=None):
        self.s.settimeout(timeout)
        hdr = recv_exact(self.s, 48)
        cmd, seq = struct.unpack(">II", hdr[:8])
        status, actual, _sf, npk, _ec = struct.unpack(">iiiii", hdr[20:40])
        data = b""
        if cmd == RET_SUBMIT and seq in self.in_seqs and actual > 0:
            data = recv_exact(self.s, actual)
        return dict(cmd=cmd, seq=seq, status=status, actual=actual, npk=npk, data=data)

    def control(self, bm, req, value, index, length, data=b""):
        setup = struct.pack("<BBHHH", bm, req, value, index, length if bm & 0x80 else len(data))
        direction = DIR_IN if bm & 0x80 else DIR_OUT
        seq = self.submit(direction, 0, length if direction == DIR_IN else len(data), setup, data)
        while True:
            r = self.reply(timeout=3)
            if r["seq"] == seq:
                return r


def hexs(b):
    return " ".join(f"{x:02x}" for x in b)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("host")
    ap.add_argument("--port", type=int, default=3240)
    ap.add_argument("--busid", default="1-1")
    ap.add_argument("--seconds", type=float, default=10)
    a = ap.parse_args()

    t0 = time.perf_counter()
    ver, code, status, devs = devlist(a.host, a.port)
    print(f"DEVLIST reply version={ver:#06x} code={code:#06x} status={status} devices={len(devs)} "
          f"({(time.perf_counter() - t0) * 1000:.1f} ms)")
    for d in devs:
        print(f"  busid {d['busid']} {d['vid']:04x}:{d['pid']:04x} speed={d['speed']} cfg={d['cfg']} "
              f"interfaces={[hexs(i) for i in d['interfaces']]}")

    c = Client(a.host, a.port, a.busid)
    print(f"IMPORT ok: {c.dev['vid']:04x}:{c.dev['pid']:04x} path={c.dev['path']} devid={c.devid:#x}")

    r = c.control(0x80, 6, 0x0100, 0, 18)
    print(f"GET_DESCRIPTOR(device) status={r['status']} len={r['actual']} npk={r['npk']}: {hexs(r['data'])}")
    r = c.control(0x80, 6, 0x0200, 0, 9)
    total = struct.unpack("<H", r["data"][2:4])[0] if r["actual"] >= 4 else 9
    r = c.control(0x80, 6, 0x0200, 0, total)
    print(f"GET_DESCRIPTOR(config, {total}) status={r['status']} len={r['actual']}: {hexs(r['data'])}")
    for idx in (1, 2, 3):
        r = c.control(0x80, 6, 0x0300 | idx, 0x0409, 255)
        text = r["data"][2:].decode("utf-16-le", "replace") if r["status"] == 0 else ""
        print(f"GET_DESCRIPTOR(string {idx}) status={r['status']} '{text}'")
    r = c.control(0x80, 6, 0x03EE, 0, 18)
    print(f"GET_DESCRIPTOR(string 0xEE, MS OS) status={r['status']} len={r['actual']}: {hexs(r['data'])}")
    r = c.control(0x00, 9, 1, 0, 0)
    print(f"SET_CONFIGURATION(1) status={r['status']}")
    r = c.control(0xC1, 0x01, 0x0100, 0, 20)
    print(f"XInput init (0xC1/0x01/0x0100) status={r['status']} len={r['actual']}: {hexs(r['data'])}")
    seq = c.submit(DIR_OUT, 2, 3, data=bytes([0x01, 0x03, 0x06]))
    r = c.reply(timeout=3)
    print(f"LED player-1 on ep 0x02 status={r['status']} actual={r['actual']}")

    # Keep 4 interrupt-IN transfers queued on ep 0x81, as a host driver does.
    print(f"Streaming interrupt-IN on ep 0x81 for {a.seconds:.0f} s...")
    pending = {c.submit(DIR_IN, 1, 32) for _ in range(4)}
    reports, changed, last, times = 0, 0, None, []
    end = time.perf_counter() + a.seconds
    while time.perf_counter() < end:
        try:
            r = c.reply(timeout=max(0.05, end - time.perf_counter()))
        except (socket.timeout, TimeoutError):
            break
        pending.discard(r["seq"])
        if r["cmd"] == RET_SUBMIT and r["status"] == 0 and r["data"]:
            reports += 1
            times.append(time.perf_counter())
            if r["data"] != last:
                changed += 1
                if changed <= 5:
                    print(f"  report {reports}: {hexs(r['data'])}")
            last = r["data"]
        elif r["status"] != 0:
            print(f"  completion status {r['status']}")
        pending.add(c.submit(DIR_IN, 1, 32))
    gaps = sorted((b - a2) * 1000 for a2, b in zip(times, times[1:]))
    print(f"interrupt-IN reports: {reports}, distinct payloads: {changed}"
          + (f", inter-report gap ms (client side, NOT latency) min {gaps[0]:.2f} p50 {gaps[len(gaps)//2]:.2f}" if gaps else ""))

    # Unlink one still-pending transfer; expect RET_UNLINK -ECONNRESET (or 0 if it raced to completion).
    target = next(iter(pending))
    useq = c.unlink(target)
    t = time.perf_counter()
    while True:
        r = c.reply(timeout=3)
        if r["seq"] == useq:
            print(f"UNLINK of seq {target}: RET_UNLINK status={r['status']} after {(time.perf_counter() - t) * 1000:.1f} ms")
            break
        if r["seq"] == target:
            print(f"(seq {target} completed before the unlink: status {r['status']})")
    c.s.close()
    print("closed connection (server must discard the remaining pending transfers)")


if __name__ == "__main__":
    sys.exit(main())
