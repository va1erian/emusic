package dev.emusic.mobile.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import uniffi.emusic_mobile.Track

/**
 * The playing track's cover and full metadata (title, artist, album, year,
 * track number, genre, format, duration), then the queue with the current
 * entry highlighted. Tapping a queue entry jumps to it.
 */
@Composable
fun NowPlayingView(
    track: Track?,
    queue: List<Track>,
    queueIndex: Int,
    onPlayAt: (Int) -> Unit,
    onOpenAlbum: ((Track) -> Unit)?,
    onOpenArtist: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (track == null) {
        EmptyState("Nothing is playing. Pick a track to start.", modifier)
        return
    }
    LazyColumn(modifier = modifier.testTag("now_playing_view")) {
        item(key = "header") {
            Row(
                modifier = Modifier.padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                AlbumArt(
                    artId = track.artId(),
                    colorKey = albumKey(track) ?: track.id,
                    modifier = Modifier.widthIn(max = 180.dp).weight(0.45f),
                )
                Column(
                    modifier = Modifier.weight(0.55f),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Text(
                        text = track.displayTitle(),
                        style = MaterialTheme.typography.titleLarge,
                        modifier = Modifier.testTag("now_playing_title"),
                    )
                    track.artist?.takeIf { it.isNotBlank() }?.let { artist ->
                        Text(
                            text = artist,
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.clickable { onOpenArtist(artist) },
                        )
                    }
                    track.album?.takeIf { it.isNotBlank() }?.let { album ->
                        Text(
                            text = listOfNotNull(album, track.year?.let { "($it)" }).joinToString(" "),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = if (onOpenAlbum != null && albumKey(track) != null) {
                                Modifier.clickable { onOpenAlbum(track) }
                            } else {
                                Modifier
                            },
                        )
                    }
                    Text(
                        text = trackFacts(track),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
        item(key = "queue_heading") {
            Text(
                text = "Queue · ${countLabel(queue.size, "track")}",
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(top = 16.dp, bottom = 4.dp),
            )
        }
        itemsIndexed(queue, key = { index, entry -> "${index}_${entry.id}" }) { index, entry ->
            TrackRow(
                track = entry,
                leading = "${index + 1}.",
                playing = index == queueIndex,
                onClick = { onPlayAt(index) },
            )
        }
    }
}

/** "Track 3 · Disc 1 · Rock · FLAC · 4:12" for whatever is tagged. */
fun trackFacts(track: Track): String =
    listOfNotNull(
        track.trackNo?.let { "Track $it" },
        track.discNo?.let { "Disc $it" },
        track.genre?.takeIf { it.isNotBlank() },
        track.format.uppercase(),
        track.channels?.let { if (it == 1u) "mono" else "$it ch" },
        track.durationSecs?.let(::formatDuration),
    ).joinToString(" · ")
