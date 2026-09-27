package dev.emusic.mobile.ui

import android.content.Context
import androidx.media3.common.MediaItem
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory

/**
 * Builds an [ExoPlayer] that authenticates every request with the server's
 * bearer token.
 *
 * `DefaultHttpDataSource` forwards HTTP `Range` on its own, so seeking works
 * against `/tracks/{id}/stream` without a custom data source.
 */
fun createAuthenticatedPlayer(context: Context, bearerToken: String): ExoPlayer {
    val dataSourceFactory = DefaultHttpDataSource.Factory()
        .setDefaultRequestProperties(mapOf("Authorization" to "Bearer $bearerToken"))
    return ExoPlayer.Builder(context)
        .setMediaSourceFactory(DefaultMediaSourceFactory(dataSourceFactory))
        .build()
}

/** A playable item for a direct stream URL. */
fun streamMediaItem(url: String): MediaItem = MediaItem.fromUri(url)
