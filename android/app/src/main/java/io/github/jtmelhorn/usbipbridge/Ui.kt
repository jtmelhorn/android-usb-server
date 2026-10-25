// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.jtmelhorn.usbipbridge

import android.content.Context
import android.graphics.Typeface
import android.view.ContextThemeWrapper
import android.view.View
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView

/** Small helpers for building D-pad friendly screens in code (no extra libraries). */
object Ui {
    fun dp(ctx: Context, v: Int) = (v * ctx.resources.displayMetrics.density).toInt()

    fun column(ctx: Context, padDp: Int = 48) = LinearLayout(ctx).apply {
        orientation = LinearLayout.VERTICAL
        val p = dp(ctx, padDp)
        setPadding(p, dp(ctx, 32), p, dp(ctx, 32))
    }

    fun row(ctx: Context) = LinearLayout(ctx).apply {
        orientation = LinearLayout.HORIZONTAL
        setPadding(0, dp(ctx, 12), 0, dp(ctx, 12))
    }

    fun text(ctx: Context, s: String = "", sizeSp: Float = 18f, mono: Boolean = false, color: Int? = null) =
        TextView(ctx).apply {
            text = s
            textSize = sizeSp
            if (mono) typeface = Typeface.MONOSPACE
            setTextColor(color ?: ctx.getColor(R.color.fg))
            setPadding(0, dp(ctx, 6), 0, dp(ctx, 6))
        }

    fun button(ctx: Context, label: String, onClick: (View) -> Unit) =
        Button(ContextThemeWrapper(ctx, R.style.TvButton), null, 0).apply {
            text = label
            isAllCaps = false
            setOnClickListener(onClick)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { marginEnd = dp(ctx, 16); bottomMargin = dp(ctx, 8) }
        }
}
