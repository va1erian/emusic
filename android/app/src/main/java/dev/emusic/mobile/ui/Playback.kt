package dev.emusic.mobile.ui

import androidx.media3.common.MediaItem

/** A playable item for a direct stream (or local `file://`) URL. */
fun streamMediaItem(url: String): MediaItem = MediaItem.fromUri(url)
