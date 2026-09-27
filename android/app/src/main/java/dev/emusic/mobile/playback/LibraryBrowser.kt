package dev.emusic.mobile.playback

import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import uniffi.emusic_mobile.Track

/**
 * Builds an Android Auto / Media3 browse tree from a synced library.
 *
 * Ids are opaque and stable: `root`, `albums`, `artists`, `folders`, `tracks`,
 * `album:<n>`, `artist:<n>`, `folder:<n>` and `track:<id>`. The tree is built
 * once from the cached library and held in memory.
 *
 * Artwork is not attached yet (the art endpoint needs the bearer token, which
 * the Media3 bitmap loader cannot send); a content provider will add it later.
 */
class LibraryBrowser(
    tracks: List<Track>,
    private val streamUrl: (String) -> String,
) {
    private data class Node(val item: MediaItem, val childIds: List<String>)

    private val nodes = LinkedHashMap<String, Node>()
    private val root: MediaItem

    init {
        root = browsable(ROOT, "emusic")
        val albums = LinkedHashMap<String, MutableList<String>>()
        val artists = LinkedHashMap<String, MutableList<String>>()
        val folders = LinkedHashMap<String, MutableList<String>>()

        val trackIds = ArrayList<String>(tracks.size)
        for (track in tracks) {
            val id = "track:${track.id}"
            nodes[id] = Node(playable(id, track), emptyList())
            trackIds += id
            track.album?.takeIf { it.isNotBlank() }?.let {
                albums.getOrPut(it) { mutableListOf() } += id
            }
            track.artist?.takeIf { it.isNotBlank() }?.let {
                artists.getOrPut(it) { mutableListOf() } += id
            }
            folders.getOrPut(track.directory) { mutableListOf() } += id
        }

        val albumIds = albums.entries.mapIndexed { index, (album, ids) ->
            val id = "album:$index"
            nodes[id] = Node(browsable(id, album), ids)
            id
        }
        val artistIds = artists.entries.mapIndexed { index, (artist, ids) ->
            val id = "artist:$index"
            nodes[id] = Node(browsable(id, artist), ids)
            id
        }
        val folderIds = folders.entries.mapIndexed { index, (path, ids) ->
            val id = "folder:$index"
            nodes[id] = Node(browsable(id, path.ifEmpty { "/" }), ids)
            id
        }

        nodes[ALBUMS] = Node(browsable(ALBUMS, "Albums"), albumIds)
        nodes[ARTISTS] = Node(browsable(ARTISTS, "Artists"), artistIds)
        nodes[FOLDERS] = Node(browsable(FOLDERS, "Folders"), folderIds)
        nodes[TRACKS] = Node(browsable(TRACKS, "All tracks"), trackIds)
        nodes[ROOT] = Node(root, listOf(ALBUMS, ARTISTS, FOLDERS, TRACKS))
    }

    /** The library root item. */
    fun root(): MediaItem = root

    /** The item for `id`, or `null` when unknown. */
    fun item(id: String): MediaItem? = nodes[id]?.item

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

    private fun browsable(id: String, title: String): MediaItem =
        MediaItem.Builder()
            .setMediaId(id)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(title)
                    .setIsBrowsable(true)
                    .setIsPlayable(false)
                    .build(),
            )
            .build()

    private fun playable(id: String, track: Track): MediaItem =
        MediaItem.Builder()
            .setMediaId(id)
            .setUri(streamUrl(track.id))
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(titleOf(track))
                    .setArtist(track.artist)
                    .setAlbumTitle(track.album)
                    .setIsPlayable(true)
                    .setIsBrowsable(false)
                    .build(),
            )
            .build()

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
