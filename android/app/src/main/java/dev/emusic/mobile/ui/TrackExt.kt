package dev.emusic.mobile.ui

import uniffi.emusic_mobile.Track

/** Title to display: the tagged title, else the file name, else the id. */
fun Track.displayTitle(): String =
    title?.takeIf { it.isNotBlank() }
        ?: filename?.takeIf { it.isNotBlank() }
        ?: id
