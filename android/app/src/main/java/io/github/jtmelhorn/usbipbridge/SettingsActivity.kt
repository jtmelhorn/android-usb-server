// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.jtmelhorn.usbipbridge

import android.app.Activity
import android.os.Bundle
import android.text.InputType
import android.widget.CheckBox
import android.widget.EditText
import android.widget.ScrollView

class SettingsActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val prefs = Prefs(this)
        val col = Ui.column(this)
        col.addView(Ui.text(this, getString(R.string.settings), 26f))
        col.addView(Ui.text(this, "Changes apply the next time sharing starts.", 15f, color = getColor(R.color.muted)))

        col.addView(Ui.text(this, getString(R.string.port)))
        val port = EditText(this).apply {
            inputType = InputType.TYPE_CLASS_NUMBER
            setText(prefs.port.toString())
            setTextColor(getColor(R.color.fg))
        }
        col.addView(port)

        col.addView(Ui.text(this, getString(R.string.allowed_client)))
        val allowed = EditText(this).apply {
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
            setText(prefs.allowedClient)
            hint = "e.g. 192.168.15.133"
            setTextColor(getColor(R.color.fg))
        }
        col.addView(allowed)

        val verbose = CheckBox(this).apply {
            text = getString(R.string.verbose)
            isChecked = prefs.verbose
            setTextColor(getColor(R.color.fg))
            textSize = 18f
        }
        col.addView(verbose)

        col.addView(
            Ui.text(
                this,
                "Security: USB/IP has no authentication or encryption. Connections are only accepted " +
                    "from local-network addresses. Never forward this port on your router.",
                15f, color = getColor(R.color.muted),
            )
        )

        val msg = Ui.text(this, "", 16f, color = getColor(R.color.accent))
        col.addView(Ui.button(this, getString(R.string.save)) {
            val p = port.text.toString().trim().toIntOrNull()
            val a = allowed.text.toString().trim()
            when {
                p == null || p !in 1..65535 -> msg.text = "Port must be 1–65535."
                a.isNotEmpty() && !looksLikeIp(a) -> msg.text = "Allowed client must be an IP address."
                else -> {
                    prefs.port = p
                    prefs.allowedClient = a
                    prefs.verbose = verbose.isChecked
                    msg.text = "Saved."
                }
            }
        })
        col.addView(msg)
        setContentView(ScrollView(this).apply { addView(col) })
    }

    private fun looksLikeIp(s: String): Boolean {
        val v4 = Regex("""^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}$""")
        return v4.matches(s) || (s.contains(':') && s.all { it.isLetterOrDigit() || it == ':' || it == '.' })
    }
}
