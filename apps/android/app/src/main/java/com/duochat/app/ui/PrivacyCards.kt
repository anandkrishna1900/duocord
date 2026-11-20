package com.duochat.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.duochat.app.data.CheckupItem
import com.duochat.app.data.HostRow

/** A short list of privacy basics, how many hold, and a one-tap "Turn on" where the phone can do it itself. */
@Composable
fun PrivacyCheckupCard(items: List<CheckupItem>, onTurnOn: (String) -> Unit, canTurnOn: Set<String>) {
    val score = items.count { it.ok }
    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Privacy checkup", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
                Text("$score/${items.size}", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
            }
            Spacer(Modifier.height(8.dp))
            LinearProgressIndicator(
                progress = { score / items.size.toFloat() },
                modifier = Modifier.fillMaxWidth().height(6.dp),
                color = MaterialTheme.colorScheme.primary,
                trackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                strokeCap = androidx.compose.ui.graphics.StrokeCap.Round,
                drawStopIndicator = {},
            )
            Spacer(Modifier.height(8.dp))
            items.forEach { item ->
                Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.Top) {
                    Icon(
                        if (item.ok) Icons.Filled.CheckCircle else Icons.Filled.Warning,
                        contentDescription = if (item.ok) "In place" else "Needs attention",
                        tint = if (item.ok) com.duochat.app.ui.Online else MaterialTheme.colorScheme.tertiary,
                        modifier = Modifier.size(20.dp),
                    )
                    Spacer(Modifier.width(10.dp))
                    Column(Modifier.weight(1f)) {
                        Text(item.label, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.SemiBold)
                        Text(item.hint, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    if (!item.ok && item.id in canTurnOn) TextButton(onClick = { onTurnOn(item.id) }) { Text("Turn on") }
                }
            }
        }
    }
}

/** Every internet host this phone may contact, and why, straight from the settings that decide it. */
@Composable
fun NetworkHostsCard(rows: List<HostRow>) {
    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("What this app connects to", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            if (rows.isEmpty()) {
                Text("Nothing. In this mode PrivCord only uses Bluetooth and the local network.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            } else {
                rows.forEach { r ->
                    Column {
                        SelectionContainer { Text(r.host, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodyMedium) }
                        Text(r.why, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            Text(
                "That is the whole list: no analytics, no update check, no ads, no account server. Your messages go phone to phone; " +
                    "the hosts above only help two phones find each other, and see sealed data they cannot read.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
