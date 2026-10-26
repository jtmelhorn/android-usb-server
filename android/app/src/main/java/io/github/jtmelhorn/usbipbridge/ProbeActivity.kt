// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.jtmelhorn.usbipbridge

import android.app.Activity
import android.os.Bundle
import android.util.Log
import android.widget.ScrollView
import android.widget.TextView
import io.github.jtmelhorn.usbipbridge.BridgeState.Phase

class ProbeActivity : Activity() {
    private lateinit var logView: TextView
    private lateinit var scroll: ScrollView
    @Volatile private var running = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val col = Ui.column(this)
        col.addView(Ui.text(this, "USB access probe", 26f))
        col.addView(
            Ui.text(
                this,
                "Opens the selected device with Android's public USB API, claims every interface " +
                    "(detaching Android's gamepad driver) and records interrupt reports. Results are " +
                    "also written to logcat tag UsbIpProbe.",
                15f, color = getColor(R.color.muted),
            )
        )
        val run = Ui.button(this, getString(R.string.run_probe)) { runProbe() }
        col.addView(run)
        col.addView(Ui.button(this, getString(R.string.reset_adapter)) { resetAdapter() })
        logView = Ui.text(this, "", 13f, mono = true)
        col.addView(logView)
        scroll = ScrollView(this).apply { addView(col) }
        setContentView(scroll)
        run.requestFocus()
    }

    private fun append(s: String) = runOnUiThread {
        logView.append(s + "\n")
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }

    /** Re-enumerate the adapter without unplugging it (it restarts its host/mode detection). */
    private fun resetAdapter() {
        if (BridgeState.phase != Phase.STOPPED && BridgeState.phase != Phase.ERROR) {
            append("Stop sharing first.")
            return
        }
        val d = UsbDevices.findSelected(this) ?: UsbDevices.list(this).singleOrNull()
        if (d == null) {
            append("No device to reset.")
            return
        }
        val usb = UsbDevices.manager(this)
        if (!usb.hasPermission(d)) {
            append("No USB permission for ${UsbDevices.vidPid(d)}; grant it on the main screen.")
            return
        }
        val conn = usb.openDevice(d) ?: run { append("openDevice failed"); return }
        val r = NativeBridge.nativeResetDevice(conn.fileDescriptor)
        conn.close()
        append("USB reset of ${UsbDevices.vidPid(d)} ${d.deviceName} -> ${if (r == 0) "OK" else "error $r"}")
        Log.i(UsbProbe.TAG, "USB reset of ${UsbDevices.vidPid(d)} ${d.deviceName} -> $r")
    }

    private fun runProbe() {
        if (running) return
        if (BridgeState.phase != Phase.STOPPED && BridgeState.phase != Phase.ERROR) {
            append("Stop sharing first: the device is in use by the USB/IP server.")
            return
        }
        val d = UsbDevices.findSelected(this)
        if (d == null) {
            append("No selected device connected. Select it on the main screen.")
            return
        }
        logView.text = ""
        running = true
        Thread({
            try {
                UsbProbe(this, d, ::append).run(30)
            } finally {
                running = false
            }
        }, "usb-probe").start()
    }
}
