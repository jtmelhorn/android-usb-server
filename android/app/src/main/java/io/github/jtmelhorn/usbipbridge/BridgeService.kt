// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.jtmelhorn.usbipbridge

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbManager
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import io.github.jtmelhorn.usbipbridge.BridgeState.Phase
import org.json.JSONObject
import java.io.IOException

/**
 * Foreground service that owns the USB device while sharing.
 *
 * Every USB and native call happens on one worker thread, so the native handle is never used
 * concurrently. The Rust core runs its own server thread; this service only starts/stops it and
 * polls a status snapshot.
 */
class BridgeService : Service() {
    companion object {
        private const val TAG = "UsbIpBridge"
        const val ACTION_START = "io.github.jtmelhorn.usbipbridge.START"
        const val ACTION_STOP = "io.github.jtmelhorn.usbipbridge.STOP"
        private const val CHANNEL = "sharing"
        private const val NOTIFICATION_ID = 1
        private const val STATUS_PERIOD_MS = 500L

        fun start(ctx: Context) {
            ctx.startForegroundService(Intent(ctx, BridgeService::class.java).setAction(ACTION_START))
        }

        fun stop(ctx: Context) {
            Prefs(ctx).sharingEnabled = false
            ctx.startService(Intent(ctx, BridgeService::class.java).setAction(ACTION_STOP))
        }
    }

    private lateinit var worker: HandlerThread
    private lateinit var handler: Handler
    private lateinit var prefs: Prefs

    private var device: UsbDevice? = null
    private var connection: UsbDeviceConnection? = null
    private val claimed = mutableListOf<UsbInterface>()
    private var handle = 0L
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    private var lastNotified = ""

