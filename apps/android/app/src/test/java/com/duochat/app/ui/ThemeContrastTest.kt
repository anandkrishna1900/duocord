package com.duochat.app.ui

import androidx.compose.material3.ColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import com.duochat.app.data.THEMES
import com.duochat.app.data.accentFill
import com.duochat.app.data.contrastRatio
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Every text colour of the Material scheme must read (4.5 to 1) on every surface it can sit on, in every built-in theme
 * and in the default plum look. This is what makes labels on buttons, chips and tabs never disappear.
 */
class ThemeContrastTest {
    private val plum = Color(0xFF7A3E73)

    private fun all(): List<Pair<String, ColorScheme>> =
        THEMES.map { it.name to scheme(it.dark, Color(it.bg), Color(it.surface), Color(it.text), Color(it.accent)) } +
            listOf(
                "default dark" to scheme(true, Color(0xFF131112), Color(0xFF1C191B), Color(0xFFF1ECEA), plum),
                "default light" to scheme(false, Color(0xFFF7F3EE), Color(0xFFFFFFFF), Color(0xFF221C1F), plum),
            )

    @Test
    fun `labels read on the colour they sit on`() {
        val failures = mutableListOf<String>()
        for ((name, c) in all()) {
            val pairs = listOf(
                "onPrimary/primary" to (c.onPrimary to c.primary),
                "onSecondary/secondary" to (c.onSecondary to c.secondary),
                "onTertiary/tertiary" to (c.onTertiary to c.tertiary),
                "onPrimaryContainer/primaryContainer" to (c.onPrimaryContainer to c.primaryContainer),
                "onSecondaryContainer/secondaryContainer" to (c.onSecondaryContainer to c.secondaryContainer),
                "onTertiaryContainer/tertiaryContainer" to (c.onTertiaryContainer to c.tertiaryContainer),
                "onErrorContainer/errorContainer" to (c.onErrorContainer to c.errorContainer),
                "inverseOnSurface/inverseSurface" to (c.inverseOnSurface to c.inverseSurface),
            ) + listOf(c.background, c.surfaceContainer, c.surfaceContainerHigh, c.surfaceContainerHighest).flatMapIndexed { i, s ->
                listOf(
                    "onSurface/surface#$i" to (c.onSurface to s),
                    "onSurfaceVariant/surface#$i" to (c.onSurfaceVariant to s),
                    "primary text/surface#$i" to (c.primary to s),
                    "error text/surface#$i" to (c.error to s),
                )
            }
            for ((label, p) in pairs) {
                val ratio = contrastRatio(p.first.toArgb(), p.second.toArgb())
                if (ratio < 4.5) failures += "$name $label ${"%.2f".format(ratio)}"
            }
            val own = Color(accentFill(c.primary.toArgb()))
            assertTrue("$name white on own bubble", contrastRatio(Color.White.toArgb(), own.toArgb()) >= 4.5)
        }
        assertTrue("Unreadable pairs:\n" + failures.joinToString("\n"), failures.isEmpty())
    }
}
