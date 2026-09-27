package dev.emusic.mobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.emusic_mobile.MobileCore
import java.io.File

class MainActivity : ComponentActivity() {
    @OptIn(ExperimentalMaterial3Api::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            MaterialTheme {
                Scaffold(
                    topBar = { TopAppBar(title = { Text("emusic") }) },
                ) { insets ->
                    PairingScreen(modifier = Modifier.padding(insets))
                }
            }
        }
    }
}

@Composable
private fun PairingScreen(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var url by rememberSaveable { mutableStateOf("http://10.0.2.2:8080") }
    var code by rememberSaveable { mutableStateOf("") }
    var deviceName by rememberSaveable { mutableStateOf("Android phone") }
    var status by rememberSaveable { mutableStateOf("Not paired") }
    var busy by remember { mutableStateOf(false) }

    fun core(): MobileCore {
        val dataDir = File(context.filesDir, "emusic").absolutePath
        return MobileCore(url.trim(), dataDir)
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        OutlinedTextField(
            value = url,
            onValueChange = { url = it },
            label = { Text("Server URL") },
            singleLine = true,
            enabled = !busy,
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = code,
            onValueChange = { code = it.filter(Char::isDigit).take(12) },
            label = { Text("Pairing code") },
            singleLine = true,
            enabled = !busy,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = deviceName,
            onValueChange = { deviceName = it },
            label = { Text("Device name") },
            singleLine = true,
            enabled = !busy,
            modifier = Modifier.fillMaxWidth(),
        )
        Button(
            enabled = !busy && url.isNotBlank() && code.isNotBlank(),
            onClick = {
                busy = true
                status = "Pairing..."
                scope.launch {
                    status = withContext(Dispatchers.IO) {
                        runCatching { core().pair(code.trim(), deviceName.trim()) }
                            .fold(
                                onSuccess = { state -> "Paired as ${state.deviceName} (${state.deviceId})" },
                                onFailure = { error -> "Pairing failed: ${error.message}" },
                            )
                    }
                    busy = false
                }
            },
        ) {
            Text("Pair")
        }
        Button(
            enabled = !busy && url.isNotBlank(),
            onClick = {
                busy = true
                status = "Syncing..."
                scope.launch {
                    status = withContext(Dispatchers.IO) {
                        runCatching { core().sync() }.fold(
                            onSuccess = { delta ->
                                "Library version ${delta.version}: ${delta.tracks.size} changed, " +
                                    "${delta.deleted.size} deleted"
                            },
                            onFailure = { error -> "Sync failed: ${error.message}" },
                        )
                    }
                    busy = false
                }
            },
        ) {
            Text("Sync")
        }
        Card(modifier = Modifier.fillMaxWidth()) {
            Text(
                text = status,
                modifier = Modifier.padding(16.dp),
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}
