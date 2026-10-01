package dev.emusic.mobile.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Play
import com.composables.icons.lucide.Shuffle
import uniffi.emusic_mobile.Track

/** Minimum cover width; the grid fits as many columns as the pane allows. */
private val CELL = 132.dp

/** The Albums view: a toolbar (count, sort, shuffle) over a cover grid. */
@Composable
fun AlbumGrid(
    albums: List<AlbumGroup>,
    sort: AlbumSort,
    onSort: (AlbumSort) -> Unit,
    onOpen: (AlbumGroup) -> Unit,
    onShuffle: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val state = rememberLazyGridState()
    FastScrollBox(state = rememberGridScrollAdapter(state), modifier = modifier) {
        LazyVerticalGrid(
            columns = GridCells.Adaptive(CELL),
            state = state,
            contentPadding = PaddingValues(bottom = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
            modifier = Modifier.testTag("album_grid"),
        ) {
            fullWidth("album_toolbar") {
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = countLabel(albums.size, "album"),
                            style = MaterialTheme.typography.titleSmall,
                            modifier = Modifier.weight(1f),
                        )
                        OutlinedButton(onClick = onShuffle, enabled = albums.isNotEmpty()) {
                            Icon(Lucide.Shuffle, contentDescription = null)
                            Text("Shuffle", modifier = Modifier.padding(start = 8.dp))
                        }
                    }
                    Row(
                        modifier = Modifier.horizontalScroll(rememberScrollState()),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        AlbumSort.entries.forEach { candidate ->
                            FilterChip(
                                selected = candidate == sort,
                                onClick = { onSort(candidate) },
                                label = { Text(candidate.label) },
                            )
                        }
                    }
                }
            }
            if (albums.isEmpty()) {
                fullWidth("album_empty") { EmptyState("No albums match.") }
            }
            items(albums, key = { it.key }) { album ->
                AlbumCell(album = album, onClick = { onOpen(album) })
            }
        }
    }
}

