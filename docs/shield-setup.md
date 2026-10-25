# Shield setup

Verified on: SHIELD Android TV (2019 Pro, `mdarcy`), Android 11 (API 30), kernel 4.9.141-tegra,
build `7825230_4387.0822` (see `evidence/004-device-props.txt`).

## Install the app

1. On the Shield, go to Settings → Device Preferences → About and select **Build** 7 times to enable
   developer options.
2. Turn on Settings → Device Preferences → Developer options → **Network debugging**.
3. On the PC (Android platform-tools):

   ```bash
   adb connect <shield-ip>:5555
   ```

   Accept the "Allow network debugging?" prompt on the TV, ticking "Always allow from this computer".

4. Install the APK:

   ```bash
   adb install -r usbip-bridge-0.1.0-debug.apk
   ```

The app appears on the home screen's app row as **USB/IP Gamepad Bridge**. Network debugging
can be switched off again after installing.

## First run

1. Plug the 8BitDo USB Wireless Adapter 2 into a Shield USB port and connect the controller.
2. Android may ask *"Open USB/IP Gamepad Bridge to handle …?"*. Tick **Always** (or "Use by
   default") and choose **OK**. This lets Android grant USB permission again automatically when the
   adapter re-enumerates, so sharing can resume without the remote.
3. In the app, select the adapter (✓) and choose **Grant permission** if it shows "no permission".
4. Optional: run **Probe device**, then press buttons for 10 s. This confirms Android lets the app
   claim the adapter (see TICKET-004).
5. Choose **Start sharing**. A notification stays while sharing; its **Stop sharing** action and
   the app's button both stop it.

While sharing, the controller is **claimed by this app**. Android's own gamepad driver is detached,
so the Shield UI and Moonlight do not see the controller. Navigate the Shield with its remote. When
you stop sharing, Android gets the controller back.

## Notes

- The adapter's mode (XInput, DS4, Switch, …) changes its USB vendor/product ID. The app remembers
  the selected ID, so switching modes requires selecting the device again.
- Settings → *Only allow this PC* restricts connections to one IP address (recommended).
