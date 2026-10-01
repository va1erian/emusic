package dev.emusic.mobile.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Pause
import com.composables.icons.lucide.Play
import com.composables.icons.lucide.Repeat
import com.composables.icons.lucide.Repeat1
import com.composables.icons.lucide.Shuffle
import com.composables.icons.lucide.SkipBack
import com.composables.icons.lucide.SkipForward
import uniffi.emusic_mobile.Track

/**
 * The bottom transport: cover and title (tap to open Now Playing),
 * shuffle/prev/play-pause/next/repeat and a seek bar. It alone reads the
 * polled position, so the 500 ms tick recomposes nothing else.
 */
@Composable
fun PlayerBar(player: PlayerState, onOpenNowPlaying: () -> Unit) {
    val track = player.nowPlaying ?: return
    val preparing = player.preparing
    val isPlaying = player.isPlaying
    val shuffle = player.shuffle
    val repeatMode = player.repeatMode
    val positionMs = player.positionMs
    val durationMs = player.durationMs
    HorizontalDivider()
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 4.dp)
            .testTag("now_playing"),
    ) {
        // Narrow (a phone, a folded cover screen): the title gets its own line
        // above the buttons instead of being squeezed beside them.
        BoxWithConstraints(modifier = Modifier.fillMaxWidth()) {
            if (maxWidth < SINGLE_ROW_MIN_WIDTH) {
                Column {
                    TrackSummary(track, preparing, onOpenNowPlaying, Modifier.fillMaxWidth())
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceEvenly,
                    ) {
                        TransportButtons(player, preparing, isPlaying, shuffle, repeatMode)
                    }
                }
            } else {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    TrackSummary(track, preparing, onOpenNowPlaying, Modifier.weight(1f))
                    TransportButtons(player, preparing, isPlaying, shuffle, repeatMode)
                }
            }
        }
        Slider(
            value = positionMs.coerceAtLeast(0).toFloat(),
            onValueChange = { player.seek(it.toLong()) },
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

/** Below this width the transport stacks the title over the buttons. */
private val SINGLE_ROW_MIN_WIDTH = 520.dp

/** Cover, title and "artist · album"; tapping opens Now Playing. */
@Composable
private fun TrackSummary(
    track: Track,
    preparing: Boolean,
    onOpenNowPlaying: () -> Unit,
    modifier: Modifier,
) {
    Row(
        modifier = modifier
            .clickable(onClickLabel = "Open now playing", onClick = onOpenNowPlaying)
            .testTag("open_now_playing"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        AlbumArt(
            artId = track.artId(),
            colorKey = albumKey(track) ?: track.id,
            modifier = Modifier.size(44.dp),
        )
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = track.displayTitle(),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                style = MaterialTheme.typography.titleSmall,
            )
            Text(
                text = if (preparing) "Preparing…" else track.subtitle(),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                style = MaterialTheme.typography.bodySmall,
            )
        }
    }
}

/** Shuffle, previous, play/pause, next and repeat. */
@Composable
private fun TransportButtons(
    player: PlayerState,
    preparing: Boolean,
    isPlaying: Boolean,
    shuffle: Boolean,
    repeatMode: RepeatMode,
) {
    IconButton(
        onClick = player::toggleShuffle,
        enabled = !preparing,
        modifier = Modifier.testTag("shuffle"),
    ) {
        Icon(
            imageVector = Lucide.Shuffle,
            contentDescription = if (shuffle) "Shuffle on" else "Shuffle off",
            tint = if (shuffle) {
                MaterialTheme.colorScheme.primary
            } else {
                LocalContentColor.current
            },
        )
    }
    IconButton(
        onClick = player::previous,
        enabled = !preparing,
        modifier = Modifier.testTag("previous"),
    ) {
        Icon(Lucide.SkipBack, contentDescription = "Previous")
    }
    IconButton(
        onClick = player::toggle,
        enabled = !preparing,
        modifier = Modifier.testTag("play_pause"),
    ) {
        Icon(
            imageVector = if (isPlaying) Lucide.Pause else Lucide.Play,
            contentDescription = if (isPlaying) "Pause" else "Play",
        )
    }
    IconButton(
        onClick = player::advance,
        enabled = !preparing,
        modifier = Modifier.testTag("next"),
    ) {
        Icon(Lucide.SkipForward, contentDescription = "Next")
    }
    IconButton(
        onClick = player::cycleRepeat,
        enabled = !preparing,
        modifier = Modifier.testTag("repeat"),
    ) {
        Icon(
            imageVector = if (repeatMode == RepeatMode.One) Lucide.Repeat1 else Lucide.Repeat,
            contentDescription = when (repeatMode) {
                RepeatMode.Off -> "Repeat off"
                RepeatMode.All -> "Repeat all"
                RepeatMode.One -> "Repeat one"
            },
            tint = if (repeatMode == RepeatMode.Off) {
                LocalContentColor.current
            } else {
                MaterialTheme.colorScheme.primary
            },
        )
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
