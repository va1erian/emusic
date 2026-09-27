package dev.emusic.mobile.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.emusic_mobile.MobileCore
import uniffi.emusic_mobile.Track

/**
 * Connects to `url`, fetches the server's library and shows a summary plus the
 * track list. Reads only; playback comes later.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen(url: String, dataDir: String, onBack: () -> Unit) {
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var version by remember { mutableStateOf(0L) }
    var tracks by remember { mutableStateOf<List<Track>>(emptyList()) }
    var nonce by remember { mutableStateOf(0) }

    LaunchedEffect(nonce) {
        loading = true
        error = null
        val result = withContext(Dispatchers.IO) {
            runCatching { MobileCore(url, dataDir).library() }
        }
        result
            .onSuccess {
                version = it.version
                tracks = it.tracks
            }
            .onFailure { error = it.message ?: it.toString() }
        loading = false
    }

    Scaffold(
        modifier = Modifier.semantics { testTagsAsResourceId = true },
        topBar = {
            TopAppBar(
                title = { Text("Library") },
                navigationIcon = { TextButton(onClick = onBack) { Text("Back") } },
                actions = { TextButton(onClick = { nonce++ }) { Text("Refresh") } },
            )
        },
    ) { insets ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(insets)
                .padding(horizontal = 16.dp),
        ) {
            when {
                loading -> Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(24.dp),
                    horizontalArrangement = Arrangement.Center,
                ) {
                    CircularProgressIndicator()
                }

                error != null -> Column(
                    modifier = Modifier.padding(24.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        text = error ?: "",
                        color = MaterialTheme.colorScheme.error,
                    )
                    Button(onClick = { nonce++ }) { Text("Retry") }
                }

                else -> {
                    LibrarySummary(url = url, version = version, tracks = tracks)
                    HorizontalDivider()
                    TrackList(tracks)
                }
            }
        }
    }
}

@Composable
private fun LibrarySummary(url: String, version: Long, tracks: List<Track>) {
    val distinctAlbums = tracks.mapNotNull { it.album }.filter { it.isNotBlank() }.distinct().size
    val distinctArtists = tracks.mapNotNull { it.artist }.filter { it.isNotBlank() }.distinct().size
    val totalSeconds = tracks.sumOf { it.durationSecs ?: 0.0 }
    val formats = tracks
        .groupingBy { it.format }
        .eachCount()
        .entries
        .sortedByDescending { it.value }
    val special = tracks.count { it.specialized }

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 12.dp)
            .testTag("library_summary"),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(url, style = MaterialTheme.typography.titleMedium)
            Text("Version $version", style = MaterialTheme.typography.bodySmall)
            Text(
                text = "${tracks.size} tracks · $distinctAlbums albums · $distinctArtists artists",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                text = "${formatDuration(totalSeconds)} total · $special specialized",
                style = MaterialTheme.typography.bodySmall,
            )
            if (formats.isNotEmpty()) {
                Text(
                    text = formats.joinToString("  ") { "${it.key} ${it.value}" },
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            if (distinctAlbums == 0 && distinctArtists == 0) {
                Text(
                    text = "No tagged albums or artists on the server.",
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        }
    }
}

@Composable
private fun TrackList(tracks: List<Track>) {
    if (tracks.isEmpty()) {
        Text(
            text = "The server library is empty.",
            modifier = Modifier.padding(vertical = 12.dp),
        )
        return
    }
    LazyColumn(
        modifier = Modifier
            .fillMaxWidth()
            .testTag("track_list"),
    ) {
        items(tracks, key = { it.id }) { track ->
            ListItem(
                headlineContent = { Text(track.titleOrFileName()) },
                supportingContent = {
                    val subtitle = listOfNotNull(track.artist, track.album)
                        .filter { it.isNotBlank() }
                        .joinToString(" · ")
                    Text(if (subtitle.isBlank()) track.format else subtitle)
                },
                trailingContent = {
                    Text(
                        text = track.durationSecs?.let { formatDuration(it) } ?: track.format,
                        style = MaterialTheme.typography.labelMedium,
                    )
                },
            )
        }
    }
}

/** Title to display: the tagged title, else the file name, else the id. */
private fun Track.titleOrFileName(): String =
    title?.takeIf { it.isNotBlank() }
        ?: filename?.takeIf { it.isNotBlank() }
        ?: id

/** Formats seconds as `h:mm:ss` or `m:ss`. */
private fun formatDuration(seconds: Double): String {
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
