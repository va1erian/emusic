package dev.emusic.mobile.ui

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.VerticalDivider
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Menu
import dev.emusic.mobile.playback.ActiveServer
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.emusic_mobile.MobileCore
import uniffi.emusic_mobile.Track

/** From this width the navigator is a permanent left pane (an unfolded Fold). */
val TWO_PANE_MIN_WIDTH: Dp = 600.dp

/** The navigator pane's width for a window `width` wide. */
fun navigatorWidth(width: Dp): Dp = (width * 0.32f).coerceIn(220.dp, 300.dp)

/**
 * Connects to `url`, shows the cached library at once, then applies the
 * server's delta. The layout adapts to the window: two panes (the desktop-like
 * navigator beside the content) from [TWO_PANE_MIN_WIDTH], e.g. a Galaxy Fold
 * unfolded; one pane with the navigator in a drawer on a phone or folded cover
 * screen. The activity handles size changes itself, so folding keeps state.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen(url: String, dataDir: String, onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val notificationPermission =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
    val core = remember { runCatching { MobileCore(url, dataDir) }.getOrNull() }

    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var version by remember { mutableLongStateOf(0L) }
    var tracks by remember { mutableStateOf<List<Track>>(emptyList()) }
    var starredIds by remember { mutableStateOf<List<String>>(emptyList()) }
    var nonce by remember { mutableIntStateOf(0) }
    var starredNonce by remember { mutableIntStateOf(0) }
    var query by rememberSaveable { mutableStateOf("") }
    var showInfo by remember { mutableStateOf(false) }
    var showAccent by remember { mutableStateOf(false) }

    LaunchedEffect(nonce) {
        error = null
        val client = core
        if (client == null) {
            error = "invalid server"
            loading = false
            return@LaunchedEffect
        }

        // Show the cached library immediately, then apply the server's delta.
        loading = tracks.isEmpty()
        withContext(Dispatchers.IO) { runCatching { client.cachedLibrary() } }
            .onSuccess { cached ->
                if (cached.tracks.isNotEmpty()) {
                    version = cached.version
                    tracks = cached.tracks
                    loading = false
                    withContext(Dispatchers.IO) { runCatching { client.cachedStarred() } }
                        .onSuccess { starredIds = it }
                    // Let the playback service browse this server.
                    ActiveServer.set(context, url)
                }
            }

        withContext(Dispatchers.IO) { runCatching { client.refreshLibrary() } }
            .onSuccess { refreshed ->
                version = refreshed.version
                tracks = refreshed.tracks
                error = null
                ActiveServer.set(context, url)
                // After the library, so newly synced starred tracks are kept.
                refreshStarred(client)?.let { starredIds = it }
            }
            .onFailure { failure ->
                // Keep the cache when the refresh fails (for example offline).
                if (tracks.isEmpty()) error = failure.message ?: failure.toString()
            }
        loading = false
    }

    // Starring happens on the desktop: re-check (cheaply, by ETag) whenever the
    // app returns to the foreground.
    LaunchedEffect(starredNonce) {
        val client = core
        if (starredNonce > 0 && client != null) refreshStarred(client)?.let { starredIds = it }
    }
    val lifecycle = (context as? LifecycleOwner)?.lifecycle
    DisposableEffect(lifecycle) {
        // The first ON_RESUME is the initial composition, covered by the load above.
        var resumedOnce = false
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                if (resumedOnce) starredNonce++
                resumedOnce = true
            }
        }
        lifecycle?.addObserver(observer)
        onDispose { lifecycle?.removeObserver(observer) }
    }

    val player = rememberPlayerState(core, url) {
        // Ask for notification permission (API 33+) so playback can show its controls.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
    val nav = remember { LibraryNavigation() }
    val options = remember { BrowseOptions() }
    val library = remember(tracks) { LibraryIndex(tracks) }
    val searched = remember(library, query) {
        if (query.isBlank()) library else LibraryIndex(searchTracks(tracks, query))
    }
    val starredSet = remember(starredIds) { starredIds.toSet() }
    val drawer = rememberDrawerState(DrawerValue.Closed)

    // Back closes the drawer, then pops a drill-down, then leaves the server.
    BackHandler {
        when {
            drawer.isOpen -> scope.launch { drawer.close() }
            nav.canGoBack -> nav.pop()
            else -> onBack()
        }
    }

    if (showInfo) {
        // A full-screen dialog overlay, so the library (and its player) stays
        // composed underneath and playback continues while info is open.
        Dialog(
            onDismissRequest = { showInfo = false },
            properties = DialogProperties(usePlatformDefaultWidth = false),
        ) {
            Surface(modifier = Modifier.fillMaxSize()) {
                InfoScreen(url = url, version = version, tracks = tracks, onBack = { showInfo = false })
            }
        }
    }

    val accentPreference = LocalAccentPreference.current
    if (showAccent && accentPreference != null) {
        AccentPickerDialog(
            selected = accentPreference.accent,
            onSelect = accentPreference::select,
            onDismiss = { showAccent = false },
        )
    }

    val artSource = remember(core) { core?.let { client -> ArtSource { client.albumArtFile(it) } } }
    CompositionLocalProvider(LocalArtSource provides artSource, LocalStarredIds provides starredSet) {
        BoxWithConstraints(
            modifier = Modifier
                .fillMaxSize()
                .semantics { testTagsAsResourceId = true },
        ) {
            val wide = maxWidth >= TWO_PANE_MIN_WIDTH
            val paneWidth = navigatorWidth(maxWidth)
            val navigator = @Composable { modifier: Modifier ->
                NavigatorPane(
                    server = url,
                    selected = nav.root,
                    counts = if (loading) emptyMap() else library.counts(starredIds),
                    onSelect = { root ->
                        nav.select(root)
                        scope.launch { drawer.close() }
                    },
                    onServerInfo = { showInfo = true },
                    onRefresh = { nonce++ },
                    onSwitchServer = onBack,
                    onAccent = {
                        showAccent = true
                        scope.launch { drawer.close() }
                    },
                    modifier = modifier,
                )
            }
            val screen = @Composable {
                Scaffold(
                    topBar = {
                        TopAppBar(
                            title = {
                                Text(
                                    text = nav.current.title(library),
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            },
                            navigationIcon = {
                                when {
                                    nav.canGoBack -> IconButton(
                                        onClick = nav::pop,
                                        modifier = Modifier.testTag("back_action"),
                                    ) { Icon(Lucide.ArrowLeft, contentDescription = "Back") }

                                    !wide -> IconButton(
                                        onClick = { scope.launch { drawer.open() } },
                                        modifier = Modifier.testTag("menu_action"),
                                    ) { Icon(Lucide.Menu, contentDescription = "Open navigator") }
                                }
                            },
                        )
                    },
                    bottomBar = {
                        Column(modifier = Modifier.navigationBarsPadding()) {
                            player.message?.let { message ->
                                Text(
                                    text = message,
                                    color = MaterialTheme.colorScheme.error,
                                    style = MaterialTheme.typography.bodySmall,
                                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                                )
                            }
                            PlayerBar(player = player, onOpenNowPlaying = {
                                nav.select(Destination.Root.NowPlaying)
                            })
                        }
                    },
                ) { insets ->
                    Row(modifier = Modifier.padding(insets).fillMaxSize()) {
                        if (wide) {
                            navigator(Modifier.width(paneWidth))
                            VerticalDivider()
                        }
                        Column(
                            modifier = Modifier
                                .weight(1f)
                                .fillMaxHeight()
                                .padding(horizontal = 16.dp),
                        ) {
                            LibraryBody(
                                loading = loading,
                                error = error,
                                onRetry = { nonce++ },
                            ) {
                                // The queue is not searchable; everything else is.
                                if (nav.current != Destination.Root.NowPlaying) {
                                    SearchField(query = query, onQueryChange = { query = it })
                                }
                                ContentPane(
                                    nav = nav,
                                    library = library,
                                    searched = searched,
                                    query = query,
                                    starredIds = starredIds,
                                    options = options,
                                    player = player,
                                    modifier = Modifier.weight(1f),
                                )
                            }
                        }
                    }
                }
            }
            if (wide) {
                screen()
            } else {
                ModalNavigationDrawer(
                    drawerState = drawer,
                    drawerContent = { ModalDrawerSheet { navigator(Modifier) } },
                ) { screen() }
            }
        }
    }
}

/** Loading spinner, error with retry, or the library [content]. */
@Composable
private fun ColumnScope.LibraryBody(
    loading: Boolean,
    error: String?,
    onRetry: () -> Unit,
    content: @Composable ColumnScope.() -> Unit,
) {
    when {
        loading -> Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(24.dp)
                .testTag("loading"),
            horizontalArrangement = Arrangement.Center,
        ) {
            CircularProgressIndicator()
        }

        error != null -> Column(
            modifier = Modifier.padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(text = error, color = MaterialTheme.colorScheme.error)
            Button(onClick = onRetry) { Text("Retry") }
        }

        else -> content()
    }
}

/** Fetches the starred ids, or `null` when the server cannot be reached. */
private suspend fun refreshStarred(client: MobileCore): List<String>? =
    withContext(Dispatchers.IO) { runCatching { client.refreshStarred() }.getOrNull() }

/** Navigator badges: how many tracks, albums, artists, genres, folders and stars. */
private fun LibraryIndex.counts(starredIds: List<String>): Map<Destination.Root, Int> = mapOf(
    Destination.Root.Music to tracks.size,
    Destination.Root.Albums to albums.size,
    Destination.Root.Artists to artists.size,
    Destination.Root.Genres to genres.size,
    Destination.Root.Folders to folders.size,
    Destination.Root.Starred to inOrder(starredIds).size,
)
