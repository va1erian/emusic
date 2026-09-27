package dev.emusic.mobile.playback

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.media3.common.AudioAttributes
import androidx.media3.common.MediaItem
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.LibraryResult
import androidx.media3.session.MediaLibraryService
import androidx.media3.session.MediaSession
import androidx.media3.session.SessionError
import com.google.common.collect.ImmutableList
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import dev.emusic.mobile.MainActivity
import uniffi.emusic_mobile.MobileCore
import java.io.File

/**
 * Foreground media service that owns the player and a **media library** session,
 * so playback keeps running when the UI is backgrounded or destroyed, and the
 * library can be browsed from Android Auto and other media browsers.
 *
 * The UI drives it through a `MediaController`; Android Auto browses it through
 * a `MediaBrowser`. The browse tree comes from the cached library
 * ([LibraryBrowser]), so it works offline.
 */
class PlaybackService : MediaLibraryService() {
    private var librarySession: MediaLibraryService.MediaLibrarySession? = null
    private var browser: LibraryBrowser? = null
    private var browserStamp: Long = Long.MIN_VALUE

    override fun onCreate() {
        super.onCreate()
        val player = ExoPlayer.Builder(this)
            .setAudioAttributes(AudioAttributes.DEFAULT, /* handleAudioFocus = */ true)
            .setHandleAudioBecomingNoisy(true)
            .setMediaSourceFactory(DefaultMediaSourceFactory(authenticatedSourceFactory(this)))
            .build()
        librarySession = MediaLibraryService.MediaLibrarySession.Builder(this, player, LibraryCallback())
            .setSessionActivity(openAppIntent())
            .build()
    }

