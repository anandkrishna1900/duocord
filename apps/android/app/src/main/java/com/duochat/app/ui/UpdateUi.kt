package com.duochat.app.ui

import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.duochat.app.AppUpdater
import com.duochat.app.data.AppUpdate
import com.duochat.app.data.NetworkConfig
import com.duochat.app.data.UpdateInfo
import kotlinx.coroutines.launch

/** "Install" for a found update: downloads it, then Android asks the user to confirm. */
@Composable
fun UpdateInstallButton(info: UpdateInfo, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf(false) }
    var percent by remember { mutableStateOf(0) }
    var message by remember { mutableStateOf<String?>(null) }
    Column(modifier) {
        Button(
            enabled = !busy,
            onClick = {
                if (!context.packageManager.canRequestPackageInstalls()) {
                    message = "Allow installing apps from PrivCord on the next screen, then come back and tap Install again."
                    context.startActivity(
                        Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}"))
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                    )
                } else {
                    busy = true
                    message = null
                    scope.launch {
                        AppUpdater.install(context, info) { percent = it }
                            .onSuccess { message = "Downloaded. Confirm the install when Android asks." }
                            .onFailure { message = it.message ?: "The update failed." }
                        busy = false
                    }
                }
            },
        ) { Text(if (busy) "Downloading… $percent%" else "Install ${info.versionName}") }
        message?.let {
            Spacer(Modifier.height(6.dp))
            Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** The prompt shown at start-up when a newer build exists. "Later" is remembered for a few days by the caller. */
@Composable
fun UpdateDialog(info: UpdateInfo, onLater: () -> Unit) {
    AlertDialog(
        onDismissRequest = onLater,
        title = { Text("Update available") },
        text = {
            Column {
                Text("PrivCord ${info.versionName} is ready. Your messages and settings stay as they are.")
                Spacer(Modifier.height(12.dp))
                UpdateInstallButton(info)
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onLater) { Text("Later") } },
    )
}

/** Settings: "Check now", with the result and an Install button. Blocked while Hide my IP or Tor is on. */
@Composable
fun UpdateCheckRow(network: NetworkConfig) {
    val scope = rememberCoroutineScope()
    var checking by remember { mutableStateOf(false) }
    var status by remember { mutableStateOf<String?>(null) }
    var found by remember { mutableStateOf<UpdateInfo?>(null) }
    Column {
        if (!AppUpdate.mayCheck(network)) {
            Text(
                "Turn off Hide my IP and Tor mode to check for updates: a check contacts GitHub directly.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            OutlinedButton(
                enabled = !checking,
                onClick = {
                    checking = true
                    status = null
                    found = null
                    scope.launch {
                        AppUpdater.latest()
                            .onSuccess { info -> found = info; status = if (info == null) "You are up to date." else "Version ${info.versionName} is available." }
                            .onFailure { status = "Could not check for updates. Try again later." }
                        checking = false
                    }
                },
            ) { Text(if (checking) "Checking…" else "Check for updates now") }
        }
        status?.let {
            Spacer(Modifier.height(6.dp))
            Text(it, style = MaterialTheme.typography.bodySmall)
        }
        found?.let {
            Spacer(Modifier.height(8.dp))
            UpdateInstallButton(it)
        }
    }
}
