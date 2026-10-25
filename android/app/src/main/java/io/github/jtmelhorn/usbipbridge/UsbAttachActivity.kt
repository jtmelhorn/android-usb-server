// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.jtmelhorn.usbipbridge

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/**
 * Invisible target of USB_DEVICE_ATTACHED. When the user has ticked "always open for this device",
 * Android grants USB permission before launching this, which lets sharing resume automatically
 * after the adapter re-enumerates.
 */
class UsbAttachActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val prefs = Prefs(this)
        val d = intent.usbDevice()
        when {
            d != null && prefs.matches(d) && prefs.sharingEnabled -> BridgeService.start(this)
            prefs.selectedVid < 0 -> startActivity(Intent(this, MainActivity::class.java))
        }
        finish()
    }
}