    override fun onGetSession(
        controllerInfo: MediaSession.ControllerInfo,
    ): MediaLibraryService.MediaLibrarySession? = librarySession

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Keep playing when the task is swiped away, but do not linger once
        // playback has stopped.
        val player = librarySession?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        librarySession?.run {
            player.release()
            release()
        }
        librarySession = null
        super.onDestroy()
    }

    /**
     * The browse tree, rebuilt whenever the cached library file changes.
     *
     * A tree built without an active server or with no tracks is returned but
     * not cached: it is transient (the app may not be paired/synced yet) and
     * caching it would leave Android Auto stuck on an empty library.
     */
    private fun library(): LibraryBrowser {
        val active = ActiveServer.get(this)
        val snapshot = File(ActiveServer.dataDir(this), "library.json")
        val stamp = if (active == null) Long.MIN_VALUE else snapshot.lastModified()
        browser?.let { if (stamp == browserStamp) return it }

        val core = active?.let { runCatching { MobileCore(it, ActiveServer.dataDir(this)) }.getOrNull() }
        val tracks = core?.let { runCatching { it.cachedLibrary().tracks }.getOrNull() }.orEmpty()
        val built = LibraryBrowser(tracks) { track ->
            when {
                core == null -> TrackSource("", false)
                !track.specialized -> TrackSource(core.streamUrl(track.id), true)
                core.canRender(track.format) -> TrackSource(
                    MediaContentProvider.renderUri(track.id, track.format).toString(),
                    true,
                )

                else -> TrackSource("", false)
            }
        }
        if (core != null && tracks.isNotEmpty()) {
            browser = built
            browserStamp = stamp
        }
        return built
    }

    private fun openAppIntent(): PendingIntent =
        PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    /** Serves the browse tree to Android Auto and other media browsers. */
    private inner class LibraryCallback : MediaLibraryService.MediaLibrarySession.Callback {
        /**
         * Resolves a browser-picked item into a playable one, and expands a
         * picked track into its queue.
         *
         * The session strips `localConfiguration`, so the playable URI travels
         * in `requestMetadata.mediaUri` and is restored here. Expanding the
         * whole track list (starting at the picked track) gives the car a real
         * queue, so next/previous/repeat/shuffle work there too. The phone UI's
         * items have no `track:` id and are passed through untouched.
         */
        override fun onAddMediaItems(
            mediaSession: MediaSession,
            controller: MediaSession.ControllerInfo,
            mediaItems: List<MediaItem>,
        ): ListenableFuture<List<MediaItem>> {
            val resolved = ArrayList<MediaItem>(mediaItems.size)
            for (item in mediaItems) {
                val queue = if (item.mediaId.startsWith("track:")) {
                    library().queueFrom(item.mediaId)
                } else {
                    emptyList()
                }
                for (candidate in queue.ifEmpty { listOf(item) }) {
                    val mediaUri = candidate.requestMetadata.mediaUri
                    resolved += if (candidate.localConfiguration == null && mediaUri != null) {
                        candidate.buildUpon().setUri(mediaUri).build()
                    } else {
                        candidate
                    }
                }
            }
            return Futures.immediateFuture(resolved)
        }

        override fun onGetLibraryRoot(
            session: MediaLibraryService.MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            params: MediaLibraryService.LibraryParams?,
        ): ListenableFuture<LibraryResult<MediaItem>> =
            Futures.immediateFuture(LibraryResult.ofItem(library().root(), params))

        override fun onGetItem(
            session: MediaLibraryService.MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            mediaId: String,
        ): ListenableFuture<LibraryResult<MediaItem>> {
            val item = library().item(mediaId)
                ?: return Futures.immediateFuture(LibraryResult.ofError(SessionError.ERROR_BAD_VALUE))
            return Futures.immediateFuture(LibraryResult.ofItem(item, null))
        }

        override fun onGetChildren(
            session: MediaLibraryService.MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            parentId: String,
            page: Int,
            pageSize: Int,
            params: MediaLibraryService.LibraryParams?,
        ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> {
            val children = library().children(parentId, page, pageSize)
                ?: return Futures.immediateFuture(LibraryResult.ofError(SessionError.ERROR_BAD_VALUE))
            return Futures.immediateFuture(LibraryResult.ofItemList(children, params))
        }

        override fun onSubscribe(
            session: MediaLibraryService.MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            parentId: String,
            params: MediaLibraryService.LibraryParams?,
        ): ListenableFuture<LibraryResult<Void>> =
            Futures.immediateFuture(LibraryResult.ofVoid(params))

        override fun onUnsubscribe(
            session: MediaLibraryService.MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            parentId: String,
        ): ListenableFuture<LibraryResult<Void>> =
            Futures.immediateFuture(LibraryResult.ofVoid())

        override fun onSearch(
            session: MediaLibraryService.MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            query: String,
            params: MediaLibraryService.LibraryParams?,
        ): ListenableFuture<LibraryResult<Void>> {
            val results = library().search(query)
            session.notifySearchResultChanged(browser, query, results.size, params)
            return Futures.immediateFuture(LibraryResult.ofVoid(params))
        }

        override fun onGetSearchResult(
            session: MediaLibraryService.MediaLibrarySession,
            browser: MediaSession.ControllerInfo,
            query: String,
            page: Int,
            pageSize: Int,
            params: MediaLibraryService.LibraryParams?,
        ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> =
            Futures.immediateFuture(LibraryResult.ofItemList(library().search(query), params))
    }
}

/**
 * A data source factory that authenticates each HTTP request with the active
 * server's bearer token (resolved per request, so it refreshes) and also plays
 * local `file://` renditions. [DefaultHttpDataSource] forwards HTTP `Range`, so
 * seeking works against `/stream` and `/render`.
 */
fun authenticatedSourceFactory(context: Context): DataSource.Factory = object : DataSource.Factory {
    override fun createDataSource(): DataSource {
        val url = ActiveServer.get(context)
        val token = url?.let { server ->
            runCatching { MobileCore(server, ActiveServer.dataDir(context)).bearerToken() }
                .getOrNull()
        }
        val http = DefaultHttpDataSource.Factory()
            .setDefaultRequestProperties(mapOf("Authorization" to "Bearer ${token.orEmpty()}"))
        return DefaultDataSource.Factory(context, http).createDataSource()
    }
}
