package dev.emusic.mobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import dev.emusic.mobile.ui.AccentPreference
import dev.emusic.mobile.ui.EmusicApp
import dev.emusic.mobile.ui.EmusicTheme
import dev.emusic.mobile.ui.LocalAccentPreference

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val accent = AccentPreference(this)
        setContent {
            CompositionLocalProvider(LocalAccentPreference provides accent) {
                EmusicTheme(accent = accent.accent) {
                    Surface(modifier = Modifier.fillMaxSize()) {
                        EmusicApp()
                    }
                }
            }
        }
    }
}
