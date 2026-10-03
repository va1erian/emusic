package dev.emusic.mobile.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.emusic_mobile.Track

class LibraryIndexTest {
    private fun track(
        id: String,
        title: String? = null,
        filename: String? = null,
        artist: String? = null,
        albumArtist: String? = null,
        album: String? = null,
        albumId: String? = null,
        genre: String? = null,
        trackNo: UInt? = null,
        discNo: UInt? = null,
        directory: String = "",
        hasArt: Boolean = false,
        addedAt: Long = 0,
    ) = Track(
        id = id, filename = filename, directory = directory, format = "mp3", kind = "stream",
        specialized = false, title = title, artist = artist, albumArtist = albumArtist,
        album = album, albumId = albumId, genre = genre, year = null, trackNo = trackNo,
        discNo = discNo, durationSecs = null, subtunes = 1u, channels = null, fileSize = 0u,
        hasArt = hasArt, syncVersion = 0, addedAt = addedAt,
    )

    @Test
    fun titleFallsBackToTheFileStemNeverTheId() {
        val hash = "0e0a2e72050fc119033f89f75829941e4b6fac1c1b4072f8930a408dd53b9cac"
        assertEquals("AMAZONS -KISS IN THE DARK", track(hash, filename = "AMAZONS -KISS IN THE DARK.mp3").displayTitle())
        assertEquals("v1.2 mix", track(hash, filename = "v1.2 mix.flac").displayTitle())
        assertEquals(".hidden", track(hash, filename = ".hidden").displayTitle())
        assertEquals(UNTITLED, track(hash).displayTitle())
        assertEquals(UNTITLED, track(hash, title = "  ").displayTitle())
        assertEquals("Real", track(hash, title = " Real ", filename = "x.mp3").displayTitle())
    }

    @Test
    fun albumsGroupByAlbumIdAndOrderTracksByDiscThenNumber() {
        val tracks = listOf(
            track("a2", title = "Two", album = "Hits", albumId = "alb1", artist = "A", trackNo = 2u, discNo = 1u),
            track("b1", title = "Other", album = "Hits", albumId = "alb2", artist = "B", trackNo = 1u),
            track("a3", title = "Three", album = "Hits", albumId = "alb1", trackNo = 1u, discNo = 2u, hasArt = true),
            track("a1", title = "One", album = "Hits", albumId = "alb1", trackNo = 1u, discNo = 1u),
            track("loose", title = "No album"),
        )
        val albums = albumsOf(tracks)
        // Same name, different album ids: two albums, the loose track in none.
        assertEquals(listOf("alb1", "alb2"), albums.map { it.key }.sorted())
        val first = albums.single { it.key == "alb1" }
        assertEquals(listOf("a1", "a2", "a3"), first.tracks.map { it.id })
        assertEquals("A", first.artist)
        assertEquals("alb1", first.artId)
        assertNull(albums.single { it.key == "alb2" }.artId)
    }

    @Test
    fun anAlbumIdWithoutANameStillGroupsUnderAFallbackName() {
        val albums = albumsOf(listOf(track("x", albumId = "alb1"), track("y", albumId = "alb1")))
        assertEquals(UNKNOWN_ALBUM, albums.single().album)
        assertEquals(2, albums.single().tracks.size)
    }

    @Test
    fun albumsWithoutAnIdGroupByNameIgnoringCase() {
        val albums = albumsOf(listOf(track("x", album = "Live"), track("y", album = "live ")))
        assertEquals(1, albums.size)
        assertEquals(2, albums.single().tracks.size)
    }

    @Test
    fun indexGroupsArtistsGenresFoldersAndRecent() {
        val index = LibraryIndex(
            listOf(
                track("1", artist = "Zed", genre = "Rock", directory = "a/b", addedAt = 5),
                track("2", artist = "alpha", genre = " ", directory = "a", addedAt = 9),
                track("3", artist = "Zed", directory = "c", addedAt = 1),
            ),
        )
        assertEquals(listOf("alpha", "Zed"), index.artists.map { it.name })
        assertEquals(listOf("Rock"), index.genres.map { it.name })
        assertEquals(listOf("2", "1", "3"), index.recentlyAdded.map { it.id })
        assertEquals(setOf("1", "2"), index.folderTracks("a", includeSubfolders = true).map { it.id }.toSet())
        assertEquals(listOf("2"), index.folderTracks("a", includeSubfolders = false).map { it.id })
    }

    @Test
    fun searchMatchesFileNamesAndGenres() {
        val tracks = listOf(
            track("1", filename = "Crazy For You.mp3"),
            track("2", title = "Other", genre = "City Pop"),
        )
        assertEquals(listOf("1"), searchTracks(tracks, "crazy").map { it.id })
        assertEquals(listOf("2"), searchTracks(tracks, "city pop").map { it.id })
        assertEquals(2, searchTracks(tracks, "  ").size)
    }

    @Test
    fun navigationPushesPopsAndResetsOnSelect() {
        val nav = LibraryNavigation()
        nav.select(Destination.Root.Albums)
        nav.push(Destination.Album("alb1"))
        nav.push(Destination.Album("alb1"))
        assertEquals(2, nav.stack.size)
        assertEquals(Destination.Root.Albums, nav.root)
        nav.pop()
        assertEquals(Destination.Root.Albums, nav.current)
        nav.push(Destination.Artist("A"))
        nav.select(Destination.Root.Music)
        assertEquals(listOf<Destination>(Destination.Root.Music), nav.stack)
    }

    @Test
    fun starredFollowsTheServerOrderAndTheSearch() {
        val library = LibraryIndex(
            listOf(track("a", title = "Alpha"), track("b", title = "Beta"), track("c", title = "Gamma")),
        )
        // Newest first, as the server sends them; unknown ids are skipped.
        assertEquals(listOf("c", "a"), library.inOrder(listOf("c", "gone", "a")).map { it.id })
        val searched = LibraryIndex(searchTracks(library.tracks, "alp"))
        assertEquals(listOf("a"), searched.inOrder(listOf("c", "a")).map { it.id })
        assertEquals(
            listOf("PLAYLISTS"),
            NAVIGATOR_SECTIONS.filter { (_, roots) -> Destination.Root.Starred in roots }.map { it.first },
        )
    }
}
