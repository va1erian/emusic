package dev.emusic.mobile.ui

import uniffi.emusic_mobile.Track

/** Tracks grouped under one album, in disc/track order. */
data class AlbumGroup(
    /** Stable key: the server's album id, else the lowercased album name. */
    val key: String,
    val album: String,
    val artist: String,
    val year: Int?,
    /** The album id to fetch cover art with, when the server has art. */
    val artId: String?,
    /** When the album's newest track was first seen (Unix seconds). */
    val addedAt: Long,
    val tracks: List<Track>,
) {
    /** Total running time in seconds, counting only tracks with a duration. */
    val durationSecs: Double get() = tracks.sumOf { it.durationSecs ?: 0.0 }
}

/** Tracks sharing a name (an artist or a genre). */
data class NamedGroup(val name: String, val tracks: List<Track>)

/** Tracks that share a directory. */
data class FolderGroup(val path: String, val tracks: List<Track>)

/** How the album grid is ordered. */
enum class AlbumSort(val label: String) {
    Name("Name"),
    Artist("Artist"),
    Year("Year"),
    Added("Recently added"),
}

/**
 * The synced library grouped every way the navigator can browse it. Built
 * once per track list or search (it groups thousands of tracks), never per
 * frame.
 */
class LibraryIndex(val tracks: List<Track>) {
    // Lazy, so a search only groups what the current view shows.

    /** Every track in folder order, the Music view's default order. */
    val music: List<Track> by lazy { sortTracksByDirectory(tracks) }

    val albums: List<AlbumGroup> by lazy { albumsOf(tracks) }
    val artists: List<NamedGroup> by lazy { namedGroups(tracks) { it.artist } }
    val genres: List<NamedGroup> by lazy { namedGroups(tracks) { it.genre } }
    val folders: List<FolderGroup> by lazy { foldersOf(tracks) }

    /** Newest first, as the server first saw them. */
    val recentlyAdded: List<Track> by lazy { tracks.sortedByDescending { it.addedAt } }

    private val albumsByKey by lazy { albums.associateBy { it.key } }
    private val tracksById by lazy { tracks.associateBy { it.id } }

    /** The tracks named by `ids`, in that order, skipping ids not in this index. */
    fun inOrder(ids: List<String>): List<Track> = ids.mapNotNull { tracksById[it] }

    fun album(key: String): AlbumGroup? = albumsByKey[key]

    fun artist(name: String): NamedGroup? = artists.firstOrNull { it.name == name }

    fun genre(name: String): NamedGroup? = genres.firstOrNull { it.name == name }

    /** Tracks in `folder`, optionally with its subfolders, in folder order. */
    fun folderTracks(folder: String, includeSubfolders: Boolean): List<Track> =
        music.filter { isInFolder(it.directory, folder, includeSubfolders) }
}

/** The name shown for an album the server identifies but no track names. */
const val UNKNOWN_ALBUM = "Unknown album"

/** The album grouping key: the server's album id, else the album name. */
fun albumKey(track: Track): String? =
    track.albumId?.takeIf { it.isNotBlank() }
        ?: track.album?.trim()?.takeIf { it.isNotEmpty() }?.let { "name:${it.lowercase()}" }

/** Groups tracks into albums, alphabetically by album name. */
fun albumsOf(tracks: List<Track>): List<AlbumGroup> =
    tracks
        .groupBy { albumKey(it) }
        .mapNotNull { (key, list) ->
            if (key == null) return@mapNotNull null
            AlbumGroup(
                key = key,
                // An album id can arrive without a name (tagged elsewhere).
                album = list.firstNotNullOfOrNull { it.album?.trim()?.takeIf(String::isNotEmpty) }
                    ?: UNKNOWN_ALBUM,
                artist = list.firstNotNullOfOrNull { it.albumArtistOrArtist() }.orEmpty(),
                year = list.firstNotNullOfOrNull { it.year },
                artId = list.firstNotNullOfOrNull { it.artId() },
                addedAt = list.maxOf { it.addedAt },
                tracks = sortAlbumTracks(list),
            )
        }
        .sortedBy { it.album.lowercase() }

/** Orders albums for the grid. */
fun sortAlbums(albums: List<AlbumGroup>, sort: AlbumSort): List<AlbumGroup> = when (sort) {
    AlbumSort.Name -> albums
    AlbumSort.Artist -> albums.sortedWith(
        compareBy({ it.artist.isEmpty() }, { it.artist.lowercase() }, { it.year ?: 0 }),
    )
    AlbumSort.Year -> albums.sortedWith(compareByDescending<AlbumGroup> { it.year ?: Int.MIN_VALUE })
    AlbumSort.Added -> albums.sortedByDescending { it.addedAt }
}

/** Album order: disc, then track number, then title. */
fun sortAlbumTracks(tracks: List<Track>): List<Track> =
    tracks.sortedWith(
        compareBy(
            { it.discNo ?: 0u },
            { it.trackNo ?: UInt.MAX_VALUE },
            { it.displayTitle().lowercase() },
        ),
    )

/** Groups tracks by a non-blank name, alphabetically. */
private fun namedGroups(tracks: List<Track>, name: (Track) -> String?): List<NamedGroup> =
    tracks
        .groupBy { name(it)?.trim()?.takeIf(String::isNotEmpty) }
        .mapNotNull { (key, list) -> key?.let { NamedGroup(it, sortTracksByDirectory(list)) } }
        .sortedBy { it.name.lowercase() }

/** Groups tracks by directory, alphabetically (root first). */
fun foldersOf(tracks: List<Track>): List<FolderGroup> =
    tracks
        .groupBy { it.directory }
        .map { (path, list) -> FolderGroup(path, list) }
        .sortedBy { it.path.lowercase() }

/** Whether `directory` is `folder` itself or nested under it. */
fun isInFolder(directory: String, folder: String, includeSubdirectories: Boolean): Boolean =
    when {
        !includeSubdirectories -> directory == folder
        // The root prefix is empty: everything is under it.
        folder.isEmpty() -> true
        else -> directory == folder || directory.startsWith("$folder/")
    }

/** Tracks ordered by directory, then track number, then title. */
fun sortTracksByDirectory(tracks: List<Track>): List<Track> =
    tracks.sortedWith(
        compareBy(
            { it.directory.lowercase() },
            { it.trackNo ?: 0u },
            { it.displayTitle().lowercase() },
        ),
    )

/** Free-text filter over title, file name, artist, album and genre. */
fun searchTracks(tracks: List<Track>, query: String): List<Track> {
    val needle = query.trim().lowercase()
    if (needle.isEmpty()) return tracks
    return tracks.filter { track ->
        sequenceOf(track.displayTitle(), track.filename, track.artist, track.album, track.genre)
            .any { it?.lowercase()?.contains(needle) == true }
    }
}