/** One cover with its title, artist and year beneath, like the desktop grid. */
@Composable
fun AlbumCell(album: AlbumGroup, onClick: () -> Unit, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier
            .clickable(onClick = onClick)
            .testTag("album_cell"),
    ) {
        AlbumArt(artId = album.artId, colorKey = album.key)
        Text(
            text = album.album,
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 6.dp),
        )
        if (album.artist.isNotEmpty()) {
            Text(
                text = album.artist,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        album.year?.let {
            Text(
                text = it.toString(),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/**
 * One album: cover and facts beside Play/Shuffle, then its tracks numbered
 * (with disc headings when the album spans several discs).
 */
@Composable
fun AlbumDetail(
    album: AlbumGroup,
    nowPlayingId: String?,
    onPlay: (Track, List<Track>) -> Unit,
    onShuffle: (List<Track>) -> Unit,
    onOpenArtist: ((String) -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val multiDisc = album.tracks.mapNotNull { it.discNo }.distinct().size > 1
    LazyVerticalGrid(
        columns = GridCells.Fixed(1),
        contentPadding = PaddingValues(bottom = 16.dp),
        modifier = modifier.testTag("album_detail"),
    ) {
        item(key = "header") {
            Row(
                modifier = Modifier.padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                AlbumArt(
                    artId = album.artId,
                    colorKey = album.key,
                    modifier = Modifier.width(140.dp),
                )
                Column(
                    modifier = Modifier.weight(1f),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Text(album.album, style = MaterialTheme.typography.titleLarge)
                    if (album.artist.isNotEmpty()) {
                        Text(
                            text = album.artist,
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = if (onOpenArtist != null) {
                                Modifier.clickable { onOpenArtist(album.artist) }
                            } else {
                                Modifier
                            },
                        )
                    }
                    Text(
                        text = listOfNotNull(
                            album.year?.toString(),
                            countLabel(album.tracks.size, "track"),
                            album.durationSecs.takeIf { it > 0 }?.let(::formatDuration),
                        ).joinToString(" · "),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    PlayShuffleButtons(
                        onPlay = { album.tracks.firstOrNull()?.let { onPlay(it, album.tracks) } },
                        onShuffle = { onShuffle(album.tracks) },
                    )
                }
            }
        }
        var disc: UInt? = null
        album.tracks.forEachIndexed { index, track ->
            if (multiDisc && track.discNo != disc) {
                disc = track.discNo
                item(key = "disc_${track.discNo}_$index") {
                    Text(
                        text = "Disc ${track.discNo ?: "?"}",
                        style = MaterialTheme.typography.labelLarge,
                        modifier = Modifier.padding(top = 12.dp, bottom = 4.dp),
                    )
                }
            }
            item(key = track.id) {
                TrackRow(
                    track = track,
                    leading = (track.trackNo ?: (index + 1).toUInt()).toString(),
                    showAlbum = false,
                    playing = track.id == nowPlayingId,
                    onClick = { onPlay(track, album.tracks) },
                )
            }
        }
    }
}

/**
 * An artist's or genre's page: a heading with Play/Shuffle, its albums as a
 * cover grid, then every track (including those without an album).
 */
@Composable
fun GroupDetail(
    title: String,
    tracks: List<Track>,
    albums: List<AlbumGroup>,
    nowPlayingId: String?,
    onOpenAlbum: (AlbumGroup) -> Unit,
    onPlay: (Track, List<Track>) -> Unit,
    onShuffle: (List<Track>) -> Unit,
    modifier: Modifier = Modifier,
) {
    val state = rememberLazyGridState()
    FastScrollBox(state = rememberGridScrollAdapter(state), modifier = modifier) {
        LazyVerticalGrid(
            columns = GridCells.Adaptive(CELL),
            state = state,
            contentPadding = PaddingValues(bottom = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier.testTag("group_detail"),
        ) {
            fullWidth("header") {
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(title, style = MaterialTheme.typography.titleLarge)
                    Text(
                        text = listOf(countLabel(albums.size, "album"), countLabel(tracks.size, "track"))
                            .joinToString(" · "),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    PlayShuffleButtons(
                        onPlay = { tracks.firstOrNull()?.let { onPlay(it, tracks) } },
                        onShuffle = { onShuffle(tracks) },
                    )
                }
            }
            if (albums.isNotEmpty()) {
                fullWidth("albums_heading") { SectionTitle("Albums") }
                items(albums, key = { it.key }) { album ->
                    AlbumCell(album = album, onClick = { onOpenAlbum(album) })
                }
            }
            fullWidth("tracks_heading") { SectionTitle("Tracks") }
            items(tracks, key = { it.id }, span = { GridItemSpan(maxLineSpan) }) { track ->
                TrackRow(
                    track = track,
                    playing = track.id == nowPlayingId,
                    onClick = { onPlay(track, tracks) },
                )
            }
        }
    }
}

@Composable
private fun PlayShuffleButtons(onPlay: () -> Unit, onShuffle: () -> Unit) {
    // Wraps rather than squeezing the labels on a narrow (folded) screen.
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
        modifier = Modifier.padding(top = 4.dp),
    ) {
        Button(onClick = onPlay, modifier = Modifier.testTag("play_all")) {
            Icon(Lucide.Play, contentDescription = null)
            Text("Play", modifier = Modifier.padding(start = 8.dp))
        }
        OutlinedButton(onClick = onShuffle, modifier = Modifier.testTag("shuffle_all")) {
            Icon(Lucide.Shuffle, contentDescription = null)
            Text("Shuffle", modifier = Modifier.padding(start = 8.dp))
        }
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleMedium,
        modifier = Modifier.padding(top = 12.dp, bottom = 4.dp),
    )
}

/** A grid item spanning the whole row (headings, toolbars, empty states). */
private fun LazyGridScope.fullWidth(key: String, content: @Composable () -> Unit) {
    item(key = key, span = { GridItemSpan(maxLineSpan) }) { content() }
}

/** "1 album", "12 albums". */
fun countLabel(count: Int, noun: String): String = if (count == 1) "1 $noun" else "$count ${noun}s"
