package dev.emusic.mobile.playback

import android.content.Context
import java.io.File

/**
 * The server the UI is currently browsing.
 *
 * The playback service outlives the UI, so it cannot be handed the URL through
 * the composition; the library screen records it here before playing and the
 * service reads it when it resolves the bearer token.
 */
object ActiveServer {
    private const val PREFS = "emusic_playback"
    private const val KEY_URL = "server_url"

    /** The app's private data directory, shared by the UI and the service. */
    fun dataDir(context: Context): String = File(context.filesDir, "emusic").absolutePath

    /** Records the server the UI is browsing. */
    fun set(context: Context, url: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_URL, url)
            .apply()
    }

    /** The last server the UI browsed, or `null`. */
    fun get(context: Context): String? =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY_URL, null)
}
