package dev.emusic.mobile.ui

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Music
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Resolves an album id to a local cover file (the Rust core downloads and
 * disk-caches it). `null` when no server is connected.
 */
fun interface ArtSource {
    fun artFile(albumId: String): String
}

/** The art source for the current server, provided by the library screen. */
val LocalArtSource = staticCompositionLocalOf<ArtSource?> { null }

/** Decoded covers, bounded by bytes; shared by the grid, details and player. */
private object ArtMemoryCache {
    private const val MAX_BYTES = 32 * 1024 * 1024
    private val cache = object : LruCache<String, ImageBitmap>(MAX_BYTES) {
        override fun sizeOf(key: String, value: ImageBitmap) = value.width * value.height * 4
    }

    /**
     * Albums whose downloaded art could not be decoded, so scrolling does not
     * refetch them. Download errors (offline, timeouts) are not recorded here:
     * they stay retryable.
     */
    private val failed = HashSet<String>()

    fun get(albumId: String): ImageBitmap? = cache.get(albumId)
    fun put(albumId: String, bitmap: ImageBitmap) {
        cache.put(albumId, bitmap)
    }

    @Synchronized fun hasFailed(albumId: String) = albumId in failed
    @Synchronized fun markFailed(albumId: String) {
        failed += albumId
    }
}

/** At most four covers download/decode at once, so a fast fling stays light. */
private val artDispatcher = Dispatchers.IO.limitedParallelism(4)

/** Longest edge, in pixels, covers are decoded at. */
private const val DECODE_EDGE = 512

/**
 * An album cover, or a coloured placeholder with a note (as on the desktop)
 * while it loads, when the server has none, or when it fails to load.
 * [colorKey] picks the placeholder colour, so each album keeps its own.
 */
@Composable
fun AlbumArt(artId: String?, colorKey: String, modifier: Modifier = Modifier) {
    val source = LocalArtSource.current
    val bitmap by produceState(initialValue = artId?.let(ArtMemoryCache::get), artId, source) {
        if (value != null || artId == null || source == null || ArtMemoryCache.hasFailed(artId)) {
            return@produceState
        }
        val path = try {
            withContext(artDispatcher) { source.artFile(artId) }
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            // Likely transient (offline, timeout): show the placeholder now and
            // try again the next time this cover is composed.
            return@produceState
        }
        val cover = withContext(artDispatcher) { decodeCover(path) }
        if (cover == null) {
            ArtMemoryCache.markFailed(artId)
        } else {
            ArtMemoryCache.put(artId, cover)
            value = cover
        }
    }
    Box(
        modifier = modifier
            .fillMaxWidth()
            .aspectRatio(1f)
            .clip(RoundedCornerShape(8.dp)),
        contentAlignment = Alignment.Center,
    ) {
        val cover = bitmap
        if (cover != null) {
            Image(
                bitmap = cover,
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize(),
            )
        } else {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(placeholderColor(colorKey)),
                contentAlignment = Alignment.Center,
            ) {
                Icon(Lucide.Music, contentDescription = null, tint = Color.White.copy(alpha = 0.85f))
            }
        }
    }
}

/** Decodes `path` downsampled so its longest edge is about [DECODE_EDGE]. */
private fun decodeCover(path: String): ImageBitmap? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeFile(path, bounds)
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
    var sample = 1
    while (maxOf(bounds.outWidth, bounds.outHeight) / (sample * 2) >= DECODE_EDGE) sample *= 2
    val options = BitmapFactory.Options().apply { inSampleSize = sample }
    return BitmapFactory.decodeFile(path, options)?.asImageBitmap()
}

/** A muted, stable colour per album, like the desktop's art placeholders. */
fun placeholderColor(key: String): Color {
    val hue = ((key.hashCode() % 360) + 360) % 360
    return Color.hsl(hue.toFloat(), saturation = 0.38f, lightness = 0.42f)
}
