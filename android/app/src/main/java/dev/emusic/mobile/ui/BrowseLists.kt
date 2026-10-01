package dev.emusic.mobile.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.composables.icons.lucide.AudioLines
import com.composables.icons.lucide.Folder
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Search
import com.composables.icons.lucide.X
import uniffi.emusic_mobile.Track

/** The library-wide search box; it filters every view, as on the desktop. */
@Composable
fun SearchField(
    query: String,
    onQueryChange: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    OutlinedTextField(
        value = query,
        onValueChange = onQueryChange,
        placeholder = { Text("Search library") },
        leadingIcon = { Icon(Lucide.Search, contentDescription = null) },
        trailingIcon = {
            if (query.isNotEmpty()) {
                IconButton(onClick = { onQueryChange("") }) {
                    Icon(Lucide.X, contentDescription = "Clear search")
                }
            }
        },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
            .testTag("search"),
    )
}

/** Format chips with per-format counts. */
@Composable
fun FormatFilter(
    tracks: List<Track>,
    selected: String?,
    onSelect: (String?) -> Unit,
    modifier: Modifier = Modifier,
) {
    val formats = tracks
        .groupingBy { it.format }
        .eachCount()
        .entries
        .sortedByDescending { it.value }
    Row(
        modifier = modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(vertical = 4.dp)
            .testTag("format_filter"),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        FilterChip(
            selected = selected == null,
            onClick = { onSelect(null) },
            label = { Text("All ${tracks.size}") },
        )
        formats.forEach { (format, count) ->
            FilterChip(
                selected = selected == format,
                onClick = { onSelect(format) },
                label = { Text("$format $count") },
            )
        }
    }
}

/** A fast-scrolling track list; taps play the track with the list as queue. */
@Composable
fun TrackList(
    tracks: List<Track>,
    nowPlayingId: String?,
    emptyText: String,
    onPlay: (Track) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (tracks.isEmpty()) {
        EmptyState(emptyText, modifier)
        return
    }
    val state = rememberLazyListState()
    FastScrollBox(state = rememberListScrollAdapter(state), modifier = modifier) {
        LazyColumn(
            state = state,
            modifier = Modifier
                .fillMaxWidth()
                .testTag("track_list"),
        ) {
            items(tracks, key = { it.id }) { track ->
                TrackRow(
                    track = track,
                    playing = track.id == nowPlayingId,
                    onClick = { onPlay(track) },
                )
            }
        }
    }
}

/**
 * One track: title over "artist · album" (or just the artist inside an
 * album), duration trailing. [leading] is an optional track number.
 */
@Composable
fun TrackRow(
    track: Track,
    playing: Boolean,
    onClick: () -> Unit,
    leading: String? = null,
    showAlbum: Boolean = true,
) {
    val accent = if (playing) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface
    val playingIcon: @Composable () -> Unit = {
        Icon(Lucide.AudioLines, contentDescription = "Playing", tint = accent)
    }
    val number: @Composable () -> Unit = {
        Text(leading.orEmpty(), style = MaterialTheme.typography.labelLarge)
    }
    val leadingSlot = when {
        playing -> playingIcon
        leading != null -> number
        else -> null
    }
    ListItem(
        modifier = Modifier
            .clickable(onClick = onClick)
            .testTag("track_row"),
        colors = ListItemDefaults.colors(headlineColor = accent),
        leadingContent = leadingSlot,
        headlineContent = {
            Text(track.displayTitle(), maxLines = 1, overflow = TextOverflow.Ellipsis)
        },
        supportingContent = {
            val subtitle = if (showAlbum) track.subtitle() else track.artist.orEmpty().ifBlank { track.format }
            Text(subtitle, maxLines = 1, overflow = TextOverflow.Ellipsis)
        },
        trailingContent = {
            Text(
                text = track.durationSecs?.let { formatDuration(it) } ?: track.format,
                style = MaterialTheme.typography.labelMedium,
            )
        },
    )
}

/** Artists or genres: a name with its track count. */
@Composable
fun NamedList(
    groups: List<NamedGroup>,
    emptyText: String,
    tag: String,
    onOpen: (NamedGroup) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (groups.isEmpty()) {
        EmptyState(emptyText, modifier)
        return
    }
    val state = rememberLazyListState()
    FastScrollBox(state = rememberListScrollAdapter(state), modifier = modifier) {
        LazyColumn(state = state, modifier = Modifier.fillMaxWidth().testTag("${tag}_list")) {
            items(groups, key = { it.name }) { group ->
                ListItem(
                    modifier = Modifier
                        .clickable { onOpen(group) }
                        .testTag("${tag}_row"),
                    headlineContent = {
                        Text(group.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    },
                    supportingContent = { Text(countLabel(group.tracks.size, "track")) },
                )
            }
        }
    }
}

@Composable
fun FolderList(
    folders: List<FolderGroup>,
    onOpen: (FolderGroup) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (folders.isEmpty()) {
        EmptyState("No folders match.", modifier)
        return
    }
    val state = rememberLazyListState()
    FastScrollBox(state = rememberListScrollAdapter(state), modifier = modifier) {
        LazyColumn(state = state, modifier = Modifier.fillMaxWidth().testTag("folder_list")) {
            items(folders, key = { it.path }) { group ->
                ListItem(
                    modifier = Modifier
                        .clickable { onOpen(group) }
                        .testTag("folder_row"),
                    leadingContent = { Icon(Lucide.Folder, contentDescription = null) },
                    headlineContent = {
                        Text(group.path.ifEmpty { "/" }, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    },
                    supportingContent = { Text(countLabel(group.tracks.size, "track")) },
                )
            }
        }
    }
}

/** One folder's tracks, with an include-subfolders toggle. */
@Composable
fun FolderDetail(
    path: String,
    tracks: List<Track>,
    includeSubfolders: Boolean,
    onToggleSubfolders: (Boolean) -> Unit,
    nowPlayingId: String?,
    onPlay: (Track) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .testTag("folder_filter"),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = path.ifEmpty { "/" },
                style = MaterialTheme.typography.titleSmall,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            FilterChip(
                selected = includeSubfolders,
                onClick = { onToggleSubfolders(!includeSubfolders) },
                label = { Text("Subfolders") },
                modifier = Modifier.testTag("subfolders_toggle"),
            )
        }
        TrackList(
            tracks = tracks,
            nowPlayingId = nowPlayingId,
            emptyText = "This folder has no tracks.",
            onPlay = onPlay,
            modifier = Modifier.weight(1f),
        )
    }
}

/** A centred note for an empty list, so the pane never looks broken. */
@Composable
fun EmptyState(text: String, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(32.dp)
            .testTag("empty_state"),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            Lucide.Search,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(32.dp),
        )
        Text(
            text = text,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 12.dp),
        )
    }
}

/** Formats seconds as `h:mm:ss` or `m:ss`. */
fun formatDuration(seconds: Double): String {
    val total = seconds.toLong().coerceAtLeast(0)
    val hours = total / 3600
    val minutes = (total % 3600) / 60
    val secs = total % 60
    return if (hours > 0) {
        "%d:%02d:%02d".format(hours, minutes, secs)
    } else {
        "%d:%02d".format(minutes, secs)
    }
}
