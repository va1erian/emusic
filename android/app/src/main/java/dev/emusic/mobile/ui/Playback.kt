package dev.emusic.mobile.ui

import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import dev.emusic.mobile.playback.MediaContentProvider
import uniffi.emusic_mobile.Track

/**
 * A playable item for a direct stream (or local `file://`) URL, carrying the
 * track's metadata so the media notification and lock screen show its title,
 * artist, album and cover rather than a bare URL.
 */
fun streamMediaItem(url: String, track: Track): MediaItem {
    val metadata = MediaMetadata.Builder()
        .setTitle(track.displayTitle())
        .setArtist(track.artist)
        .setAlbumTitle(track.album)
        .setAlbumArtist(track.albumArtist)
        .setGenre(track.genre)
    track.trackNo?.let { metadata.setTrackNumber(it.toInt()) }
    track.year?.let { metadata.setRecordingYear(it) }
    track.artId()?.let { metadata.setArtworkUri(MediaContentProvider.artUri(it)) }
    return MediaItem.Builder()
        .setUri(url)
        .setMediaId(track.id)
        .setMediaMetadata(metadata.build())
        .build()
}
