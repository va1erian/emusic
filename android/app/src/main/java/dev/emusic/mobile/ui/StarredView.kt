package dev.emusic.mobile.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Star
import uniffi.emusic_mobile.Track

/** Ids of the server's starred tracks, so track rows can show a star. */
val LocalStarredIds = staticCompositionLocalOf<Set<String>> { emptySet() }

/** The Starred view's empty-state text: starring happens on the desktop. */
const val NO_STARRED = "No starred tracks yet. Star tracks in emusic on your desktop."

/** The small star shown next to a starred track. */
@Composable
fun StarMark() {
    Icon(
        Lucide.Star,
        contentDescription = "Starred",
        tint = MaterialTheme.colorScheme.primary,
        modifier = Modifier
            .padding(end = 8.dp)
            .size(16.dp)
            .testTag("star_mark"),
    )
}

/**
 * The read-only Starred playlist: the server's starred tracks, newest first,
 * with Play/Shuffle over the list. Stars are set on the desktop, so there is
 * no toggle here.
 */
@Composable
fun StarredView(
    tracks: List<Track>,
    nowPlayingId: String?,
    emptyText: String,
    onPlay: (Track, List<Track>) -> Unit,
    onShuffle: (List<Track>) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (tracks.isEmpty()) {
        EmptyState(emptyText, modifier, icon = Lucide.Star)
        return
    }
    Column(modifier = modifier.testTag("starred_view")) {
        Text(
            text = countLabel(tracks.size, "track"),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp),
        )
        PlayShuffleButtons(
            onPlay = { onPlay(tracks.first(), tracks) },
            onShuffle = { onShuffle(tracks) },
        )
        // Every row here is starred; the per-row star would only be noise.
        CompositionLocalProvider(LocalStarredIds provides emptySet()) {
            TrackList(
                tracks = tracks,
                nowPlayingId = nowPlayingId,
                emptyText = emptyText,
                onPlay = { onPlay(it, tracks) },
                modifier = Modifier.weight(1f),
            )
        }
    }
}
