package dev.emusic.mobile.ui

import android.Manifest
import android.content.ComponentName
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
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
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.ContextCompat
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.composables.icons.lucide.Info
import com.composables.icons.lucide.Lucide
import com.google.common.util.concurrent.MoreExecutors
import dev.emusic.mobile.playback.ActiveServer
import dev.emusic.mobile.playback.PlaybackService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
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
    val notificationPermission =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
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
    var folderFilter by remember { mutableStateOf<String?>(null) }
    var includeSubdirs by remember { mutableStateOf(true) }
    var showInfo by remember { mutableStateOf(false) }
    var renderJob by remember { mutableStateOf<Job?>(null) }
    var repeatMode by remember { mutableStateOf(RepeatMode.Off) }
    var shuffle by remember { mutableStateOf(false) }

    var controller by remember { mutableStateOf<MediaController?>(null) }
    var nowPlaying by remember { mutableStateOf<Track?>(null) }
    var isPlaying by remember { mutableStateOf(false) }
    var preparing by remember { mutableStateOf(false) }
    var playbackMessage by remember { mutableStateOf<String?>(null) }
    var queue by remember { mutableStateOf<List<Track>>(emptyList()) }
    var queueIndex by remember { mutableStateOf(-1) }
    var positionMs by remember { mutableStateOf(0L) }
    var durationMs by remember { mutableStateOf(0L) }

    LaunchedEffect(nonce) {
        error = null
        val client = core
        if (client == null) {
            error = "invalid server"
            loading = false
            return@LaunchedEffect
        }

        // Show the cached library immediately, then apply the server's delta.
        loading = tracks.isEmpty()
        withContext(Dispatchers.IO) { runCatching { client.cachedLibrary() } }
            .onSuccess { cached ->
                if (cached.tracks.isNotEmpty()) {
                    version = cached.version
                    tracks = cached.tracks
                    loading = false
                    // Let the playback service browse this server.
                    ActiveServer.set(context, url)
                }
            }

        withContext(Dispatchers.IO) { runCatching { client.refreshLibrary() } }
            .onSuccess { refreshed ->
                version = refreshed.version
                tracks = refreshed.tracks
                error = null
                ActiveServer.set(context, url)
            }
            .onFailure { failure ->
                // Keep the cache when the refresh fails (for example offline).
                if (tracks.isEmpty()) error = failure.message ?: failure.toString()
            }
        loading = false
    }

    // Connect to the playback service; it owns the player, so playback
    // survives this screen (and the task) being destroyed.
    DisposableEffect(context) {
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        val future = MediaController.Builder(context, token).buildAsync()
        future.addListener({ controller = future.get() }, MoreExecutors.directExecutor())
        onDispose {
            controller = null
            MediaController.releaseFuture(future)
        }
    }

    // Poll the transport while a track is loaded.
    LaunchedEffect(controller, nowPlaying) {
        val active = controller ?: return@LaunchedEffect
        while (nowPlaying != null) {
            positionMs = active.currentPosition.coerceAtLeast(0)
            durationMs = active.duration.coerceAtLeast(0)
            delay(500)
        }
    }

    /** Asks for notification permission (API 33+) so playback can show its controls. */
    fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    /** Resolves and starts playback for one track. */
    fun startTrack(track: Track) {
        // A newer pick supersedes any render still in flight.
        renderJob?.cancel()
        renderJob = null
        preparing = false

        val client = core
        val active = controller
        if (client == null || active == null) {
            playbackMessage = "not connected yet"
            return
        }
        playbackMessage = null
        nowPlaying = track
        ActiveServer.set(context, url)
        requestNotificationPermission()

        if (!track.specialized) {
            active.setMediaItem(streamMediaItem(client.streamUrl(track.id)))
            active.prepare()
            active.play()
            return
        }

        if (client.canRender(track.format)) {
            preparing = true
            renderJob = scope.launch {
                val result = withContext(Dispatchers.IO) {
                    runCatching { client.renderToFile(track.id, track.format) }
                }
                // The user may have picked another track while this rendered.
                if (nowPlaying?.id != track.id) return@launch
                preparing = false
                result
                    .onSuccess { path ->
                        controller?.run {
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

    /** Moves to the next track in the queue (used by the Next button). */
    fun advance() {
        if (queue.isEmpty()) return
        when {
            shuffle -> playAt(queue.indices.random())
            queueIndex + 1 < queue.size -> playAt(queueIndex + 1)
            repeatMode == RepeatMode.All -> playAt(0)
            else -> isPlaying = false
        }
    }

    /** Reacts to a track finishing according to the repeat mode. */
    fun onEnded() {
        if (repeatMode == RepeatMode.One && queueIndex in queue.indices) {
            playAt(queueIndex)
        } else {
            advance()
        }
    }

    fun previous() {
        if (queueIndex > 0) playAt(queueIndex - 1) else controller?.seekTo(0)
    }

    DisposableEffect(controller) {
        val active = controller
        if (active == null) {
            onDispose { }
        } else {
            val listener = object : Player.Listener {
                override fun onIsPlayingChanged(playing: Boolean) {
                    isPlaying = playing
                }

                override fun onPlaybackStateChanged(state: Int) {
                    if (state == Player.STATE_ENDED) onEnded()
                }

                override fun onPlayerError(error: androidx.media3.common.PlaybackException) {
                    playbackMessage = "playback failed: ${error.message}"
                    isPlaying = false
                }
            }
            active.addListener(listener)
            onDispose { active.removeListener(listener) }
        }
    }

    if (showInfo) {
        // A full-screen dialog overlay, so the library (and its player) stays
        // composed underneath and playback continues while info is open.
        Dialog(
            onDismissRequest = { showInfo = false },
            properties = DialogProperties(usePlatformDefaultWidth = false),
        ) {
            Surface(modifier = Modifier.fillMaxSize()) {
                InfoScreen(
                    url = url,
                    version = version,
                    tracks = tracks,
                    onBack = { showInfo = false },
                )
            }
        }
    }

    Scaffold(
        modifier = Modifier.semantics { testTagsAsResourceId = true },
        topBar = {
            TopAppBar(
                title = { Text("Library") },
                navigationIcon = { TextButton(onClick = onBack) { Text("Back") } },
                actions = {
                    IconButton(
                        onClick = { showInfo = true },
                        modifier = Modifier.testTag("info_action"),
                    ) {
                        Icon(Lucide.Info, contentDescription = "Server info")
                    }
                    TextButton(onClick = { nonce++ }) { Text("Refresh") }
                },
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
                    val folder = folderFilter
                    val searched = searchTracks(tracks, query)
                    val visibleTracks = sortTracksByDirectory(
                        searched
                            .filter { formatFilter == null || it.format == formatFilter }
                            .filter { albumFilter == null || it.album == albumFilter }
                            .filter { artistFilter == null || it.artist == artistFilter }
                            .filter {
                                folder == null || isInFolder(it.directory, folder, includeSubdirs)
                            },
                    )
                    SearchField(query = query, onQueryChange = { query = it })
                    BrowseTabs(mode = browseMode, onSelect = { browseMode = it })
                    val activeLabel = albumFilter ?: artistFilter
                    if (activeLabel != null) {
                        FilterBanner(label = activeLabel, onClear = {
                            albumFilter = null
                            artistFilter = null
                        })
                    }
                    if (folder != null) {
                        FolderFilterBanner(
                            folder = folder,
                            includeSubdirectories = includeSubdirs,
                            onToggleSubdirectories = { includeSubdirs = it },
                            onClear = { folderFilter = null },
                        )
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

                        BrowseMode.Folders -> FolderList(
                            folders = foldersOf(searched),
                            onSelect = { group ->
                                folderFilter = group.path
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
                            repeatMode = repeatMode,
                            shuffle = shuffle,
                            onSeek = { millis -> controller?.seekTo(millis) },
                            onToggle = {
                                controller?.let { active ->
                                    if (isPlaying) active.pause() else active.play()
                                }
                            },
                            onPrevious = { previous() },
                            onNext = { advance() },
                            onCycleRepeat = {
                                repeatMode = when (repeatMode) {
                                    RepeatMode.Off -> RepeatMode.All
                                    RepeatMode.All -> RepeatMode.One
                                    RepeatMode.One -> RepeatMode.Off
                                }
                            },
                            onToggleShuffle = { shuffle = !shuffle },
                        )
                    }
                }
            }
        }
    }
}

