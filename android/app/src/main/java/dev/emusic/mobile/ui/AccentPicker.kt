package dev.emusic.mobile.ui

import android.os.Build
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.composables.icons.lucide.Check
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Wallpaper

/** A dialog of accent swatches; picking one applies it at once. */
@Composable
fun AccentPickerDialog(
    selected: Accent,
    onSelect: (Accent) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Accent colour") },
        text = {
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
                // A dialog is its own window: expose its tags to UI automation too.
                modifier = Modifier
                    .semantics { testTagsAsResourceId = true }
                    .testTag("accent_picker"),
            ) {
                Accent.available().forEach { accent ->
                    Swatch(accent = accent, selected = accent == selected, onClick = { onSelect(accent) })
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } },
    )
}

@Composable
private fun Swatch(accent: Accent, selected: Boolean, onClick: () -> Unit) {
    val fill = accent.seed ?: wallpaperPrimary()
    // Keep the check legible on light swatches (e.g. a dark-theme wallpaper tone).
    val mark = if (fill.luminance() > 0.5f) Color.Black else Color.White
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .width(64.dp)
            .clip(MaterialTheme.shapes.small)
            .clickable(role = Role.RadioButton, onClick = onClick)
            .semantics { this.selected = selected }
            .testTag("accent_${accent.name.lowercase()}"),
    ) {
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier
                .size(44.dp)
                .clip(CircleShape)
                .background(fill)
                .border(
                    width = if (selected) 3.dp else 0.dp,
                    color = if (selected) MaterialTheme.colorScheme.onSurface else Color.Transparent,
                    shape = CircleShape,
                ),
        ) {
            when {
                selected -> Icon(Lucide.Check, contentDescription = null, tint = mark)
                accent.seed == null -> Icon(Lucide.Wallpaper, contentDescription = null, tint = mark)
            }
        }
        Text(
            text = accent.label,
            style = MaterialTheme.typography.labelSmall,
            textAlign = TextAlign.Center,
        )
    }
}

/** The wallpaper palette's primary colour, to preview the Wallpaper swatch. */
@Composable
private fun wallpaperPrimary(): Color {
    val context = LocalContext.current
    val dark = isSystemInDarkTheme()
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        (if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)).primary
    } else {
        Color.Gray
    }
}
