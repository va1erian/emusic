package dev.emusic.mobile.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.emusic_mobile.Track

/** The bottom transport: title, previous/play-pause/next and a seek bar. */
@Composable
fun PlayerBar(
    track: Track,
    isPlaying: Boolean,
    preparing: Boolean,
    positionMs: Long,
    durationMs: Long,
    onSeek: (Long) -> Unit,
    onToggle: () -> Unit,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
) {
    HorizontalDivider()
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
            .testTag("now_playing"),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = track.displayTitle(),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = if (preparing) {
                        "Preparing…"
                    } else {
                        listOfNotNull(track.artist, track.album)
                            .filter { it.isNotBlank() }
                            .joinToString(" · ")
                    },
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            TextButton(onClick = onPrevious, enabled = !preparing) { Text("Prev") }
            TextButton(
                onClick = onToggle,
                enabled = !preparing,
                modifier = Modifier.testTag("play_pause"),
            ) {
                Text(if (isPlaying) "Pause" else "Play")
            }
            TextButton(onClick = onNext, enabled = !preparing) { Text("Next") }
        }
        Slider(
            value = positionMs.coerceAtLeast(0).toFloat(),
            onValueChange = { onSeek(it.toLong()) },
            valueRange = 0f..(if (durationMs > 0) durationMs.toFloat() else 1f),
            modifier = Modifier
                .fillMaxWidth()
                .testTag("seek"),
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(formatMillis(positionMs), style = MaterialTheme.typography.labelSmall)
            Text(formatMillis(durationMs), style = MaterialTheme.typography.labelSmall)
        }
    }
}

/** Formats milliseconds as `h:mm:ss` or `m:ss`. */
private fun formatMillis(millis: Long): String {
    val total = (millis / 1000).coerceAtLeast(0)
    val hours = total / 3600
    val minutes = (total % 3600) / 60
    val seconds = total % 60
    return if (hours > 0) {
        "%d:%02d:%02d".format(hours, minutes, seconds)
    } else {
        "%d:%02d".format(minutes, seconds)
    }
}
