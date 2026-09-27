package dev.emusic.mobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import dev.emusic.mobile.ui.EmusicApp
import dev.emusic.mobile.ui.EmusicTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            EmusicTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    EmusicApp()
                }
            }
        }
    }
}
