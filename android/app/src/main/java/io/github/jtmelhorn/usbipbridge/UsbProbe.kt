// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.jtmelhorn.usbipbridge

import android.content.Context
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbEndpoint
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbRequest
import android.os.Build
import android.util.Log
import java.nio.ByteBuffer
import java.util.concurrent.TimeoutException

/**
 * Milestone-1 feasibility probe (TICKET-004): uses only public Android USB APIs to open the device,
 * force-claim every interface (detaching the kernel driver) and capture interrupt-IN reports.
 * Everything is logged under the `UsbIpProbe` tag for evidence collection.
 */
class UsbProbe(private val ctx: Context, private val device: UsbDevice, private val out: (String) -> Unit) {
    companion object {
        const val TAG = "UsbIpProbe"
    }

    private fun log(s: String) {
        Log.i(TAG, s)
        out(s)
    }

    fun run(seconds: Int) {
        log("=== probe ${UsbDevices.label(device)} ===")
        log("android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT}) ${Build.MANUFACTURER} ${Build.MODEL} abis=${Build.SUPPORTED_ABIS.joinToString()}")
        log("fingerprint ${Build.FINGERPRINT}")
        log("device ${device.deviceName} class %02x/%02x/%02x".format(device.deviceClass, device.deviceSubclass, device.deviceProtocol))
        log(UsbDevices.describeEndpoints(device).trimEnd())

        val usb = UsbDevices.manager(ctx)
        if (!usb.hasPermission(device)) {
            log("RESULT: no USB permission (grant it from the main screen)")
            return
        }
        val conn = usb.openDevice(device)
        if (conn == null) {
            log("RESULT: FAIL openDevice returned null")
            return
        }
        val claimed = mutableListOf<UsbInterface>()
        val requests = mutableListOf<Pair<UsbRequest, ByteBuffer>>()
        try {
            val raw = conn.rawDescriptors
            log("raw descriptors (${raw?.size ?: 0} bytes): ${raw?.joinToString(" ") { "%02x".format(it) }}")
            log("usbfs fd = ${conn.fileDescriptor}")

            var allClaimed = true
            for (i in 0 until device.interfaceCount) {
                val itf = device.getInterface(i)
                if (itf.alternateSetting != 0) continue
                val ok = conn.claimInterface(itf, true)
                log("claimInterface(${itf.id}, force=true) -> $ok")
                if (ok) claimed += itf else allClaimed = false
            }

            val inEps = mutableListOf<UsbEndpoint>()
            for (itf in claimed) for (e in 0 until itf.endpointCount) {
                val ep = itf.getEndpoint(e)
                if (ep.type == UsbConstants.USB_ENDPOINT_XFER_INT && ep.direction == UsbConstants.USB_DIR_IN) inEps += ep
            }
            if (inEps.isEmpty()) {
                log("RESULT: ${if (allClaimed) "claimed all interfaces but" else "FAIL:"} no interrupt-IN endpoint available")
                return
            }
            for (ep in inEps) {
                val r = UsbRequest()
                if (!r.initialize(conn, ep)) {
                    log("UsbRequest.initialize(ep 0x%02x) failed".format(ep.address))
                    continue
                }
                val buf = ByteBuffer.allocate(ep.maxPacketSize.coerceAtLeast(64))
                r.clientData = ep
                if (r.queue(buf)) requests += r to buf else log("queue(ep 0x%02x) failed".format(ep.address))
            }

            log("capturing for $seconds s — press buttons and move sticks now…")
            val counts = HashMap<Int, Int>()
            val changed = HashMap<Int, Int>()
            val last = HashMap<Int, String>()
            val intervalsUs = ArrayList<Long>()
            var lastNs = 0L
            val deadline = System.nanoTime() + seconds * 1_000_000_000L
            while (System.nanoTime() < deadline) {
                val r = try {
                    conn.requestWait(200)
                } catch (e: TimeoutException) {
                    null
                } ?: continue
                val now = System.nanoTime()
                val ep = r.clientData as UsbEndpoint
                val buf = requests.first { it.first === r }.second
                val n = buf.position()
                val hex = (0 until n).joinToString(" ") { "%02x".format(buf.get(it)) }
                counts[ep.address] = (counts[ep.address] ?: 0) + 1
                if (n > 0) {
                    if (last[ep.address] != hex) changed[ep.address] = (changed[ep.address] ?: 0) + 1
                    last[ep.address] = hex
                    if (lastNs != 0L) intervalsUs += (now - lastNs) / 1000
                    lastNs = now
                }
                val c = counts[ep.address]!!
                if (c <= 5 || c % 100 == 0) log("ep 0x%02x #%d (%d bytes): %s".format(ep.address, c, n, hex))
                buf.clear()
                r.queue(buf)
            }
            for (ep in inEps) {
                log("ep 0x%02x: %d reports, %d with changed payload".format(ep.address, counts[ep.address] ?: 0, changed[ep.address] ?: 0))
            }
            if (intervalsUs.isNotEmpty()) {
                intervalsUs.sort()
                fun pct(p: Int) = intervalsUs[((intervalsUs.size - 1) * p + 50) / 100] / 1000.0
                log(
                    "inter-report interval ms (report interval, NOT latency): min %.2f p50 %.2f p99 %.2f max %.2f n=%d"
                        .format(pct(0), pct(50), pct(99), pct(100), intervalsUs.size)
                )
            }
            val total = counts.values.sum()
            val anyChange = changed.values.sum() > 0
            log(
                when {
                    !allClaimed -> "RESULT: PARTIAL — some interfaces could not be claimed"
                    total >= 100 && anyChange -> "RESULT: PASS — claimed all interfaces and captured $total reports with changing input"
                    total > 0 -> "RESULT: CAPTURED $total reports (press more buttons for ≥100 with changes)"
                    else -> "RESULT: claimed interfaces but no reports arrived (is the controller connected to the adapter?)"
                }
            )
        } catch (e: Exception) {
            log("RESULT: FAIL ${e.javaClass.simpleName}: ${e.message}")
        } finally {
            for ((r, _) in requests) {
                r.cancel()
                r.close()
            }
            for (itf in claimed) conn.releaseInterface(itf)
            conn.close()
            log("released interfaces; Android's own driver may now re-bind")
        }
    }
}
