// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.jtmelhorn.usbipbridge

import android.app.Activity
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import io.github.jtmelhorn.usbipbridge.BridgeState.Phase
import org.json.JSONObject

/** Single screen: pick device, grant permission, start/stop, live status. D-pad only. */
class MainActivity : Activity() {
    private lateinit var prefs: Prefs
    private lateinit var statusView: TextView
    private lateinit var hintView: TextView
    private lateinit var statsView: TextView
    private lateinit var deviceList: LinearLayout
    private lateinit var startStop: Button
    private lateinit var grant: Button
    private val main = Handler(Looper.getMainLooper())

    private val tick = object : Runnable {
        override fun run() {
            render()
            main.postDelayed(this, 1000)
        }
    }
    private val stateListener: () -> Unit = { render() }

    private val usbReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == ACTION_USB_PERMISSION) {
                val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
                if (!granted) statusView.text = "USB permission was denied."
            }
            refreshDevices()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = Prefs(this)
        val col = Ui.column(this)
        col.addView(Ui.text(this, getString(R.string.app_name), 28f))
        statusView = Ui.text(this, "", 22f, color = getColor(R.color.accent))
        col.addView(statusView)
        hintView = Ui.text(this, "", 16f, mono = true, color = getColor(R.color.muted))
        col.addView(hintView)

        val actions = Ui.row(this)
        startStop = Ui.button(this, getString(R.string.start)) { toggleSharing() }
        grant = Ui.button(this, getString(R.string.grant)) { requestPermission() }
        actions.addView(startStop)
        actions.addView(grant)
        actions.addView(Ui.button(this, getString(R.string.probe)) { startActivity(Intent(this, ProbeActivity::class.java)) })
        actions.addView(Ui.button(this, getString(R.string.settings)) { startActivity(Intent(this, SettingsActivity::class.java)) })
        col.addView(actions)

        val header = Ui.row(this)
        header.addView(Ui.text(this, getString(R.string.devices), 20f))
        col.addView(header)
        deviceList = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        col.addView(deviceList)
        col.addView(Ui.button(this, getString(R.string.refresh)) { refreshDevices() })

        statsView = Ui.text(this, "", 15f, mono = true, color = getColor(R.color.muted))
        col.addView(statsView)
        col.addView(Ui.text(this, "core: ${runCatching { NativeBridge.nativeVersion() }.getOrDefault("unavailable")}", 12f, color = getColor(R.color.muted)))

        setContentView(ScrollView(this).apply { addView(col) })
        startStop.requestFocus()
    }

    override fun onResume() {
        super.onResume()
        registerReceiverCompat(
            usbReceiver,
            IntentFilter().apply {
                addAction(ACTION_USB_PERMISSION)
                addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED)
                addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
            },
            exported = true,
        )
        BridgeState.addListener(stateListener)
        refreshDevices()
        main.post(tick)
    }

    override fun onPause() {
        main.removeCallbacks(tick)
        BridgeState.removeListener(stateListener)
        unregisterReceiver(usbReceiver)
        super.onPause()
    }

    private fun refreshDevices() {
        // Rebuilding the list drops focus; remember which row had it so D-pad users stay in place.
        val focusedIndex = (0 until deviceList.childCount).indexOfFirst { deviceList.getChildAt(it).hasFocus() }
        deviceList.removeAllViews()
        val devices = UsbDevices.list(this)
        if (devices.isEmpty()) deviceList.addView(Ui.text(this, getString(R.string.no_devices), 16f))
        // With exactly one device present, select it unless the selection already matches. The
        // 8BitDo adapter changes VID:PID when it switches mode, which would otherwise leave a stale
        // selection and a disabled "Grant permission" button (TICKET-023).
        if (devices.size == 1 && !prefs.matches(devices[0])) prefs.select(devices[0])
        val usb = UsbDevices.manager(this)
        for (d in devices) {
            val mark = if (prefs.matches(d)) "✓ " else "   "
            val perm = if (usb.hasPermission(d)) "permission ✓" else "no permission"
            deviceList.addView(Ui.button(this, "$mark${UsbDevices.label(d)}  ($perm)") {
                prefs.select(d)
                refreshDevices()
            }.apply {
                layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT)
                    .apply { bottomMargin = Ui.dp(this@MainActivity, 8) }
            })
        }
        if (focusedIndex in 0 until deviceList.childCount) deviceList.getChildAt(focusedIndex).requestFocus()
        render()
    }

    private fun requestPermission() {
        val d = UsbDevices.findSelected(this)
        when {
            d == null -> statusView.text = "Select a device first."
            UsbDevices.manager(this).hasPermission(d) -> statusView.text = "Permission already granted for ${UsbDevices.vidPid(d)}."
            else -> UsbDevices.requestPermission(this, d)
        }
    }

    private fun toggleSharing() {
        if (BridgeState.phase == Phase.STOPPED || BridgeState.phase == Phase.ERROR && !prefs.sharingEnabled) {
            val d = UsbDevices.findSelected(this)
            if (d == null) {
                statusView.text = "Select the adapter first."
                return
            }
            if (!UsbDevices.manager(this).hasPermission(d)) {
                UsbDevices.requestPermission(this, d)
                statusView.text = "Grant USB permission, then press Start again."
                return
            }
            BridgeService.start(this)
        } else {
            BridgeService.stop(this)
        }
    }

    private fun render() {
        val phase = BridgeState.phase
        statusView.text = BridgeState.message
        val sharing = phase != Phase.STOPPED && !(phase == Phase.ERROR && !prefs.sharingEnabled)
        startStop.text = getString(if (sharing) R.string.stop else R.string.start)
        val sel = UsbDevices.findSelected(this)
        grant.isEnabled = sel != null && !UsbDevices.manager(this).hasPermission(sel)
        val ip = Net.localIpv4().firstOrNull() ?: "<shield-ip>"
        hintView.text = "On the PC:  usbip attach -r $ip -b 1-1" +
            if (prefs.port != 3240) "   (custom port ${prefs.port}: see docs)" else ""
        statsView.text = if (phase == Phase.STOPPED) "" else formatStats(BridgeState.status)
    }

    private fun formatStats(j: JSONObject?): String {
        if (j == null) return ""
        val ri = j.optJSONObject("reportIntervalUs")
        val ta = j.optJSONObject("turnaroundUs")
        fun ms(o: JSONObject?, k: String) = "%.2f".format((o?.optLong(k) ?: 0) / 1000.0)
        return buildString {
            appendLine("sessions ${j.optLong("sessions")}   rejected peers ${j.optLong("rejectedPeers")}")
            appendLine("transfers submitted ${j.optLong("submits")}  completed ${j.optLong("completions")}  unlinked ${j.optLong("unlinks")}")
            appendLine("errors ${j.optLong("errors")} (last status ${j.optInt("lastErrorStatus")})")
            appendLine("interrupt-IN reports ${j.optLong("interruptInReports")}")
            appendLine(
                "report interval (NOT latency) ms: min ${ms(ri, "min")}  p50 ${ms(ri, "p50")}  p99 ${ms(ri, "p99")}  max ${ms(ri, "max")}  (n=${ri?.optLong("n") ?: 0})"
            )
            appendLine(
                "server turnaround ms (completion → socket): p50 ${ms(ta, "p50")}  p99 ${ms(ta, "p99")}  max ${ms(ta, "max")}"
            )
            append("Network RTT and button-to-screen latency are not measured here (see docs/latency-method.md).")
        }
    }
}
