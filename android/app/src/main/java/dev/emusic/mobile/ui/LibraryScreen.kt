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
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.emusic_mobile.MobileCore
import uniffi.emusic_mobile.Track
import java.io.File

/**
 * Connects to `url`, fetches the server's library, shows a summary plus the
 * track list, and plays both standard audio (Media3 streaming) and specialized
 * formats. Tracker modules (MOD/XM/S3M/IT) and SID render locally to a cached
 * FLAC; anything else specialized is deferred to the server's `/render`.
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
    var formatFilter by remember { mutableStateOf<String?>(null) }
    var browseMode by remember { mutableStateOf(BrowseMode.Tracks) }
    var query by remember { mutableStateOf("") }
    var albumFilter by remember { mutableStateOf<String?>(null) }
    var artistFilter by remember { mutableStateOf<String?>(null) }

    var player by remember { mutableStateOf<ExoPlayer?>(null) }
    var nowPlaying by remember { mutableStateOf<Track?>(null) }
    var isPlaying by remember { mutableStateOf(false) }
    var preparing by remember { mutableStateOf(false) }
    var playbackMessage by remember { mutableStateOf<String?>(null) }
    var queue by remember { mutableStateOf<List<Track>>(emptyList()) }
    var queueIndex by remember { mutableStateOf(-1) }
    var positionMs by remember { mutableStateOf(0L) }
    var durationMs by remember { mutableStateOf(0L) }

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

                override fun onPlayerError(error: androidx.media3.common.PlaybackException) {
                    playbackMessage = "playback failed: ${error.message}"
                    isPlaying = false
                }
            }
            active.addListener(listener)
            onDispose {
                active.removeListener(listener)
                active.release()
            }
        }
    }

    // Poll the transport while a track is loaded.
    LaunchedEffect(player, nowPlaying) {
        val active = player ?: return@LaunchedEffect
        while (nowPlaying != null) {
            positionMs = active.currentPosition.coerceAtLeast(0)
            durationMs = active.duration.coerceAtLeast(0)
            delay(500)
        }
    }

    /** Resolves and starts playback for one track. */
    fun startTrack(track: Track) {
        val client = core
        val active = player
        if (client == null || active == null) {
            playbackMessage = "not connected yet"
            return
        }
        playbackMessage = null
        nowPlaying = track

        if (!track.specialized) {
            active.setMediaItem(streamMediaItem(client.streamUrl(track.id)))
            active.prepare()
            active.play()
            return
        }

        if (client.canRender(track.format)) {
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

        // Anything else specialized: ask the server to render.
        active.setMediaItem(streamMediaItem(client.renderUrl(track.id, 0u)))
        active.prepare()
        active.play()
    }

    fun play(track: Track, list: List<Track>) {
        queue = list
        queueIndex = list.indexOfFirst { it.id == track.id }
        startTrack(track)
    }

    fun playAt(index: Int) {
        if (index in queue.indices) {
            queueIndex = index
            startTrack(queue[index])
        }
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
                    val searched = searchTracks(tracks, query)
                    val visibleTracks = searched
                        .filter { formatFilter == null || it.format == formatFilter }
                        .filter { albumFilter == null || it.album == albumFilter }
                        .filter { artistFilter == null || it.artist == artistFilter }
                    LibrarySummary(url = url, version = version, tracks = tracks)
                    HorizontalDivider()
                    SearchField(query = query, onQueryChange = { query = it })
                    BrowseTabs(mode = browseMode, onSelect = { browseMode = it })
                    val activeLabel = albumFilter ?: artistFilter
                    if (activeLabel != null) {
                        FilterBanner(label = activeLabel, onClear = {
                            albumFilter = null
                            artistFilter = null
                        })
                    }
                    when (browseMode) {
                        BrowseMode.Albums -> AlbumList(
                            albums = albumsOf(searched),
                            onSelect = { group ->
                                albumFilter = group.album
                                artistFilter = null
                                browseMode = BrowseMode.Tracks
                            },
                            modifier = Modifier.weight(1f),
                        )

                        BrowseMode.Artists -> ArtistList(
                            artists = artistsOf(searched),
                            onSelect = { group ->
                                artistFilter = group.artist
                                albumFilter = null
                                browseMode = BrowseMode.Tracks
                            },
                            modifier = Modifier.weight(1f),
                        )

                        BrowseMode.Tracks -> {
                            FormatFilter(
                                tracks = tracks,
                                selected = formatFilter,
                                onSelect = { formatFilter = it },
                            )
                            TrackList(
                                tracks = visibleTracks,
                                onPlay = { track -> play(track, visibleTracks) },
                                modifier = Modifier.weight(1f),
                            )
                        }
                    }
                    playbackMessage?.let { message ->
                        Text(
                            text = message,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(vertical = 8.dp),
                        )
                    }
                    nowPlaying?.let { track ->
                        PlayerBar(
                            track = track,
                            isPlaying = isPlaying,
                            preparing = preparing,
                            positionMs = positionMs,
                            durationMs = durationMs,
                            onSeek = { millis -> player?.seekTo(millis) },
                            onToggle = {
                                player?.let { active ->
                                    if (isPlaying) active.pause() else active.play()
                                }
                            },
                            onPrevious = {
                                if (queueIndex > 0) playAt(queueIndex - 1) else player?.seekTo(0)
                            },
                            onNext = { playAt(queueIndex + 1) },
                        )
                    }
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
                headlineContent = { Text(track.displayTitle()) },
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
