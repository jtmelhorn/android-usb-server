// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.jtmelhorn.usbipbridge

import android.os.Handler
import android.os.Looper
import org.json.JSONObject

/** Process-wide sharing state published by [BridgeService] and observed by the UI. */
object BridgeState {
    enum class Phase { STOPPED, STARTING, LISTENING, ATTACHED, WAITING_FOR_DEVICE, ERROR }

    @Volatile var phase: Phase = Phase.STOPPED
        private set
    @Volatile var message: String = "Sharing is off."
        private set
    @Volatile var status: JSONObject? = null
        private set
    @Volatile var port: Int = 3240
        private set

    private val main = Handler(Looper.getMainLooper())
    private val listeners = mutableListOf<() -> Unit>()

    fun update(phase: Phase, message: String, status: JSONObject? = this.status, port: Int = this.port) {
        this.phase = phase
        this.message = message
        this.status = status
        this.port = port
        main.post { listeners.toList().forEach { it() } }
    }

    fun updateStatus(status: JSONObject?) {
        this.status = status
    }

    fun addListener(l: () -> Unit) = main.post { listeners.add(l) }
    fun removeListener(l: () -> Unit) = main.post { listeners.remove(l) }
}
