# Testing

## Host unit tests (any OS)

```bash
cd core && cargo test
```

These cover the protocol codec (byte-exact vectors, malformed input, 200k randomized inputs), the
session state machine (control/interrupt transfers, intercepts, UNLINK races, duplicate seqnum,
pending limit, steady-state buffer reuse), the connection engine (ACL, busy/eviction, client
disconnect with pending URBs, device loss, 100× reconnect, slow client, shutdown, turnaround
metric), descriptor parsing, `usbdevfs` struct layout and ioctl numbers, and statistics.

## On-device tests (Shield, via adb)

The same tests plus the Linux-only socket driver test (`server::tests`: device list, import, 500
interrupt reports over real TCP, unlink, client vanishing with a pending transfer, reconnect, stop
while attached, port-in-use) run on the Shield as plain executables:

```bash
cargo test --no-run --target aarch64-linux-android
```

```bash
adb push target/aarch64-linux-android/debug/deps/usbipcore-<hash> /data/local/tmp/usbipcore-tests
```

```bash
adb shell /data/local/tmp/usbipcore-tests
```

Set `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER` to the NDK's `aarch64-linux-android26-clang` first.
From Git Bash on Windows, set `MSYS_NO_PATHCONV=1` so `/data/...` is not rewritten. Repeat with
`armv7-linux-androideabi` for the 32-bit layout.

## Hardware procedures

See the **[HW]** tickets: 004 (Shield USB access), 005 (descriptor/URB trace), 010 (Windows
enumeration and input), 011 (Moonlight background), 017 (soak), 018 (latency).
