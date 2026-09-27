package dev.emusic.mobile.ui

import android.content.Context
import androidx.media3.common.MediaItem
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory

/**
 * Builds an [ExoPlayer] that authenticates every HTTP request with the server's
 * bearer token and also plays local `file://` renditions.
 *
 * [bearerToken] is called for each new data source, so an expired token is
 * refreshed (the client refreshes when close to expiry) instead of being baked
 * in once at construction. [DefaultHttpDataSource] forwards HTTP `Range` on its
 * own, so seeking works against `/tracks/{id}/stream` and `/render`; the
 * surrounding [DefaultDataSource] routes `file://` URIs (locally rendered
 * modules, downloads) to the filesystem.
 */
fun createAuthenticatedPlayer(context: Context, bearerToken: () -> String?): ExoPlayer {
    val sourceFactory = object : DataSource.Factory {
        override fun createDataSource(): DataSource {
            val token = bearerToken()
            val httpFactory = DefaultHttpDataSource.Factory()
                .setDefaultRequestProperties(mapOf("Authorization" to "Bearer ${token.orEmpty()}"))
            return DefaultDataSource.Factory(context, httpFactory).createDataSource()
        }
    }
    return ExoPlayer.Builder(context)
        .setMediaSourceFactory(DefaultMediaSourceFactory(sourceFactory))
        .build()
}

/** A playable item for a direct stream URL. */
fun streamMediaItem(url: String): MediaItem = MediaItem.fromUri(url)
