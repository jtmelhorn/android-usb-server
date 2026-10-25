// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.jtmelhorn.usbipbridge

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbManager
import android.os.Build
import java.net.Inet4Address
import java.net.NetworkInterface

const val ACTION_USB_PERMISSION = "io.github.jtmelhorn.usbipbridge.USB_PERMISSION"

/** Persistent user choices. Sharing is never enabled by default. */
class Prefs(ctx: Context) {
    private val sp = ctx.getSharedPreferences("bridge", Context.MODE_PRIVATE)

    var selectedVid: Int
        get() = sp.getInt("vid", -1)
        set(v) = sp.edit().putInt("vid", v).apply()
    var selectedPid: Int
        get() = sp.getInt("pid", -1)
        set(v) = sp.edit().putInt("pid", v).apply()
    var port: Int
        get() = sp.getInt("port", 3240)
        set(v) = sp.edit().putInt("port", v).apply()
    var allowedClient: String
        get() = sp.getString("allowedClient", "") ?: ""
        set(v) = sp.edit().putString("allowedClient", v).apply()
    var verbose: Boolean
        get() = sp.getBoolean("verbose", false)
        set(v) = sp.edit().putBoolean("verbose", v).apply()
    /** The user's last explicit start/stop; used to resume after the adapter re-enumerates. */
    var sharingEnabled: Boolean
        get() = sp.getBoolean("sharingEnabled", false)
        set(v) = sp.edit().putBoolean("sharingEnabled", v).apply()

    fun select(d: UsbDevice) {
        selectedVid = d.vendorId
        selectedPid = d.productId
    }

    fun matches(d: UsbDevice) = d.vendorId == selectedVid && d.productId == selectedPid
}

object UsbDevices {
    fun manager(ctx: Context) = ctx.getSystemService(Context.USB_SERVICE) as UsbManager

    fun list(ctx: Context): List<UsbDevice> = manager(ctx).deviceList.values.sortedBy { it.deviceName }

    fun findSelected(ctx: Context): UsbDevice? {
        val p = Prefs(ctx)
        return list(ctx).firstOrNull { p.matches(it) }
    }

    fun vidPid(d: UsbDevice) = "%04x:%04x".format(d.vendorId, d.productId)

    fun label(d: UsbDevice): String {
        val name = listOfNotNull(d.manufacturerName, d.productName).joinToString(" ").ifBlank { "USB device" }
        val ifaces = (0 until d.interfaceCount).joinToString(" ") {
            val i = d.getInterface(it)
            "%02x/%02x/%02x".format(i.interfaceClass, i.interfaceSubclass, i.interfaceProtocol)
        }
        return "$name  [${vidPid(d)}]  interfaces: $ifaces"
    }

    /** `/dev/bus/usb/001/005` → (1, 5). */
    fun busAndDev(d: UsbDevice): Pair<Int, Int> {
        val parts = d.deviceName.trim('/').split('/')
        val bus = parts.getOrNull(parts.size - 2)?.toIntOrNull() ?: 1
        val dev = parts.lastOrNull()?.toIntOrNull() ?: d.deviceId
        return bus to dev
    }

    fun requestPermission(ctx: Context, d: UsbDevice) {
        val flags = if (Build.VERSION.SDK_INT >= 31) PendingIntent.FLAG_MUTABLE else 0
        val intent = Intent(ACTION_USB_PERMISSION).setPackage(ctx.packageName)
        manager(ctx).requestPermission(d, PendingIntent.getBroadcast(ctx, 0, intent, flags))
    }

    fun describeEndpoints(d: UsbDevice): String = buildString {
        for (i in 0 until d.interfaceCount) {
            val itf = d.getInterface(i)
            appendLine(
                "interface ${itf.id} alt ${itf.alternateSetting}: class %02x/%02x/%02x, %d endpoint(s)"
                    .format(itf.interfaceClass, itf.interfaceSubclass, itf.interfaceProtocol, itf.endpointCount)
            )
            for (e in 0 until itf.endpointCount) {
                val ep = itf.getEndpoint(e)
                val type = when (ep.type) {
                    UsbConstants.USB_ENDPOINT_XFER_INT -> "interrupt"
                    UsbConstants.USB_ENDPOINT_XFER_BULK -> "bulk"
                    UsbConstants.USB_ENDPOINT_XFER_ISOC -> "isochronous"
                    else -> "control"
                }
                val dir = if (ep.direction == UsbConstants.USB_DIR_IN) "IN" else "OUT"
                appendLine("  ep 0x%02x %s %s maxPacket %d interval %d".format(ep.address, type, dir, ep.maxPacketSize, ep.interval))
            }
        }
    }
}

object Net {
    /** Site-local / link-local IPv4 addresses of active interfaces, for display. */
    fun localIpv4(): List<String> = try {
        NetworkInterface.getNetworkInterfaces().toList()
            .filter { it.isUp && !it.isLoopback }
            .flatMap { it.inetAddresses.toList() }
            .filterIsInstance<Inet4Address>()
            .filter { it.isSiteLocalAddress || it.isLinkLocalAddress }
            .map { it.hostAddress ?: "" }
            .filter { it.isNotEmpty() }
    } catch (e: Exception) {
        emptyList()
    }
}

/** registerReceiver with the export flag required on API 33+. */
fun Context.registerReceiverCompat(r: BroadcastReceiver, f: IntentFilter, exported: Boolean) {
    if (Build.VERSION.SDK_INT >= 33) {
        registerReceiver(r, f, if (exported) Context.RECEIVER_EXPORTED else Context.RECEIVER_NOT_EXPORTED)
    } else {
        @Suppress("UnspecifiedRegisterReceiverFlag")
        registerReceiver(r, f)
    }
}

@Suppress("DEPRECATION")
fun Intent.usbDevice(): UsbDevice? =
    if (Build.VERSION.SDK_INT >= 33) getParcelableExtra(UsbManager.EXTRA_DEVICE, UsbDevice::class.java)
    else getParcelableExtra(UsbManager.EXTRA_DEVICE)
