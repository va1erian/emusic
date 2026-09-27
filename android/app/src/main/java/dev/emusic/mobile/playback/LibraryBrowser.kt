package dev.emusic.mobile.playback

import android.net.Uri
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import uniffi.emusic_mobile.Track

/** How a track should be played: a URI and whether it is playable at all. */
data class TrackSource(val uri: String, val playable: Boolean)

/**
 * Builds an Android Auto / Media3 browse tree from a synced library.
 *
 * Ids are opaque and stable: `root`, `albums`, `artists`, `folders`, `tracks`,
 * `album:<n>`, `artist:<n>`, `folder:<n>` and `track:<id>`. The tree is built
 * once from the cached library and held in memory.
 *
 * [source] resolves a track to its playable URI (a `/stream` URL, or a
 * `content://` render URI for specialized formats) and whether it can play.
 * Albums/artists/tracks with `has_art` get a `content://` artwork URI from
 * [MediaContentProvider].
 */
class LibraryBrowser(
    tracks: List<Track>,
    private val source: (Track) -> TrackSource,
) {
    private data class Node(val item: MediaItem, val childIds: List<String>)

    private val nodes = LinkedHashMap<String, Node>()
    private val root: MediaItem

    init {
        root = browsable(ROOT, "emusic", null)
        val albums = LinkedHashMap<String, MutableList<String>>()
        val artists = LinkedHashMap<String, MutableList<String>>()
        val folders = LinkedHashMap<String, MutableList<String>>()
        val albumArt = HashMap<String, String>()
        val artistArt = HashMap<String, String>()

        val trackIds = ArrayList<String>(tracks.size)
        for (track in tracks) {
            val id = "track:${track.id}"
            nodes[id] = Node(playable(id, track), emptyList())
            trackIds += id
            val artId = track.albumId?.takeIf { track.hasArt && it.isNotBlank() }
            track.album?.takeIf { it.isNotBlank() }?.let { album ->
                albums.getOrPut(album) { mutableListOf() } += id
                if (artId != null) albumArt.putIfAbsent(album, artId)
            }
            track.artist?.takeIf { it.isNotBlank() }?.let { artist ->
                artists.getOrPut(artist) { mutableListOf() } += id
                if (artId != null) artistArt.putIfAbsent(artist, artId)
            }
            folders.getOrPut(track.directory) { mutableListOf() } += id
        }

        val albumIds = albums.entries.map { (album, ids) ->
            val id = "album:$album"
            nodes[id] = Node(browsable(id, album, albumArt[album]), ids)
            id
        }
        val artistIds = artists.entries.map { (artist, ids) ->
            val id = "artist:$artist"
            nodes[id] = Node(browsable(id, artist, artistArt[artist]), ids)
            id
        }
        val folderIds = folders.entries.map { (path, ids) ->
            val id = "folder:$path"
            nodes[id] = Node(browsable(id, path.ifEmpty { "/" }, null), ids)
            id
        }

        nodes[ALBUMS] = Node(browsable(ALBUMS, "Albums", null), albumIds)
        nodes[ARTISTS] = Node(browsable(ARTISTS, "Artists", null), artistIds)
        nodes[FOLDERS] = Node(browsable(FOLDERS, "Folders", null), folderIds)
        nodes[TRACKS] = Node(browsable(TRACKS, "All tracks", null), trackIds)
        nodes[ROOT] = Node(root, listOf(ALBUMS, ARTISTS, FOLDERS, TRACKS))
    }

    /** The library root item. */
    fun root(): MediaItem = root

    /** The item for `id`, or `null` when unknown. */
    fun item(id: String): MediaItem? = nodes[id]?.item

    /**
     * The full track list rotated to start at `trackId`, used to give a car a
     * real queue when it picks a track. Empty when `trackId` is unknown.
     */
    fun queueFrom(trackId: String): List<MediaItem> {
        val all = nodes[TRACKS]?.childIds ?: return emptyList()
        val start = all.indexOf(trackId)
        if (start < 0) return emptyList()
        val rotated = all.subList(start, all.size) + all.subList(0, start)
        return rotated.mapNotNull { nodes[it]?.item }
    }

    /** One page of `parentId`'s children, or `null` when `parentId` is unknown. */
    fun children(parentId: String, page: Int, pageSize: Int): List<MediaItem>? {
        val childIds = nodes[parentId]?.childIds ?: return null
        val from = if (pageSize <= 0) 0 else (page * pageSize).coerceAtLeast(0)
        if (from >= childIds.size) return emptyList()
        val to = if (pageSize <= 0) childIds.size else (from + pageSize).coerceAtMost(childIds.size)
        return childIds.subList(from, to).mapNotNull { nodes[it]?.item }
    }

    /** Tracks whose title, artist or album contains `query` (case-insensitive). */
    fun search(query: String): List<MediaItem> {
        val needle = query.trim().lowercase()
        if (needle.isEmpty()) return emptyList()
        return nodes.values
            .asSequence()
            .filter { it.item.mediaId.startsWith("track:") }
            .filter { node ->
                val metadata = node.item.mediaMetadata
                listOf(metadata.title, metadata.artist, metadata.albumTitle)
                    .filterNotNull()
                    .any { it.toString().lowercase().contains(needle) }
            }
            .map { it.item }
            .take(MAX_SEARCH_RESULTS)
            .toList()
    }

    private fun browsable(id: String, title: String, artwork: String?): MediaItem {
        val metadata = MediaMetadata.Builder()
            .setTitle(title)
            .setIsBrowsable(true)
            .setIsPlayable(false)
        if (artwork != null) metadata.setArtworkUri(MediaContentProvider.artUri(artwork))
        return MediaItem.Builder()
            .setMediaId(id)
            .setMediaMetadata(metadata.build())
            .build()
    }

    private fun playable(id: String, track: Track): MediaItem {
        val resolved = source(track)
        val metadata = MediaMetadata.Builder()
            .setTitle(titleOf(track))
            .setArtist(track.artist)
            .setAlbumTitle(track.album)
            .setIsPlayable(resolved.playable)
            .setIsBrowsable(false)
        track.albumId?.takeIf { track.hasArt && it.isNotBlank() }?.let {
            metadata.setArtworkUri(MediaContentProvider.artUri(it))
        }
        val builder = MediaItem.Builder().setMediaId(id).setMediaMetadata(metadata.build())
        if (resolved.uri.isNotEmpty()) {
            // The session strips `localConfiguration` from library items, so the
            // playable URI must travel in the request metadata; the session's
            // `onAddMediaItems` resolves it back into a local item.
            builder.setUri(resolved.uri)
            builder.setRequestMetadata(
                MediaItem.RequestMetadata.Builder().setMediaUri(Uri.parse(resolved.uri)).build(),
            )
        }
        return builder.build()
    }

    /** Title to display: the tagged title, else the file name, else the id. */
    private fun titleOf(track: Track): String =
        track.title?.takeIf { it.isNotBlank() }
            ?: track.filename?.takeIf { it.isNotBlank() }
            ?: track.id

    companion object {
        const val ROOT = "root"
        const val ALBUMS = "albums"
        const val ARTISTS = "artists"
        const val FOLDERS = "folders"
        const val TRACKS = "tracks"
        private const val MAX_SEARCH_RESULTS = 100
    }
}
