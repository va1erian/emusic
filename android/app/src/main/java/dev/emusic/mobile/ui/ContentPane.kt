package dev.emusic.mobile.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier

/** View options that survive switching views (filters, sort, toggles). */
@Stable
class BrowseOptions {
    var format: String? by mutableStateOf(null)
    var albumSort by mutableStateOf(AlbumSort.Name)
    var includeSubfolders by mutableStateOf(true)
}

/** The top-bar title for a destination. */
fun Destination.title(library: LibraryIndex): String = when (this) {
    is Destination.Root -> label
    is Destination.Album -> library.album(key)?.album ?: "Album"
    is Destination.Artist -> name
    is Destination.Genre -> name
    is Destination.Folder -> path.substringAfterLast('/').ifEmpty { "/" }
}

/**
 * The right-hand pane: whichever view [LibraryNavigation.current] names.
 *
 * The search box filters every view, as on the desktop: root views list
 * [searched] (the library narrowed by the query), and drill-downs narrow their
 * album/artist/genre/folder from [library] by the same query.
 */
@Composable
fun ContentPane(
    nav: LibraryNavigation,
    library: LibraryIndex,
    searched: LibraryIndex,
    query: String,
    options: BrowseOptions,
    player: PlayerState,
    modifier: Modifier = Modifier,
) {
    val playingId = player.nowPlaying?.id
    fun empty(what: String) =
        if (query.isBlank()) "No $what yet." else "No $what match “${query.trim()}”."

    when (val destination = nav.current) {
        Destination.Root.Music -> Column(modifier) {
            val format = options.format
            val visible = remember(searched, format) {
                searched.music.filter { format == null || it.format == format }
            }
            FormatFilter(tracks = searched.music, selected = format, onSelect = { options.format = it })
            TrackList(
                tracks = visible,
                nowPlayingId = playingId,
                emptyText = empty("tracks"),
                onPlay = { player.play(it, visible) },
                modifier = Modifier.weight(1f),
            )
        }

        Destination.Root.Albums -> {
            val sort = options.albumSort
            val albums = remember(searched, sort) { sortAlbums(searched.albums, sort) }
            AlbumGrid(
                albums = albums,
                sort = sort,
                onSort = { options.albumSort = it },
                onOpen = { nav.push(Destination.Album(it.key)) },
                onShuffle = { player.shuffleAll(albums.flatMap { it.tracks }) },
                modifier = modifier,
            )
        }

        Destination.Root.Artists -> NamedList(
            groups = searched.artists,
            emptyText = empty("artists"),
            tag = "artist",
            onOpen = { nav.push(Destination.Artist(it.name)) },
            modifier = modifier,
        )

        Destination.Root.Genres -> NamedList(
            groups = searched.genres,
            emptyText = empty("genres"),
            tag = "genre",
            onOpen = { nav.push(Destination.Genre(it.name)) },
            modifier = modifier,
        )

        Destination.Root.Folders -> FolderList(
            folders = searched.folders,
            onOpen = { nav.push(Destination.Folder(it.path)) },
            modifier = modifier,
        )

        Destination.Root.Recent -> TrackList(
            tracks = searched.recentlyAdded,
            nowPlayingId = playingId,
            emptyText = empty("tracks"),
            onPlay = { player.play(it, searched.recentlyAdded) },
            modifier = modifier,
        )

        Destination.Root.NowPlaying -> NowPlayingView(
            track = player.nowPlaying,
            queue = player.queue,
            queueIndex = player.queueIndex,
            onPlayAt = player::playAt,
            onOpenAlbum = { track -> albumKey(track)?.let { nav.push(Destination.Album(it)) } },
            onOpenArtist = { name ->
                if (library.artist(name) != null) nav.push(Destination.Artist(name))
            },
            modifier = modifier,
        )

        is Destination.Album -> {
            val whole = library.album(destination.key)
            val album = remember(whole, query) {
                whole?.let { it.copy(tracks = searchTracks(it.tracks, query)) }
            }
            if (album == null) {
                EmptyState("This album is no longer in the library.", modifier)
            } else {
                AlbumDetail(
                    album = album,
                    nowPlayingId = playingId,
                    onPlay = player::play,
                    onShuffle = player::shuffleAll,
                    onOpenArtist = { name ->
                        if (library.artist(name) != null) nav.push(Destination.Artist(name))
                    },
                    modifier = modifier,
                )
            }
        }

        is Destination.Artist ->
            GroupPage(destination.name, library.artist(destination.name), query, nav, player, modifier)

        is Destination.Genre ->
            GroupPage(destination.name, library.genre(destination.name), query, nav, player, modifier)

        is Destination.Folder -> {
            val include = options.includeSubfolders
            val tracks = remember(library, destination.path, include, query) {
                searchTracks(library.folderTracks(destination.path, include), query)
            }
            FolderDetail(
                path = destination.path,
                tracks = tracks,
                includeSubfolders = include,
                onToggleSubfolders = { options.includeSubfolders = it },
                nowPlayingId = playingId,
                onPlay = { player.play(it, tracks) },
                modifier = modifier,
            )
        }
    }
}

/** An artist's or genre's page, or a note when it left the library. */
@Composable
private fun GroupPage(
    name: String,
    group: NamedGroup?,
    query: String,
    nav: LibraryNavigation,
    player: PlayerState,
    modifier: Modifier,
) {
    if (group == null) {
        EmptyState("“$name” is no longer in the library.", modifier)
        return
    }
    val tracks = remember(group, query) { searchTracks(group.tracks, query) }
    val albums = remember(tracks) { albumsOf(tracks) }
    GroupDetail(
        title = name,
        tracks = tracks,
        albums = albums,
        nowPlayingId = player.nowPlaying?.id,
        onOpenAlbum = { nav.push(Destination.Album(it.key)) },
        onPlay = player::play,
        onShuffle = player::shuffleAll,
        modifier = modifier,
    )
}
