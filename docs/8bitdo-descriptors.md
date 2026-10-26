# 8BitDo USB Wireless Adapter 2 — descriptors as seen by the Shield

Captured 2026-10-26 on SHIELD Android TV (Android 11) with a DualShock 4 paired to the adapter.
Read through this project's USB/IP server by `tools/usbip-client-test.py` (raw bytes in
`evidence/005-client-test.txt`) and cross-checked against Android's `UsbDevice` view
(`evidence/004-probe.log`).

## Modes observed

| Time | VID:PID | Meaning |
|---|---|---|
| 11:11:55 | `2dc8:3107` | **"8BitDo IDLE"**: single HID interface, shown while no controller is connected |
| 11:13:58 → | `045e:028e` | XInput (Xbox 360 wired controller) |

The adapter re-enumerated on its own several times (device number 015 → 017 within minutes),
consistent with it auto-detecting the host type. See TICKET-024.

## Device descriptor (XInput mode)

| Field | Value |
|---|---|
| bcdUSB | 2.00 |
| Class / Subclass / Protocol | ff / ff / ff (vendor) |
| bMaxPacketSize0 | 64 |
| idVendor:idProduct | 045e:028e |
| bcdDevice | 1.14 |
| Strings | manufacturer "8BitDo", product "Controller", serial present |
| Configurations | 1 |
| Speed | full (kernel `USBDEVFS_GET_SPEED` unsupported on Shield 4.9 → server falls back to full speed, which is correct for this device) |

## Configuration 1 (139 bytes, 4 interfaces, bus powered, remote wake-up, 500 mA)

| Interface | Class | Endpoints | Role (Xbox 360 convention) |
|---|---|---|---|
| 0 | ff/5d/01 | `0x81` interrupt IN 32 B, bInterval 4 · `0x02` interrupt OUT 32 B, bInterval 8 | gamepad input / rumble + LED |
| 1 | ff/5d/03 | `0x83` interrupt IN 32 B, bInterval 2 · `0x04` interrupt OUT 32 B, bInterval 4 | headset / audio port |
| 2 | ff/5d/02 | `0x86` interrupt IN 32 B, bInterval 16 | expansion port |
| 3 | ff/fd/13 | none | security (console authentication) |

Each gamepad interface also carries a vendor `0x21` descriptor, and interface 3 a `0x41` descriptor,
as on genuine Xbox 360 pads.

**bInterval 4 on `0x81`** at full speed means the host may poll at most every 4 ms. That limits the
*report rate*. It is not a latency figure (see `docs/latency-method.md`).

## Request classification (from the client test; Windows trace pending, TICKET-005)

| Request | Handling | Result on hardware |
|---|---|---|
| GET_DESCRIPTOR device / config / string 1–3 | forwarded | OK |
| GET_DESCRIPTOR string 0xEE (Microsoft OS) | forwarded | device stalls (-EPIPE), forwarded as-is |
| SET_CONFIGURATION(1) | intercepted (acknowledged) | OK |
| Vendor IN `0xC1 / 0x01 / wValue 0x0100` (XInput init) | forwarded | OK, returns 2 bytes `00 00` |
| Interrupt OUT `01 03 06` on `0x02` (LED player 1) | forwarded | OK, 3 bytes written |
| Interrupt IN on `0x81` | forwarded | queued; **no reports observed on the Shield yet** (TICKET-024) |
| CMD_UNLINK of a pending interrupt IN | `USBDEVFS_DISCARDURB` | RET_UNLINK `-ECONNRESET` in 1.6 ms |
