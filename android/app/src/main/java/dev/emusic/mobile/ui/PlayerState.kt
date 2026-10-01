package dev.emusic.mobile.ui

import android.content.ComponentName
import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors
import dev.emusic.mobile.playback.ActiveServer
import dev.emusic.mobile.playback.PlaybackService
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.emusic_mobile.MobileCore
import uniffi.emusic_mobile.Track
import java.io.File

/** How the queue repeats. */
enum class RepeatMode { Off, All, One }

/**
 * The app-side queue and transport over the playback service's [MediaController].
 *
 * Standard audio streams straight from the server. Tracker modules and SID
 * render locally to a cached FLAC; anything else specialized is deferred to
 * the server's `/render`.
 */
@Stable
class PlayerState(
    private val context: Context,
    private val core: MobileCore?,
    private val url: String,
    private val scope: CoroutineScope,
    private val onNeedsNotificationPermission: () -> Unit,
) {
    var controller: MediaController? by mutableStateOf(null)
        internal set
    var nowPlaying: Track? by mutableStateOf(null)
        private set
    var isPlaying by mutableStateOf(false)
        internal set
    var preparing by mutableStateOf(false)
        private set
    var message: String? by mutableStateOf(null)
        internal set
    var queue: List<Track> by mutableStateOf(emptyList())
        private set
    var queueIndex by mutableStateOf(-1)
        private set
    var repeatMode by mutableStateOf(RepeatMode.Off)
        private set
    var shuffle by mutableStateOf(false)
        private set

    /** Read only by the transport, so the 500 ms poll recomposes nothing else. */
    var positionMs by mutableLongStateOf(0L)
        internal set
    var durationMs by mutableLongStateOf(0L)
        internal set

    private var renderJob: Job? = null

    /** Plays `track` with `list` as the queue. */
    fun play(track: Track, list: List<Track>) {
        queue = list
        queueIndex = list.indexOfFirst { it.id == track.id }
        startTrack(track)
    }

    /** Plays `list` from a random track with shuffle on. */
    fun shuffleAll(list: List<Track>) {
        if (list.isEmpty()) return
        shuffle = true
        play(list.random(), list)
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
            // Never "advance" to the track that is already playing.
            shuffle -> playAt(
                queue.indices.filter { it != queueIndex }.ifEmpty { queue.indices.toList() }.random(),
            )
            queueIndex + 1 < queue.size -> playAt(queueIndex + 1)
            repeatMode == RepeatMode.All -> playAt(0)
            // End of the queue: stop, and let onIsPlayingChanged report it,
            // so the button never disagrees with the player.
            else -> controller?.pause()
        }
    }

    fun previous() {
        if (queueIndex > 0) playAt(queueIndex - 1) else controller?.seekTo(0)
    }

    fun toggle() {
        controller?.let { active -> if (isPlaying) active.pause() else active.play() }
    }

    fun seek(millis: Long) {
        controller?.seekTo(millis)
    }

    fun cycleRepeat() {
        repeatMode = when (repeatMode) {
            RepeatMode.Off -> RepeatMode.All
            RepeatMode.All -> RepeatMode.One
            RepeatMode.One -> RepeatMode.Off
        }
    }

    fun toggleShuffle() {
        shuffle = !shuffle
    }

    /** Reacts to a track finishing according to the repeat mode. */
    internal fun onEnded() {
        if (repeatMode == RepeatMode.One && queueIndex in queue.indices) {
            playAt(queueIndex)
        } else {
            advance()
        }
    }

    /** Resolves and starts playback for one track. */
    private fun startTrack(track: Track) {
        // A newer pick supersedes any render still in flight.
        renderJob?.cancel()
        renderJob = null
        preparing = false

        val client = core
        val active = controller
        if (client == null || active == null) {
            message = "not connected yet"
            return
        }
        message = null
        nowPlaying = track
        ActiveServer.set(context, url)
        onNeedsNotificationPermission()

        if (!track.specialized) {
            active.start(streamMediaItem(client.streamUrl(track.id), track))
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
                        controller?.start(streamMediaItem(File(path).toURI().toString(), track))
                    }
                    .onFailure { message = "render failed: ${it.message}" }
            }
            return
        }

        // Anything else specialized: ask the server to render.
        active.start(streamMediaItem(client.renderUrl(track.id, 0u), track))
    }

    private fun MediaController.start(item: androidx.media3.common.MediaItem) {
        setMediaItem(item)
        prepare()
        play()
    }
}

/**
 * Connects a [PlayerState] to the playback service. The service owns the
 * player, so playback survives this screen (and the task) being destroyed.
 */
@Composable
fun rememberPlayerState(
    core: MobileCore?,
    url: String,
    onNeedsNotificationPermission: () -> Unit,
): PlayerState {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val state = remember(core, url) {
        PlayerState(context, core, url, scope, onNeedsNotificationPermission)
    }

    DisposableEffect(state) {
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        val future = MediaController.Builder(context, token).buildAsync()
        future.addListener({ state.controller = future.get() }, MoreExecutors.directExecutor())
        onDispose {
            state.controller = null
            MediaController.releaseFuture(future)
        }
    }

    val controller = state.controller
    DisposableEffect(controller) {
        val listener = object : Player.Listener {
            override fun onIsPlayingChanged(playing: Boolean) {
                state.isPlaying = playing
            }

            override fun onPlaybackStateChanged(playbackState: Int) {
                // Only follow up our own item: Android Auto drives the same
                // player and must not have the phone's queue started on it.
                if (playbackState == Player.STATE_ENDED &&
                    controller?.currentMediaItem?.mediaId == state.nowPlaying?.id
                ) {
                    state.onEnded()
                }
            }

            override fun onPlayerError(error: PlaybackException) {
                state.message = "playback failed: ${error.message}"
                state.isPlaying = false
            }
        }
        controller?.addListener(listener)
        onDispose { controller?.removeListener(listener) }
    }

    // Poll the transport while a track is loaded.
    LaunchedEffect(controller, state.nowPlaying) {
        val active = controller ?: return@LaunchedEffect
        while (state.nowPlaying != null) {
            state.positionMs = active.currentPosition.coerceAtLeast(0)
            state.durationMs = active.duration.coerceAtLeast(0)
            delay(500)
        }
    }
    return state
}
