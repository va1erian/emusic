package dev.emusic.mobile.playback

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.media3.common.AudioAttributes
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import dev.emusic.mobile.MainActivity
import uniffi.emusic_mobile.MobileCore

/**
 * Foreground media service that owns the player and its media session, so
 * playback keeps running when the UI is backgrounded or destroyed. The UI
 * drives it through a [androidx.media3.session.MediaController].
 */
class PlaybackService : MediaSessionService() {
    private var mediaSession: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        val player = ExoPlayer.Builder(this)
            .setAudioAttributes(AudioAttributes.DEFAULT, /* handleAudioFocus = */ true)
            .setHandleAudioBecomingNoisy(true)
            .setMediaSourceFactory(DefaultMediaSourceFactory(authenticatedSourceFactory(this)))
            .build()
        mediaSession = MediaSession.Builder(this, player)
            .setSessionActivity(openAppIntent())
            .build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? =
        mediaSession

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Keep playing when the task is swiped away, but do not linger once
        // playback has stopped.
        val player = mediaSession?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        mediaSession?.run {
            player.release()
            release()
        }
        mediaSession = null
        super.onDestroy()
    }

    private fun openAppIntent(): PendingIntent =
        PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
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
