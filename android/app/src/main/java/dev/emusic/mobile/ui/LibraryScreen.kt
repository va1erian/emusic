package dev.emusic.mobile.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.emusic_mobile.MobileCore
import uniffi.emusic_mobile.Track
import java.io.File

/**
 * Connects to `url`, fetches the server's library, shows a summary plus the
 * track list, and streams the standard formats with Media3.
 *
 * Specialized formats (SID, tracker modules, MIDI) are not streamed here: they
 * need the server-side `/render` path and are tracked in #430.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen(url: String, dataDir: String, onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val core = remember { runCatching { MobileCore(url, dataDir) }.getOrNull() }

    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var version by remember { mutableStateOf(0L) }
    var tracks by remember { mutableStateOf<List<Track>>(emptyList()) }
    var nonce by remember { mutableStateOf(0) }

    var player by remember { mutableStateOf<ExoPlayer?>(null) }
    var nowPlaying by remember { mutableStateOf<Track?>(null) }
    var isPlaying by remember { mutableStateOf(false) }
    var preparing by remember { mutableStateOf(false) }
    var playbackMessage by remember { mutableStateOf<String?>(null) }
    var formatFilter by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(nonce) {
        loading = true
        error = null
        val client = core
        if (client == null) {
            error = "invalid server"
            loading = false
            return@LaunchedEffect
        }
        val result = withContext(Dispatchers.IO) { runCatching { client.library() } }
        result
            .onSuccess {
                version = it.version
                tracks = it.tracks
            }
            .onFailure { error = it.message ?: it.toString() }
        loading = false
    }

    // Build the player once a token is available, and release it on leave.
    LaunchedEffect(core) {
        val client = core ?: return@LaunchedEffect
        val token = withContext(Dispatchers.IO) {
            runCatching { client.bearerToken() }.getOrNull()
        }
        token?.let { player = createAuthenticatedPlayer(context, it) }
    }

    DisposableEffect(player) {
        val active = player
        if (active == null) {
            onDispose { }
        } else {
            val listener = object : Player.Listener {
                override fun onIsPlayingChanged(playing: Boolean) {
                    isPlaying = playing
                }

                override fun onPlaybackStateChanged(state: Int) {
                    if (state == Player.STATE_ENDED) isPlaying = false
                }
            }
            active.addListener(listener)
            onDispose {
                active.removeListener(listener)
                active.release()
            }
        }
    }

    fun play(track: Track) {
        val client = core
        val active = player
        if (client == null || active == null) {
            playbackMessage = "not connected yet"
            return
        }
        playbackMessage = null

        if (!track.specialized) {
            nowPlaying = track
            active.setMediaItem(streamMediaItem(client.streamUrl(track.id)))
            active.prepare()
            active.play()
            return
        }

        // Tracker modules (MOD/XM/S3M/IT) render locally to a cached FLAC.
        if (client.canRender(track.format)) {
            nowPlaying = track
            preparing = true
            scope.launch {
                val result = withContext(Dispatchers.IO) {
                    runCatching { client.renderToFile(track.id, track.format) }
                }
                preparing = false
                result
                    .onSuccess { path ->
                        player?.run {
                            setMediaItem(streamMediaItem(File(path).toURI().toString()))
                            prepare()
                            play()
                        }
                    }
                    .onFailure { playbackMessage = "render failed: ${it.message}" }
            }
            return
        }

        // SID and anything else specialized: ask the server to render.
        nowPlaying = track
        active.setMediaItem(streamMediaItem(client.renderUrl(track.id, 0u)))
        active.prepare()
        active.play()
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
                    val visibleTracks = formatFilter
                        ?.let { filter -> tracks.filter { it.format == filter } }
                        ?: tracks
                    LibrarySummary(url = url, version = version, tracks = tracks)
                    HorizontalDivider()
                    FormatFilter(
                        tracks = tracks,
                        selected = formatFilter,
                        onSelect = { formatFilter = it },
                    )
                    TrackList(
                        tracks = visibleTracks,
                        onPlay = ::play,
                        modifier = Modifier.weight(1f),
                    )
                    playbackMessage?.let { message ->
                        Text(
                            text = message,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(vertical = 8.dp),
                        )
                    }
                    nowPlaying?.let { track ->
                        NowPlayingBar(
                            track = track,
                            isPlaying = isPlaying,
                            preparing = preparing,
                            onToggle = {
                                player?.let { active ->
                                    if (isPlaying) active.pause() else active.play()
                                }
                            },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun NowPlayingBar(
    track: Track,
    isPlaying: Boolean,
    preparing: Boolean,
    onToggle: () -> Unit,
) {
    HorizontalDivider()
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp)
            .testTag("now_playing"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = track.titleOrFileName(),
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
        TextButton(
            onClick = onToggle,
            enabled = !preparing,
            modifier = Modifier.testTag("play_pause"),
        ) {
            Text(if (isPlaying) "Pause" else "Play")
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
private fun FormatFilter(
    tracks: List<Track>,
    selected: String?,
    onSelect: (String?) -> Unit,
) {
    val formats = tracks
        .groupingBy { it.format }
        .eachCount()
        .entries
        .sortedByDescending { it.value }
    Row(
        modifier = Modifier
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

@Composable
private fun TrackList(
    tracks: List<Track>,
    onPlay: (Track) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (tracks.isEmpty()) {
        Text(
            text = "The server library is empty.",
            modifier = modifier.padding(vertical = 12.dp),
        )
        return
    }
    LazyColumn(
        modifier = modifier
            .fillMaxWidth()
            .testTag("track_list"),
    ) {
        items(tracks, key = { it.id }) { track ->
            ListItem(
                modifier = Modifier
                    .clickable { onPlay(track) }
                    .testTag("track_row"),
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
