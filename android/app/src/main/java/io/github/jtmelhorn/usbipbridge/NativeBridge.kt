// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.jtmelhorn.usbipbridge

import java.io.IOException

/**
 * JNI surface of the Rust core (`core/src/jni_bridge.rs`).
 *
 * The handle is an owned native pointer: callers must not call [status] concurrently with or after
 * [stop] for the same handle. [BridgeService] guarantees this by making every call on its single
 * worker thread.
 */
object NativeBridge {
    init {
        System.loadLibrary("usbipcore")
    }

    /** Starts the USB/IP server on [port]; throws [IOException] with a readable message on failure. */
    @Throws(IOException::class)
    external fun nativeStart(
        fd: Int,
        rawDescriptors: ByteArray,
        busNum: Int,
        devNum: Int,
        path: String,
        port: Int,
        allowedClient: String?,
        verbose: Boolean,
    ): Long

    /** Stops the server, cancels in-flight transfers and waits for them (bounded, about 1 s). */
    external fun nativeStop(handle: Long)

    /** JSON status snapshot (see `core/src/stats.rs` for field definitions). */
    external fun nativeStatus(handle: Long): String

    external fun nativeVersion(): String
}
