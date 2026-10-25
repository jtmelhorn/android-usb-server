# Windows setup (usbip-win2 client)

The PC side uses the unmodified, **signed** release of
[usbip-win2](https://github.com/vadimgrn/usbip-win2). No test-signing mode, no Secure Boot
changes, no drivers from this project.

> Status: these steps follow usbip-win2's own documentation and source. They have **not yet been
> run against this server on the target PC** (TICKET-010). Items marked *unverified* will be
> updated with real output.

## 1. Install usbip-win2

1. Create a system restore point (usbip-win2's README recommends it): Start → "Create a restore
   point" → **Create**.
2. Download the latest installer (`USBip-x.y.z.w-x64.exe`) from
   https://github.com/vadimgrn/usbip-win2/releases. Check the release is marked as signed
   (WHLK / attestation-signed drivers). **Do not** run `bcdedit /set testsigning on`; if an installer
   asks for test signing, stop.
3. Run the installer as administrator. All USB 3.0 hubs restart briefly during installation, so
   USB keyboards and mice may drop for a moment.
4. Open a new terminal. `usbip.exe` is installed under `C:\Program Files\USBip\` (*unverified path*;
   the installer adds it to `PATH`).

Check signature enforcement is unchanged:

```bash
bcdedit /enum {current}
```

There must be no `testsigning Yes` line.

## 2. Allow the connection

The PC makes an **outgoing** TCP connection to the Shield on port 3240, so normally no Windows
Firewall change is needed. Nothing on the PC listens for this project.

## 3. Attach the controller

On the Shield: open **USB/IP Gamepad Bridge**, select the adapter, then press **Start sharing**.
The screen shows the Shield's IP, for example `192.168.15.116`.

On the PC:

```bash
usbip list -r 192.168.15.116
```

```bash
usbip attach -r 192.168.15.116 -b 1-1
```

The bus id is always `1-1`, even after the adapter re-enumerates. Check the attachment:

```bash
usbip port
```

The controller then appears in Device Manager (Xbox 360-class controller when the adapter is in
XInput mode) and in `joy.cpl`.

Detach:

```bash
usbip detach -a
```

## 4. Automatic re-attach

If the connection drops (adapter re-plugged, Shield Wi-Fi blip, sharing restarted), usbip-win2
retries the attach in the background by itself. Its defaults, from the driver's INF:

| Value | Default | Meaning |
|---|---|---|
| `ReattachFirstDelay` | 30 s | wait before the first retry |
| `ReattachMaxDelay` | 480 s | back-off ceiling |
| `ReattachMaxAttempts` | 20 | give up after this many tries |

A 30 s first delay is long while gaming. These are ordinary driver parameters, not a security
setting. To retry after 2 s, run in an **administrator** terminal and then reboot (or reinstall)
so the driver rereads them. Registry path assumed from the INF `HKR, Parameters` entry
(*unverified*):

```bash
reg add "HKLM\SYSTEM\CurrentControlSet\Services\usbip2_ude\Parameters" /v ReattachFirstDelay /t REG_DWORD /d 2 /f
```

To stop background attempts:

```bash
usbip attach -r 192.168.15.116 -b 1-1 -x
```

## 5. Uninstall

Settings → Apps → **USBip** → Uninstall.

## Security notes

- USB/IP has no authentication or encryption. The Shield app only accepts clients from
  local-network addresses, and in Settings you can restrict it to this PC's IP.
- **Never** forward TCP port 3240 on your router.
