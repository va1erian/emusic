package dev.emusic.mobile.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.NavigationDrawerItemDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.Clock
import com.composables.icons.lucide.Disc3
import com.composables.icons.lucide.Folder
import com.composables.icons.lucide.Info
import com.composables.icons.lucide.ListMusic
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Music
import com.composables.icons.lucide.Palette
import com.composables.icons.lucide.RefreshCw
import com.composables.icons.lucide.Star
import com.composables.icons.lucide.Tag
import com.composables.icons.lucide.Users

/** The navigator icon for a root view. */
val Destination.Root.icon: ImageVector
    get() = when (this) {
        Destination.Root.Music -> Lucide.Music
        Destination.Root.Albums -> Lucide.Disc3
        Destination.Root.Artists -> Lucide.Users
        Destination.Root.Genres -> Lucide.Tag
        Destination.Root.Folders -> Lucide.Folder
        Destination.Root.Starred -> Lucide.Star
        Destination.Root.Recent -> Lucide.Clock
        Destination.Root.NowPlaying -> Lucide.ListMusic
    }

/**
 * The library navigator: LIBRARY, PLAYLISTS and ACTIVITY sections like the
 * desktop sidebar, then server actions and appearance. It is the permanent left pane on wide screens
 * (an unfolded Fold, a tablet) and the drawer's content on a phone.
 *
 * [counts] labels each root with its size (tracks, albums, ...), when known.
 */
@Composable
fun NavigatorPane(
    server: String,
    selected: Destination.Root,
    counts: Map<Destination.Root, Int>,
    onSelect: (Destination.Root) -> Unit,
    onServerInfo: () -> Unit,
    onRefresh: () -> Unit,
    onSwitchServer: () -> Unit,
    onAccent: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxHeight()
            .verticalScroll(rememberScrollState())
            .padding(vertical = 8.dp)
            .testTag("navigator"),
    ) {
        Text(
            text = server,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(horizontal = 28.dp, vertical = 8.dp),
        )
        NAVIGATOR_SECTIONS.forEach { (heading, roots) ->
            SectionHeading(heading)
            roots.forEach { root ->
                NavigatorRow(
                    label = root.label,
                    icon = root.icon,
                    selected = root == selected,
                    badge = counts[root],
                    onClick = { onSelect(root) },
                    tag = "nav_${root.name.lowercase()}",
                )
            }
        }
        HorizontalDivider(modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp))
        SectionHeading("SERVER")
        NavigatorRow("Server info", Lucide.Info, false, null, onServerInfo, "nav_info")
        NavigatorRow("Refresh library", Lucide.RefreshCw, false, null, onRefresh, "nav_refresh")
        NavigatorRow("Switch server", Lucide.ArrowLeft, false, null, onSwitchServer, "nav_servers")
        SectionHeading("APPEARANCE")
        NavigatorRow("Accent colour", Lucide.Palette, false, null, onAccent, "nav_accent")
    }
}

@Composable
private fun SectionHeading(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 28.dp, top = 12.dp, bottom = 4.dp),
    )
}

@Composable
private fun NavigatorRow(
    label: String,
    icon: ImageVector,
    selected: Boolean,
    badge: Int?,
    onClick: () -> Unit,
    tag: String,
) {
    NavigationDrawerItem(
        label = { Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        icon = { Icon(icon, contentDescription = null) },
        badge = badge?.let { count -> @Composable { Text(count.toString()) } },
        selected = selected,
        onClick = onClick,
        colors = NavigationDrawerItemDefaults.colors(
            unselectedContainerColor = MaterialTheme.colorScheme.surface.copy(alpha = 0f),
        ),
        modifier = Modifier
            .padding(NavigationDrawerItemDefaults.ItemPadding)
            .testTag(tag),
    )
}
