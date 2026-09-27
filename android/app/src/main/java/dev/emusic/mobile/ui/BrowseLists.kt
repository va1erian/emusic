package dev.emusic.mobile.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.emusic_mobile.Track

/** The top-level way the library is presented. */
enum class BrowseMode { Tracks, Albums, Artists, Folders }

/** Tracks grouped under one album. */
data class AlbumGroup(val album: String, val artist: String, val tracks: List<Track>)

/** Tracks that share a directory. */
data class FolderGroup(val path: String, val tracks: List<Track>)

/** Tracks grouped under one artist. */
data class ArtistGroup(val artist: String, val tracks: List<Track>)

/** Groups tracks by album name, alphabetically. */
fun albumsOf(tracks: List<Track>): List<AlbumGroup> =
    tracks
        .filter { !it.album.isNullOrBlank() }
        .groupBy { it.album!! }
        .map { (album, list) ->
            AlbumGroup(
                album = album,
                artist = list.firstNotNullOfOrNull { it.artist?.takeIf(String::isNotBlank) }.orEmpty(),
                tracks = list,
            )
        }
        .sortedBy { it.album.lowercase() }

/** Groups tracks by artist name, alphabetically. */
fun artistsOf(tracks: List<Track>): List<ArtistGroup> =
    tracks
        .filter { !it.artist.isNullOrBlank() }
        .groupBy { it.artist!! }
        .map { (artist, list) -> ArtistGroup(artist, list) }
        .sortedBy { it.artist.lowercase() }

/** Groups tracks by directory, alphabetically (root first). */
fun foldersOf(tracks: List<Track>): List<FolderGroup> =
    tracks
        .groupBy { it.directory }
        .map { (path, list) -> FolderGroup(path, list) }
        .sortedBy { it.path.lowercase() }

/** Whether `directory` is `folder` itself or nested under it. */
fun isInFolder(directory: String, folder: String, includeSubdirectories: Boolean): Boolean =
    when {
        !includeSubdirectories -> directory == folder
        // The root prefix is empty: everything is under it.
        folder.isEmpty() -> true
        else -> directory == folder || directory.startsWith("$folder/")
    }

/** Tracks ordered by directory, then track number, then title. */
fun sortTracksByDirectory(tracks: List<Track>): List<Track> =
    tracks.sortedWith(
        compareBy(
            { it.directory.lowercase() },
            { it.trackNo ?: 0u },
            { it.displayTitle().lowercase() },
        ),
    )

/** Free-text filter over title, artist and album. */
fun searchTracks(tracks: List<Track>, query: String): List<Track> {
    val needle = query.trim().lowercase()
    if (needle.isEmpty()) return tracks
    return tracks.filter { track ->
        track.displayTitle().lowercase().contains(needle) ||
            track.artist?.lowercase()?.contains(needle) == true ||
            track.album?.lowercase()?.contains(needle) == true
    }
}

@Composable
fun SearchField(
    query: String,
    onQueryChange: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    OutlinedTextField(
        value = query,
        onValueChange = onQueryChange,
        label = { Text("Search") },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
            .testTag("search"),
    )
}

@Composable
fun AlbumList(albums: List<AlbumGroup>, onSelect: (AlbumGroup) -> Unit, modifier: Modifier = Modifier) {
    LazyColumn(modifier = modifier.fillMaxWidth().testTag("album_list")) {
        items(albums, key = { it.album }) { group ->
            ListItem(
                modifier = Modifier.clickable { onSelect(group) }.testTag("album_row"),
                headlineContent = {
                    Text(group.album, maxLines = 1, overflow = TextOverflow.Ellipsis)
                },
                supportingContent = {
                    Text(
                        text = listOf(group.artist, "${group.tracks.size} tracks")
                            .filter { it.isNotBlank() }
                            .joinToString(" · "),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
            )
        }
    }
}

@Composable
fun ArtistList(artists: List<ArtistGroup>, onSelect: (ArtistGroup) -> Unit, modifier: Modifier = Modifier) {
    LazyColumn(modifier = modifier.fillMaxWidth().testTag("artist_list")) {
        items(artists, key = { it.artist }) { group ->
            ListItem(
                modifier = Modifier.clickable { onSelect(group) }.testTag("artist_row"),
                headlineContent = {
                    Text(group.artist, maxLines = 1, overflow = TextOverflow.Ellipsis)
                },
                supportingContent = { Text("${group.tracks.size} tracks") },
            )
        }
    }
}

@Composable
fun FolderList(folders: List<FolderGroup>, onSelect: (FolderGroup) -> Unit, modifier: Modifier = Modifier) {
    LazyColumn(modifier = modifier.fillMaxWidth().testTag("folder_list")) {
        items(folders, key = { it.path }) { group ->
            ListItem(
                modifier = Modifier.clickable { onSelect(group) }.testTag("folder_row"),
                headlineContent = {
                    Text(group.path.ifEmpty { "/" }, maxLines = 1, overflow = TextOverflow.Ellipsis)
                },
                supportingContent = { Text("${group.tracks.size} tracks") },
            )
        }
    }
}

@Composable
fun BrowseTabs(mode: BrowseMode, onSelect: (BrowseMode) -> Unit, modifier: Modifier = Modifier) {
    androidx.compose.foundation.layout.Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
            .testTag("browse_tabs"),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        BrowseMode.entries.forEach { candidate ->
            androidx.compose.material3.FilterChip(
                selected = candidate == mode,
                onClick = { onSelect(candidate) },
                label = { Text(candidate.name) },
            )
        }
    }
}

/** A banner naming the active album/artist filter, with a clear action. */
@Composable
fun FilterBanner(label: String, onClear: () -> Unit, modifier: Modifier = Modifier) {
    androidx.compose.foundation.layout.Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text("Filtered by $label", style = MaterialTheme.typography.bodySmall)
        androidx.compose.material3.TextButton(onClick = onClear) { Text("Clear") }
    }
}

/** The active folder filter, with an include-subdirectories toggle and clear. */
@Composable
fun FolderFilterBanner(
    folder: String,
    includeSubdirectories: Boolean,
    onToggleSubdirectories: (Boolean) -> Unit,
    onClear: () -> Unit,
    modifier: Modifier = Modifier,
) {
    androidx.compose.foundation.layout.Row(
        modifier = modifier
            .fillMaxWidth()
            .testTag("folder_filter"),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
    ) {
        Text(
            text = folder.ifEmpty { "/" },
            style = MaterialTheme.typography.bodySmall,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f, fill = false),
        )
        androidx.compose.material3.FilterChip(
            selected = includeSubdirectories,
            onClick = { onToggleSubdirectories(!includeSubdirectories) },
            label = { Text("Subfolders") },
            modifier = Modifier.testTag("subfolders_toggle"),
        )
        androidx.compose.material3.TextButton(onClick = onClear) { Text("Clear") }
    }
}
