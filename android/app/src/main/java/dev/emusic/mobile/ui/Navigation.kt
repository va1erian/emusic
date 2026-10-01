package dev.emusic.mobile.ui

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/** A place in the library: a navigator root or a drill-down under one. */
sealed interface Destination {
    /** A view listed in the navigator, mirroring the desktop's sidebar. */
    enum class Root(val label: String) : Destination {
        Music("Music"),
        Albums("Albums"),
        Artists("Artists"),
        Genres("Genres"),
        Folders("Folders"),
        Recent("Recently added"),
        NowPlaying("Now playing"),
    }

    /** One album's tracks. */
    data class Album(val key: String) : Destination

    /** One artist's albums and tracks. */
    data class Artist(val name: String) : Destination

    /** One genre's albums and tracks. */
    data class Genre(val name: String) : Destination

    /** One folder's tracks. */
    data class Folder(val path: String) : Destination
}

/** The navigator's sections, in order, like the desktop's LIBRARY/ACTIVITY. */
val NAVIGATOR_SECTIONS: List<Pair<String, List<Destination.Root>>> = listOf(
    "LIBRARY" to listOf(
        Destination.Root.Music,
        Destination.Root.Albums,
        Destination.Root.Artists,
        Destination.Root.Genres,
        Destination.Root.Folders,
    ),
    "ACTIVITY" to listOf(Destination.Root.Recent, Destination.Root.NowPlaying),
)

/**
 * The back stack: a navigator root with drill-downs pushed on top. Selecting
 * a root in the navigator replaces the whole stack, as on the desktop.
 */
@Stable
class LibraryNavigation(start: Destination.Root = Destination.Root.Music) {
    var stack: List<Destination> by mutableStateOf(listOf(start))
        private set

    /** The view shown in the content pane. */
    val current: Destination get() = stack.last()

    /** The navigator row to highlight. */
    val root: Destination.Root get() = stack.first() as Destination.Root

    val canGoBack: Boolean get() = stack.size > 1

    fun select(root: Destination.Root) {
        stack = listOf(root)
    }

    fun push(destination: Destination) {
        if (destination != current) stack = stack + destination
    }

    fun pop() {
        if (canGoBack) stack = stack.dropLast(1)
    }
}
