#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Windows XInput monitor: reads controllers through xinput1_4.dll exactly as games do and reports
which buttons, triggers and stick ranges were seen. Used to validate TICKET-010 (buttons and sticks
in a Windows controller tester) without installing anything.

Usage: python tools/xinput-monitor.py [--seconds 30]
"""
import argparse
import ctypes
import time

BUTTONS = [
    (0x0001, "DPAD_UP"), (0x0002, "DPAD_DOWN"), (0x0004, "DPAD_LEFT"), (0x0008, "DPAD_RIGHT"),
    (0x0010, "START"), (0x0020, "BACK"), (0x0040, "LEFT_THUMB"), (0x0080, "RIGHT_THUMB"),
    (0x0100, "LEFT_SHOULDER"), (0x0200, "RIGHT_SHOULDER"), (0x0400, "GUIDE"),
    (0x1000, "A"), (0x2000, "B"), (0x4000, "X"), (0x8000, "Y"),
]


class GAMEPAD(ctypes.Structure):
    _fields_ = [("wButtons", ctypes.c_ushort), ("bLeftTrigger", ctypes.c_ubyte), ("bRightTrigger", ctypes.c_ubyte),
                ("sThumbLX", ctypes.c_short), ("sThumbLY", ctypes.c_short),
                ("sThumbRX", ctypes.c_short), ("sThumbRY", ctypes.c_short)]


class STATE(ctypes.Structure):
    _fields_ = [("dwPacketNumber", ctypes.c_uint), ("Gamepad", GAMEPAD)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seconds", type=float, default=30)
    a = ap.parse_args()
    x = ctypes.WinDLL("xinput1_4.dll")
    # Ordinal 100 is XInputGetStateEx, which also reports the Guide button.
    try:
        get = x[100]
    except Exception:
        get = x.XInputGetState
    get.argtypes = [ctypes.c_uint, ctypes.POINTER(STATE)]

    st = STATE()
    slots = [i for i in range(4) if get(i, ctypes.byref(st)) == 0]
    print(f"connected XInput slots: {slots}")
    if not slots:
        return 1
    seen, packets = set(), {i: None for i in slots}
    ext = {k: [0, 0] for k in ("LX", "LY", "RX", "RY")}
    trig = [0, 0]
    changes = 0
    end = time.perf_counter() + a.seconds
    while time.perf_counter() < end:
        for i in slots:
            if get(i, ctypes.byref(st)) != 0:
                print(f"slot {i} disconnected")
                slots.remove(i)
                break
            if st.dwPacketNumber == packets[i]:
                continue
            packets[i] = st.dwPacketNumber
            changes += 1
            g = st.Gamepad
            pressed = [n for m, n in BUTTONS if g.wButtons & m]
            seen.update(pressed)
            for k, v in (("LX", g.sThumbLX), ("LY", g.sThumbLY), ("RX", g.sThumbRX), ("RY", g.sThumbRY)):
                ext[k][0] = min(ext[k][0], v)
                ext[k][1] = max(ext[k][1], v)
            trig = [max(trig[0], g.bLeftTrigger), max(trig[1], g.bRightTrigger)]
            if changes <= 40 or changes % 50 == 0:
                print(f"slot {i} pkt {st.dwPacketNumber}: buttons={pressed} LT={g.bLeftTrigger} RT={g.bRightTrigger} "
                      f"L=({g.sThumbLX},{g.sThumbLY}) R=({g.sThumbRX},{g.sThumbRY})")
        time.sleep(0.002)
    print(f"state changes: {changes}")
    print(f"buttons seen ({len(seen)}/15): {sorted(seen)}")
    print(f"buttons never seen: {[n for _, n in BUTTONS if n not in seen]}")
    print(f"triggers max: LT={trig[0]} RT={trig[1]}")
    print("stick ranges: " + ", ".join(f"{k}=[{lo},{hi}]" for k, (lo, hi) in ext.items()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
