package dev.emusic.mobile.ui

import uniffi.emusic_mobile.Track

/** The title shown for a track with neither a title tag nor a file name. */
const val UNTITLED = "Untitled"

/**
 * Title to display: the tagged title, else the file name without its
 * extension, else [UNTITLED]. Never the opaque id, which is a 64-digit hash.
 */
fun Track.displayTitle(): String =
    title?.trim()?.takeIf { it.isNotEmpty() }
        ?: filename?.let(::fileStem)?.takeIf { it.isNotEmpty() }
        ?: UNTITLED

/** `name` without its final extension; a leading dot is part of the name. */
fun fileStem(name: String): String {
    val trimmed = name.trim()
    val dot = trimmed.lastIndexOf('.')
    return if (dot > 0) trimmed.substring(0, dot).trimEnd() else trimmed
}

/** "Artist · Album", skipping blanks; the format label when both are missing. */
fun Track.subtitle(): String =
    listOfNotNull(artist, album)
        .filter { it.isNotBlank() }
        .joinToString(" · ")
        .ifEmpty { format }

/** The album artist when tagged, else the track artist. */
fun Track.albumArtistOrArtist(): String? =
    albumArtist?.takeIf { it.isNotBlank() } ?: artist?.takeIf { it.isNotBlank() }

/** The album id to fetch artwork with, when the server has art for it. */
fun Track.artId(): String? = albumId?.takeIf { hasArt && it.isNotBlank() }
