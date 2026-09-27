package dev.emusic.mobile.ui

import android.content.Context
import androidx.media3.common.MediaItem
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory

/**
 * Builds an [ExoPlayer] that authenticates every HTTP request with the server's
 * bearer token and also plays local `file://` renditions.
 *
 * `DefaultHttpDataSource` forwards HTTP `Range` on its own, so seeking works
 * against `/tracks/{id}/stream` and `/render` without a custom data source; the
 * surrounding [DefaultDataSource] routes `file://` URIs (locally rendered
 * modules, downloads) to the filesystem instead.
 */
fun createAuthenticatedPlayer(context: Context, bearerToken: String): ExoPlayer {
    val httpFactory = DefaultHttpDataSource.Factory()
        .setDefaultRequestProperties(mapOf("Authorization" to "Bearer $bearerToken"))
    val dataSourceFactory = DefaultDataSource.Factory(context, httpFactory)
    return ExoPlayer.Builder(context)
        .setMediaSourceFactory(DefaultMediaSourceFactory(dataSourceFactory))
        .build()
}

/** A playable item for a direct stream URL. */
fun streamMediaItem(url: String): MediaItem = MediaItem.fromUri(url)
