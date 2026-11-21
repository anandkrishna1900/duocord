package com.duochat.app.data

import kotlin.math.pow

/**
 * Built-in colour themes, the same set as the desktop app (apps/desktop/src/lib/themes.ts). A theme is five
 * colours; everything else is derived so a theme cannot end up with unreadable text. "classic" is not in this
 * list: it is the original look that follows the system light/dark setting and the accent picker.
 */
data class ThemeDef(val id: String, val name: String, val dark: Boolean, val bg: Int, val surface: Int, val text: Int, val accent: Int)

const val CLASSIC_PALETTE = "classic"

val THEMES: List<ThemeDef> = listOf(
    ThemeDef("midnight", "Midnight", true, 0xFF0F1220.toInt(), 0xFF181B2E.toInt(), 0xFFDFE3F7.toInt(), 0xFF7AA2F7.toInt()),
    ThemeDef("graphite", "Graphite", true, 0xFF141414.toInt(), 0xFF1E1E20.toInt(), 0xFFE8E8EA.toInt(), 0xFF8AB4F8.toInt()),
    ThemeDef("amoled", "AMOLED black", true, 0xFF000000.toInt(), 0xFF0C0C0E.toInt(), 0xFFF1F1F3.toInt(), 0xFFC4A1FF.toInt()),
    ThemeDef("ocean", "Ocean", true, 0xFF07131B.toInt(), 0xFF0E212D.toInt(), 0xFFDCF0F7.toInt(), 0xFF3CC3E8.toInt()),
    ThemeDef("forest", "Forest", true, 0xFF1E2326.toInt(), 0xFF272E33.toInt(), 0xFFD3C6AA.toInt(), 0xFFA7C080.toInt()),
    ThemeDef("sunset", "Sunset", true, 0xFF1C1210.toInt(), 0xFF2B1C18.toInt(), 0xFFFBEEE6.toInt(), 0xFFFF8A4C.toInt()),
    ThemeDef("rose", "Rosé", true, 0xFF191724.toInt(), 0xFF1F1D2E.toInt(), 0xFFE0DEF4.toInt(), 0xFFEB6F92.toInt()),
    ThemeDef("lavender", "Lavender", true, 0xFF181825.toInt(), 0xFF1E1E2E.toInt(), 0xFFCDD6F4.toInt(), 0xFFCBA6F7.toInt()),
    ThemeDef("nord", "Nord", true, 0xFF2E3440.toInt(), 0xFF3B4252.toInt(), 0xFFECEFF4.toInt(), 0xFF88C0D0.toInt()),
    ThemeDef("dracula", "Dracula", true, 0xFF282A36.toInt(), 0xFF343746.toInt(), 0xFFF8F8F2.toInt(), 0xFFBD93F9.toInt()),
    ThemeDef("gruvbox", "Gruvbox", true, 0xFF282828.toInt(), 0xFF3C3836.toInt(), 0xFFEBDBB2.toInt(), 0xFFFABD2F.toInt()),
    ThemeDef("solarized-dark", "Solarized dark", true, 0xFF002B36.toInt(), 0xFF073642.toInt(), 0xFFEEE8D5.toInt(), 0xFF2F95C9.toInt()),
    ThemeDef("blossom", "Blossom pink", false, 0xFFFFF1F5.toInt(), 0xFFFFFFFF.toInt(), 0xFF2D1B24.toInt(), 0xFFD6336C.toInt()),
    ThemeDef("paper", "Paper", false, 0xFFF4EFE6.toInt(), 0xFFFFFCF6.toInt(), 0xFF2B2620.toInt(), 0xFFB45F06.toInt()),
    ThemeDef("sky", "Sky", false, 0xFFEAF3FB.toInt(), 0xFFFFFFFF.toInt(), 0xFF10222E.toInt(), 0xFF1478C8.toInt()),
    ThemeDef("mint", "Mint", false, 0xFFE9F6F0.toInt(), 0xFFFFFFFF.toInt(), 0xFF10261C.toInt(), 0xFF14805A.toInt()),
    ThemeDef("latte", "Latte", false, 0xFFEFF1F5.toInt(), 0xFFFFFFFF.toInt(), 0xFF262940.toInt(), 0xFF8839EF.toInt()),
    ThemeDef("solarized-light", "Solarized light", false, 0xFFFDF6E3.toInt(), 0xFFEEE8D5.toInt(), 0xFF073642.toInt(), 0xFF1F78B4.toInt()),
)

fun themeById(id: String): ThemeDef? = THEMES.firstOrNull { it.id == id }

private fun channel(c: Int, shift: Int) = (c shr shift) and 0xFF

/** [t] of the way from [a] to [b], per channel. */
fun mixColors(a: Int, b: Int, t: Float): Int {
    fun m(shift: Int) = (channel(a, shift) + (channel(b, shift) - channel(a, shift)) * t).toInt().coerceIn(0, 255)
    return (0xFF shl 24) or (m(16) shl 16) or (m(8) shl 8) or m(0)
}

private fun luminance(c: Int): Double {
    fun lin(v: Int): Double {
        val s = v / 255.0
        return if (s <= 0.03928) s / 12.92 else ((s + 0.055) / 1.055).pow(2.4)
    }
    return 0.2126 * lin(channel(c, 16)) + 0.7152 * lin(channel(c, 8)) + 0.0722 * lin(channel(c, 0))
}

/** WCAG contrast ratio, 1 to 21. */
fun contrastRatio(a: Int, b: Int): Double {
    val (hi, lo) = listOf(luminance(a), luminance(b)).sortedDescending()
    return (hi + 0.05) / (lo + 0.05)
}

/** Darken the accent until white text on it reaches 4.5:1, so filled buttons and own message bubbles stay readable. */
fun accentFill(accent: Int): Int {
    var fill = accent
    var i = 0
    while (i < 20 && contrastRatio(0xFFFFFFFF.toInt(), fill) < 4.5) {
        fill = mixColors(fill, 0xFF000000.toInt(), 0.08f)
        i++
    }
    return fill
}

fun mutedText(t: ThemeDef): Int = mixColors(t.text, t.bg, 0.42f)
