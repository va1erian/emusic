package dev.emusic.mobile.ui

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

private val LightColors = lightColorScheme(
    primary = Color(0xFF5B4FC4),
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFE5DEFF),
    onPrimaryContainer = Color(0xFF180066),
    secondary = Color(0xFF5F5C71),
    background = Color(0xFFFDF8FF),
    surface = Color(0xFFFDF8FF),
    surfaceVariant = Color(0xFFE6E0F0),
)

private val DarkColors = darkColorScheme(
    primary = Color(0xFFC8BFFF),
    onPrimary = Color(0xFF2A1B86),
    primaryContainer = Color(0xFF41349E),
    onPrimaryContainer = Color(0xFFE5DEFF),
    secondary = Color(0xFFC8C4DA),
    background = Color(0xFF131218),
    surface = Color(0xFF131218),
    surfaceVariant = Color(0xFF48454E),
)

/**
 * The app theme. Follows the system light/dark setting. [accent] picks the
 * colour: the wallpaper's dynamic palette (Android 12+), or a fixed seed
 * re-tinting the emusic palette.
 */
@Composable
fun EmusicTheme(
    accent: Accent = Accent.default,
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    val context = LocalContext.current
    val seed = accent.seed
    val colorScheme = when {
        seed == null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }

        else -> {
            val base = if (darkTheme) DarkColors else LightColors
            seed?.let { base.withAccent(it, darkTheme) } ?: base
        }
    }
    MaterialTheme(colorScheme = colorScheme, content = content)
}
