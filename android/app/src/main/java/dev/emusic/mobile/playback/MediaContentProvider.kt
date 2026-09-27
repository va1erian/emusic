package dev.emusic.mobile.playback

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import java.io.File
import java.io.FileNotFoundException
import uniffi.emusic_mobile.MobileCore

/**
 * Serves the two app-private things Android Auto needs over `content://` URIs:
 *
 * - `content://dev.emusic.mobile.media/art/<albumId>` — cover art, downloaded
 *   with the bearer token and cached.
 * - `content://dev.emusic.mobile.media/render/<trackId>?format=<fmt>` — a
 *   specialized track rendered on-device to a cached FLAC.
 *
 * A content provider is used because Android Auto (and Media3's artwork loader)
 * cannot send the `Authorization` header itself; the provider adds the token.
 */
class MediaContentProvider : ContentProvider() {
    override fun onCreate(): Boolean = true

    override fun getType(uri: Uri): String? = when (uri.pathSegments.firstOrNull()) {
        SEGMENT_ART -> "image/*"
        SEGMENT_RENDER -> "audio/flac"
        else -> null
    }

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        val ctx = context ?: throw FileNotFoundException("no context")
        val core = activeCore(ctx) ?: throw FileNotFoundException("no active server")
        // `albumArtFile`/`renderToFile` throw `MobileException` on failure, but
        // the ContentResolver contract expects `FileNotFoundException`.
        val path = runCatching { resolve(core, uri) }
            .getOrElse { error -> throw FileNotFoundException("$uri: ${error.message}") }
            ?: throw FileNotFoundException(uri.toString())
        return ParcelFileDescriptor.open(File(path), ParcelFileDescriptor.MODE_READ_ONLY)
    }

    /** Resolves a content URI to an app-private file path. */
    private fun resolve(core: MobileCore, uri: Uri): String? {
        val segments = uri.pathSegments
        return when (segments.firstOrNull()) {
            SEGMENT_ART -> segments.getOrNull(1)?.let { albumId -> core.albumArtFile(albumId) }
            SEGMENT_RENDER -> {
                val trackId = segments.getOrNull(1)
                val format = uri.getQueryParameter("format")
                if (trackId == null || format == null) {
                    null
                } else {
                    core.renderToFile(trackId, format)
                }
            }

            else -> null
        }
    }

    override fun query(
        uri: Uri,
        projection: Array<out String>?,
        selection: String?,
        selectionArgs: Array<out String>?,
        sortOrder: String?,
    ): Cursor? = null

    override fun insert(uri: Uri, values: ContentValues?): Uri? = null

    override fun update(
        uri: Uri,
        values: ContentValues?,
        selection: String?,
        selectionArgs: Array<out String>?,
    ): Int = 0

    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0

    companion object {
        const val AUTHORITY = "dev.emusic.mobile.media"
        private const val SEGMENT_ART = "art"
        private const val SEGMENT_RENDER = "render"

        /** The cover-art URI for an album. */
        fun artUri(albumId: String): Uri =
            Uri.parse("content://$AUTHORITY/$SEGMENT_ART/${Uri.encode(albumId)}")

        /** The on-device rendered-audio URI for a specialized track. */
        fun renderUri(trackId: String, format: String): Uri =
            Uri.parse(
                "content://$AUTHORITY/$SEGMENT_RENDER/${Uri.encode(trackId)}" +
                    "?format=${Uri.encode(format)}",
            )

        private fun activeCore(context: Context): MobileCore? {
            val url = ActiveServer.get(context) ?: return null
            return runCatching { MobileCore(url, ActiveServer.dataDir(context)) }.getOrNull()
        }
    }
}
