package dev.emusic.mobile.ui

import android.content.Context
import android.os.Build
import androidx.compose.material3.ColorScheme
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.core.graphics.ColorUtils

/** The app's accent colour: the wallpaper's (Android 12+) or a fixed seed. */
enum class Accent(val label: String, val seed: Color?) {
    Wallpaper("Wallpaper", null),
    Violet("Violet", Color(0xFF5B4FC4)),
    Orange("Orange", Color(0xFFE8742C)),
    Red("Red", Color(0xFFD23C3C)),
    Pink("Pink", Color(0xFFD0457F)),
    Blue("Blue", Color(0xFF2F6FD6)),
    Teal("Teal", Color(0xFF1E9A8F)),
    Green("Green", Color(0xFF3E9A3A)),
    Amber("Amber", Color(0xFFD9A21B)),
    ;

    companion object {
        /** Whether this device can follow the wallpaper (dynamic colour). */
        val wallpaperSupported: Boolean get() = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S

        /** The accents this device can show, in picker order. */
        fun available(): List<Accent> = entries.filter { it != Wallpaper || wallpaperSupported }

        /** The first-run accent: the wallpaper where supported, else violet. */
        val default: Accent get() = if (wallpaperSupported) Wallpaper else Violet
    }
}

/** The chosen accent, persisted in app preferences so it survives restarts. */
@Stable
class AccentPreference(context: Context) {
    private val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    var accent: Accent by mutableStateOf(load())
        private set

    fun select(choice: Accent) {
        accent = choice
        prefs.edit().putString(KEY, choice.name).apply()
    }

    private fun load(): Accent =
        prefs.getString(KEY, null)
            ?.let { name -> Accent.entries.firstOrNull { it.name == name } }
            ?.takeIf { it in Accent.available() }
            ?: Accent.default

    private companion object {
        const val PREFS = "emusic_ui"
        const val KEY = "accent"
    }
}

/** The accent preference for the picker; provided by [MainActivity]. */
val LocalAccentPreference = staticCompositionLocalOf<AccentPreference?> { null }

/** WCAG AA contrast for normal-size text. */
private const val MIN_CONTRAST = 4.5

/**
 * Re-tints `base` around `seed`: primary, its container, and the secondary
 * container (selected navigator rows, selected chips), so the whole UI
 * follows the accent while surfaces stay neutral.
 */
fun ColorScheme.withAccent(seed: Color, dark: Boolean): ColorScheme {
    val hsl = FloatArray(3).also { ColorUtils.colorToHSL(seed.toArgb(), it) }
    fun tone(lightness: Float, saturation: Float = hsl[1]): Color =
        Color(ColorUtils.HSLToColor(floatArrayOf(hsl[0], saturation.coerceIn(0f, 1f), lightness)))
    val muted = hsl[1] * 0.45f
    // Light-theme primary is used for text on the surface and as the filled-
    // button colour under white labels: darken it only as far as WCAG AA (4.5:1) needs, so
    // light seeds (teal, green, amber) stay readable and dark ones stay vivid.
    val lightPrimary = generateSequence(0.42f) { it - 0.02f }
        .takeWhile { it > 0.1f }
        .map { tone(it) }
        .firstOrNull { candidate ->
            listOf(Color.White, surface).all { background ->
                ColorUtils.calculateContrast(candidate.toArgb(), background.toArgb()) >= MIN_CONTRAST
            }
        }
        ?: tone(0.2f)
    return if (dark) {
        copy(
            primary = tone(0.78f),
            onPrimary = tone(0.18f),
            primaryContainer = tone(0.32f),
            onPrimaryContainer = tone(0.90f),
            secondaryContainer = tone(0.28f, muted),
            onSecondaryContainer = tone(0.90f, muted),
            surfaceTint = tone(0.78f),
        )
    } else {
        copy(
            primary = lightPrimary,
            onPrimary = Color.White,
            primaryContainer = tone(0.90f),
            onPrimaryContainer = tone(0.16f),
            secondaryContainer = tone(0.88f, muted),
            onSecondaryContainer = tone(0.16f, muted),
            surfaceTint = lightPrimary,
        )
    }
}
