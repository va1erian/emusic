package dev.emusic.mobile.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.emusic_mobile.MobileCore
import uniffi.emusic_mobile.ServerEntry
import uniffi.emusic_mobile.listServers
import java.io.File

/** A registry entry plus whether credentials are currently stored for it. */
private data class ServerView(val entry: ServerEntry, val paired: Boolean)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EmusicApp() {
    val context = LocalContext.current
    val dataDir = remember { File(context.filesDir, "emusic").absolutePath }
    val scope = rememberCoroutineScope()

    var servers by remember { mutableStateOf<List<ServerView>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }
    var adding by remember { mutableStateOf(false) }
    var status by remember { mutableStateOf<String?>(null) }
    var nonce by remember { mutableStateOf(0) }
    var browsing by remember { mutableStateOf<ServerView?>(null) }

    suspend fun reload() {
        loading = true
        servers = withContext(Dispatchers.IO) { loadServers(dataDir) }
        loading = false
    }

    LaunchedEffect(nonce) { reload() }

    fun run(action: suspend () -> Unit, onSuccess: () -> Unit = {}) {
        scope.launch {
            val error = withContext(Dispatchers.IO) {
                runCatching { action() }.exceptionOrNull()?.message
            }
            status = error
            if (error == null) onSuccess()
            nonce++
        }
    }

    val open = browsing
    if (open != null) {
        LibraryScreen(
            url = open.entry.url,
            dataDir = dataDir,
            onBack = {
                browsing = null
                nonce++
            },
        )
        return
    }

    Scaffold(
        modifier = Modifier.semantics { testTagsAsResourceId = true },
        topBar = {
            TopAppBar(
                title = { Text(if (adding) "Add server" else "emusic") },
                actions = {
                    TextButton(
                        onClick = {
                            adding = !adding
                            status = null
                        },
                        modifier = Modifier.testTag("add_action"),
                    ) {
                        Text(if (adding) "Cancel" else "Add")
                    }
                },
            )
        },
    ) { insets ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(insets)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            status?.let { message ->
                Text(
                    text = message,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }

            if (adding) {
                AddServerForm(
                    onPair = { url, code, name ->
                        var entry: ServerEntry? = null
                        run(
                            action = {
                                val core = MobileCore(url, dataDir)
                                core.pair(code, name)
                                entry = core.entry()
                            },
                            onSuccess = {
                                adding = false
                                entry?.let { browsing = ServerView(it, true) }
                            },
                        )
                    },
                )
            } else if (loading) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.Center,
                ) {
                    CircularProgressIndicator()
                }
            } else if (servers.isEmpty()) {
                Onboarding(onStart = { adding = true })
            } else {
                Column(
                    modifier = Modifier.verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    servers.forEach { view ->
                        ServerRow(
                            view = view,
                            onBrowse = { browsing = view },
                            onDisconnect = { run { MobileCore(view.entry.url, dataDir).disconnect() } },
                            onRevoke = { run { MobileCore(view.entry.url, dataDir).revoke() } },
                            onRemove = { run { MobileCore(view.entry.url, dataDir).removeServer() } },
                        )
                    }
                }
            }
        }
    }
}

/** Loads the registry and each server's pairing state. */
private fun loadServers(dataDir: String): List<ServerView> =
    listServers(dataDir).map { entry ->
        val paired = runCatching { MobileCore(entry.url, dataDir).authState().paired }
            .getOrDefault(false)
        ServerView(entry, paired)
    }

@Composable
private fun Onboarding(onStart: () -> Unit) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .testTag("onboarding"),
    ) {
        Column(
            modifier = Modifier.padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text("Welcome to emusic", style = MaterialTheme.typography.headlineSmall)
            Text(
                text = "Stream your homelab music server to this phone. The app " +
                    "reads the library from the server — it never scans local files.",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                text = "On the server, run `emusic-server pair` to get a 6-digit code, " +
                    "then pair here.",
                style = MaterialTheme.typography.bodySmall,
            )
            Button(
                onClick = onStart,
                modifier = Modifier
                    .align(Alignment.End)
                    .testTag("onboarding_start"),
            ) {
                Text("Pair a server")
            }
        }
    }
}

@Composable
private fun ServerRow(
    view: ServerView,
    onBrowse: () -> Unit,
    onDisconnect: () -> Unit,
    onRevoke: () -> Unit,
    onRemove: () -> Unit,
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(view.entry.name, style = MaterialTheme.typography.titleMedium)
            Text(view.entry.url, style = MaterialTheme.typography.bodySmall)
            Text(
                text = if (view.paired) "Paired" else "Not paired",
                style = MaterialTheme.typography.bodySmall,
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                TextButton(
                    onClick = onBrowse,
                    enabled = view.paired,
                    modifier = Modifier.testTag("browse_button"),
                ) { Text("Browse") }
                TextButton(onClick = onDisconnect, enabled = view.paired) { Text("Disconnect") }
                TextButton(onClick = onRevoke, enabled = view.paired) { Text("Revoke") }
                TextButton(onClick = onRemove) { Text("Remove") }
            }
        }
    }
}

@Composable
private fun AddServerForm(onPair: (String, String, String) -> Unit) {
    var url by rememberSaveable { mutableStateOf("http://chatonnas:11337") }
    var code by rememberSaveable { mutableStateOf("") }
    var name by rememberSaveable { mutableStateOf("Android phone") }

    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedTextField(
            value = url,
            onValueChange = { url = it },
            label = { Text("Server URL") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().testTag("server_url"),
        )
        OutlinedTextField(
            value = code,
            onValueChange = { code = it.filter(Char::isDigit).take(12) },
            label = { Text("Pairing code") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.fillMaxWidth().testTag("pairing_code"),
        )
        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            label = { Text("Device name") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().testTag("device_name"),
        )
        Button(
            enabled = url.isNotBlank() && code.isNotBlank(),
            onClick = { onPair(url.trim(), code.trim(), name.trim()) },
            modifier = Modifier
                .align(Alignment.End)
                .testTag("pair_button"),
        ) {
            Text("Pair")
        }
    }
}
