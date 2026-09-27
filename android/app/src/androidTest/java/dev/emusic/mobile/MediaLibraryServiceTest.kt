package dev.emusic.mobile

import android.content.ComponentName
import android.content.Context
import androidx.media3.session.MediaBrowser
import androidx.media3.session.SessionToken
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.emusic.mobile.playback.LibraryBrowser
import dev.emusic.mobile.playback.PlaybackService
import uniffi.emusic_mobile.MobileCore
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.After
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * Verifies the Android Auto browse tree served by [PlaybackService]: it seeds a
 * cached library, connects a [MediaBrowser] and walks the tree. This is the
 * automatable stand-in for a Desktop Head Unit session.
 */
@RunWith(AndroidJUnit4::class)
class MediaLibraryServiceTest {
    private val timeout = 20L

    @Before
    fun seedLibrary() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "emusic")
        dir.mkdirs()
        File(dir, "library.json").writeText(LIBRARY_JSON)
        context.getSharedPreferences("emusic_playback", Context.MODE_PRIVATE)
            .edit()
            .putString("server_url", "http://10.0.2.2:8080")
            .commit()
    }

    @After
    fun clearLibrary() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        File(File(context.filesDir, "emusic"), "library.json").delete()
        context.getSharedPreferences("emusic_playback", Context.MODE_PRIVATE)
            .edit()
            .clear()
            .commit()
    }

    @Test
    fun browsesCategoriesAlbumsAndTracks() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        val browser = onMain { MediaBrowser.Builder(context, token).buildAsync() }
            .get(timeout, TimeUnit.SECONDS)
        try {
            val root = onMain { browser.getLibraryRoot(null) }
                .get(timeout, TimeUnit.SECONDS).value!!
            assertEquals(LibraryBrowser.ROOT, root.mediaId)

            val categories = onMain { browser.getChildren(LibraryBrowser.ROOT, 0, 100, null) }
                .get(timeout, TimeUnit.SECONDS).value!!
            val ids = categories.map { it.mediaId }
            assertTrue("expected all categories, got $ids", ids.containsAll(CATEGORIES))

            val albums = onMain { browser.getChildren(LibraryBrowser.ALBUMS, 0, 100, null) }
                .get(timeout, TimeUnit.SECONDS).value!!
            assertEquals(2, albums.size)

            val albumTracks = onMain { browser.getChildren(albums[0].mediaId, 0, 100, null) }
                .get(timeout, TimeUnit.SECONDS).value!!
            assertTrue(albumTracks.isNotEmpty())
            assertTrue(albumTracks[0].mediaMetadata.isPlayable == true)
            assertEquals("content", albumTracks[0].mediaMetadata.artworkUri?.scheme)

            val allTracks = onMain { browser.getChildren(LibraryBrowser.TRACKS, 0, 100, null) }
                .get(timeout, TimeUnit.SECONDS).value!!
            assertEquals(2, allTracks.size)

            // A standard track streams over http; a specialized one plays its
            // on-device rendered FLAC via content://.
            val stream = allTracks.first { it.mediaId == "track:t2" }
            assertEquals("http", stream.requestMetadata.mediaUri?.scheme)

            val dataDir = File(context.filesDir, "emusic").absolutePath
            assertTrue(
                "canRender(mod) should be true on device",
                MobileCore("http://10.0.2.2:8080", dataDir).canRender("mod"),
            )
            val specialized = allTracks.first { it.mediaId == "track:t1" }
            assertEquals("content", specialized.requestMetadata.mediaUri?.scheme)
        } finally {
            onMain { browser.release() }
        }
    }

    /** Runs [block] on the application main thread and returns its result. */
    private fun <T> onMain(block: () -> T): T {
        val result = AtomicReference<T>()
        InstrumentationRegistry.getInstrumentation().runOnMainSync { result.set(block()) }
        return result.get()
    }

    private companion object {
        val CATEGORIES = listOf(
            LibraryBrowser.ALBUMS,
            LibraryBrowser.ARTISTS,
            LibraryBrowser.FOLDERS,
            LibraryBrowser.TRACKS,
        )

        const val LIBRARY_JSON = """
            {"version":1,"tracks":[
              {"id":"t1","filename":"a.mod","directory":"Mods","format":"mod","kind":"module","specialized":true,"title":"First Tune","artist":"An Artist","album_artist":null,"album":"An Album","album_id":"alb1","genre":null,"year":null,"track_no":null,"disc_no":null,"duration_secs":120.0,"subtunes":1,"channels":null,"file_size":1234,"has_art":true,"sync_version":1,"added_at":0},
              {"id":"t2","filename":"b.mp3","directory":"Songs","format":"mp3","kind":"stream","specialized":false,"title":"Second Song","artist":"An Artist","album_artist":null,"album":"Other","album_id":null,"genre":null,"year":null,"track_no":null,"disc_no":null,"duration_secs":60.0,"subtunes":1,"channels":null,"file_size":900,"has_art":false,"sync_version":1,"added_at":0}
            ]}
        """
    }
}
