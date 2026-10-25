# TICKET-007: usbdevfs URB engine (submit / reap / discard)

- **Status:** in_progress
- **Milestone:** 4
- **Priority:** P0
- **Dependencies:** 002

## Problem
Android's Java `UsbRequest` API is usable, but cancellation and completion ordering are indirect.
The input path should submit transfers asynchronously, complete them as soon as the kernel does, and
cancel precisely when the client unlinks.

## Intended outcome
`core/src/usbfs.rs`: a thin, allocation-free-per-transfer wrapper over the usbdevfs ioctls on the fd
obtained from `UsbDeviceConnection.getFileDescriptor()`.

## Scope
- `USBDEVFS_SUBMITURB` (control, bulk, interrupt), `USBDEVFS_REAPURBNDELAY`, `USBDEVFS_DISCARDURB`,
  `USBDEVFS_SETINTERFACE`, `USBDEVFS_CLEAR_HALT`, `USBDEVFS_GET_SPEED` (fallback: speed from
  `bcdUSB`/maxpacket heuristics if unsupported), reading descriptors from the fd.
- `#[repr(C)]` `usbdevfs_urb` correct for both 32-bit (armeabi-v7a) and 64-bit userspace; ioctl numbers
  derived from `size_of` at compile time.
- `poll()` on the fd: `POLLOUT` = completions available, `POLLERR|POLLHUP` = device gone.
- URB slots: fixed slab with reusable buffers (grown only when a larger transfer is requested).

## Exclusions
- Isochronous URBs.

## Acceptance criteria
1. Compiles for `aarch64-linux-android` and `armv7-linux-androideabi`; struct size asserts pass
   (56 bytes on 64-bit, 44 bytes on 32-bit, matching `<linux/usbdevice_fs.h>`; SUBMITURB = 0x8038550a / 0x802c550a).
2. Host unit tests for ioctl number encoding against known kernel values.
3. **[HW]** On the Shield: interrupt-IN reports flow to Windows (proved by TICKET-010).
4. **[HW]** Discard of a pending interrupt URB completes with `-ENOENT`/`-ECONNRESET` (verbose log).

## Validation plan
- `cargo test`; `cargo build --target …`; HW via TICKET-010/012 logs.

## Required evidence
- Test output; HW log excerpts showing submit/complete/discard.

## Implementation notes / affected files
- `core/src/usbfs.rs`, `core/src/usbfs_sys.rs` (raw ioctl definitions).
- Reaping uses the non-blocking variant only; no thread blocks inside the kernel, so stop/disconnect
  never hangs.

## Remaining uncertainties / blockers
- HW criteria 3–4 blocked by TICKET-004.

## Evidence so far
- Criteria 1–2: `usbfs_sys::tests::layout_and_ioctl_numbers_match_kernel` passes on the host (64-bit)
  **and on the Shield as both aarch64 and armv7 binaries** (`evidence/core-tests-on-shield.txt`), so
  the 32-bit layout is checked on real hardware.
- Criteria 3–4 (real URBs on the adapter) blocked by TICKET-004.
