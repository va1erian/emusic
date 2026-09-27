package dev.emusic.mobile.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.Lucide
import uniffi.emusic_mobile.Track

/**
 * The server/library status that used to sit atop the library, plus open-source
 * credits. Reached from the info button in the library's top bar.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InfoScreen(url: String, version: Long, tracks: List<Track>, onBack: () -> Unit) {
    Scaffold(
        modifier = Modifier.semantics { testTagsAsResourceId = true },
        topBar = {
            TopAppBar(
                title = { Text("Server info") },
                navigationIcon = {
                    IconButton(onClick = onBack, modifier = Modifier.testTag("info_back")) {
                        Icon(Lucide.ArrowLeft, contentDescription = "Back")
                    }
                },
            )
        },
    ) { insets ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(insets)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            ServerInfoCard(url = url, version = version, tracks = tracks)
            CreditsCard()
        }
    }
}

@Composable
private fun ServerInfoCard(url: String, version: Long, tracks: List<Track>) {
    val distinctAlbums = tracks.mapNotNull { it.album }.filter { it.isNotBlank() }.distinct().size
    val distinctArtists = tracks.mapNotNull { it.artist }.filter { it.isNotBlank() }.distinct().size
    val totalSeconds = tracks.sumOf { it.durationSecs ?: 0.0 }
    val formats = tracks
        .groupingBy { it.format }
        .eachCount()
        .entries
        .sortedByDescending { it.value }
    val special = tracks.count { it.specialized }

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 12.dp)
            .testTag("server_info"),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(url, style = MaterialTheme.typography.titleMedium)
            Text("Library version $version", style = MaterialTheme.typography.bodySmall)
            Text(
                text = "${tracks.size} tracks · $distinctAlbums albums · $distinctArtists artists",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                text = "${formatDuration(totalSeconds)} total · $special specialized",
                style = MaterialTheme.typography.bodySmall,
            )
            if (formats.isNotEmpty()) {
                Text(
                    text = formats.joinToString("  ") { "${it.key} ${it.value}" },
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        }
    }
}

@Composable
private fun CreditsCard() {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(bottom = 16.dp)
            .testTag("credits"),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text("About", style = MaterialTheme.typography.titleMedium)
            Text(
                text = "emusic Android plays a homelab emusic-server library. " +
                    "Standard audio streams with Media3; tracker modules and SID " +
                    "render on-device.",
                style = MaterialTheme.typography.bodySmall,
            )
            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
            Text("Open source", style = MaterialTheme.typography.titleSmall)
            Text(
                text = "Icons: Lucide (lucide.dev) — ISC License.\n" +
                    "Tracker modules: xmrsplayer / xmrs — MIT License.\n" +
                    "SID: sidera — MIT License; MOS 6502 core: mos6502 — BSD-3-Clause.\n" +
                    "Playback: AndroidX Media3 — Apache-2.0.",
                style = MaterialTheme.typography.bodySmall,
            )
        }
    }
}

/** Formats seconds as `h:mm:ss` or `m:ss`. */
private fun formatDuration(seconds: Double): String {
    val total = seconds.toLong().coerceAtLeast(0)
    val hours = total / 3600
    val minutes = (total % 3600) / 60
    val secs = total % 60
    return if (hours > 0) {
        "%d:%02d:%02d".format(hours, minutes, secs)
    } else {
        "%d:%02d".format(minutes, secs)
    }
}