    private val usbReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val d = intent.usbDevice() ?: return
            when (intent.action) {
                UsbManager.ACTION_USB_DEVICE_DETACHED -> handler.post {
                    if (device?.deviceName == d.deviceName) {
                        Log.i(TAG, "selected device detached: ${d.deviceName}")
                        stopNative()
                        publish(Phase.WAITING_FOR_DEVICE, "Adapter disconnected — waiting for it to come back.")
                    }
                }
                UsbManager.ACTION_USB_DEVICE_ATTACHED -> handler.postDelayed({
                    if (prefs.sharingEnabled && handle == 0L && prefs.matches(d)) {
                        Log.i(TAG, "selected device re-attached: ${d.deviceName}")
                        startSharing()
                    }
                }, 300)
            }
        }
    }

    private val statusPoll = object : Runnable {
        override fun run() {
            if (handle == 0L) return
            val json = try {
                JSONObject(NativeBridge.nativeStatus(handle))
            } catch (e: Exception) {
                null
            }
            when (json?.optString("state")) {
                "device_gone" -> {
                    stopNative()
                    publish(Phase.WAITING_FOR_DEVICE, "Adapter disconnected — waiting for it to come back.", json)
                    return
                }
                "attached" -> publish(Phase.ATTACHED, "Attached by ${json.optString("client")}", json)
                "listening" -> publish(Phase.LISTENING, listeningMessage(), json)
                "stopped" -> {
                    stopNative()
                    publish(Phase.ERROR, "Server stopped unexpectedly; see logcat (UsbIpCore).", json)
                    return
                }
            }
            handler.postDelayed(this, STATUS_PERIOD_MS)
        }
    }

    override fun onCreate() {
        super.onCreate()
        prefs = Prefs(this)
        worker = HandlerThread("bridge-worker").apply { start() }
        handler = Handler(worker.looper)
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, getString(R.string.channel_name), NotificationManager.IMPORTANCE_LOW))
        registerReceiverCompat(
            usbReceiver,
            IntentFilter().apply {
                addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED)
                addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
            },
            exported = true,
        )
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                prefs.sharingEnabled = false
                handler.post {
                    stopNative()
                    publish(Phase.STOPPED, "Sharing is off.")
                    stopForeground(STOP_FOREGROUND_REMOVE)
                    stopSelf()
                }
            }
            else -> {
                prefs.sharingEnabled = true
                goForeground("Starting…")
                handler.post { startSharing() }
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        unregisterReceiver(usbReceiver)
        handler.post { stopNative() }
        worker.quitSafely()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    // ---- worker thread ----

    private fun startSharing() {
        if (handle != 0L) return
        val usb = UsbDevices.manager(this)
        val d = UsbDevices.findSelected(this)
        if (d == null) {
            publish(Phase.WAITING_FOR_DEVICE, "Selected adapter is not connected — waiting for it.")
            return
        }
        if (!usb.hasPermission(d)) {
            publish(Phase.ERROR, "No USB permission for ${UsbDevices.vidPid(d)}. Open the app and choose “Grant permission”.")
            return
        }
        publish(Phase.STARTING, "Opening ${UsbDevices.vidPid(d)}…")
        val conn = usb.openDevice(d)
        if (conn == null) {
            publish(Phase.ERROR, "Android refused to open the device (openDevice returned null).")
            return
        }
        // Claim every interface with force=true: this detaches Android's own gamepad driver so the
        // input no longer reaches Android or Moonlight, only the USB/IP client.
        for (i in 0 until d.interfaceCount) {
            val itf = d.getInterface(i)
            if (itf.alternateSetting != 0) continue
            if (conn.claimInterface(itf, true)) {
                claimed += itf
            } else {
                Log.e(TAG, "claimInterface(${itf.id}, force) failed")
                releaseUsb(conn)
                publish(Phase.ERROR, "Could not claim interface ${itf.id} of ${UsbDevices.vidPid(d)}.")
                return
            }
        }
        val raw = conn.rawDescriptors
        if (raw == null) {
            releaseUsb(conn)
            publish(Phase.ERROR, "Could not read USB descriptors.")
            return
        }
        val (bus, dev) = UsbDevices.busAndDev(d)
        try {
            handle = NativeBridge.nativeStart(
                conn.fileDescriptor, raw, bus, dev, d.deviceName, prefs.port,
                prefs.allowedClient.ifBlank { null }, prefs.verbose,
            )
        } catch (e: IOException) {
            releaseUsb(conn)
            publish(Phase.ERROR, e.message ?: "Native start failed.")
            return
        }
        device = d
        connection = conn
        acquireLocks()
        Log.i(TAG, "sharing ${UsbDevices.label(d)} on port ${prefs.port}")
        publish(Phase.LISTENING, listeningMessage())
        handler.postDelayed(statusPoll, STATUS_PERIOD_MS)
    }

    private fun stopNative() {
        handler.removeCallbacks(statusPoll)
        if (handle != 0L) {
            NativeBridge.nativeStop(handle) // joins the server thread; URBs are reaped before return
            handle = 0L
        }
        connection?.let { releaseUsb(it) }
        connection = null
        device = null
        releaseLocks()
    }

    private fun releaseUsb(conn: UsbDeviceConnection) {
        for (itf in claimed) {
            try {
                conn.releaseInterface(itf)
            } catch (_: Exception) {
            }
        }
        claimed.clear()
        conn.close()
    }

    private fun acquireLocks() {
        val pm = getSystemService(PowerManager::class.java)
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "UsbIpBridge:sharing").apply { acquire() }
        val wm = applicationContext.getSystemService(WifiManager::class.java)
        @Suppress("DEPRECATION")
        val mode = if (Build.VERSION.SDK_INT >= 29) WifiManager.WIFI_MODE_FULL_LOW_LATENCY else WifiManager.WIFI_MODE_FULL_HIGH_PERF
        wifiLock = wm.createWifiLock(mode, "UsbIpBridge:sharing").apply { acquire() }
    }

    private fun releaseLocks() {
        wakeLock?.let { if (it.isHeld) it.release() }
        wifiLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
        wifiLock = null
    }

    private fun listeningMessage(): String {
        val ips = Net.localIpv4().ifEmpty { listOf("<shield-ip>") }
        return "Listening on ${ips.joinToString(", ")} port ${prefs.port} — waiting for the PC to attach."
    }

    private fun publish(phase: Phase, message: String, status: JSONObject? = null) {
        if (status != null) BridgeState.updateStatus(status)
        if (phase != BridgeState.phase || message != BridgeState.message) {
            BridgeState.update(phase, message, port = prefs.port)
        }
        if (phase != Phase.STOPPED) goForeground(message)
    }

    private fun goForeground(text: String) {
        if (text == lastNotified) return
        lastNotified = text
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val stop = PendingIntent.getService(
            this, 1, Intent(this, BridgeService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE,
        )
        val n = Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(getString(R.string.app_name))
            .setContentText(text)
            .setOngoing(true)
            .setContentIntent(open)
            .addAction(Notification.Action.Builder(null, getString(R.string.stop), stop).build())
            .build()
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(NOTIFICATION_ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        } else {
            startForeground(NOTIFICATION_ID, n)
        }
    }
}
