package com.duochat.app.data

/** Pinned chats first (in the order they were pinned), then everything else as given. */
fun <T> withPinsFirst(rows: List<T>, pinned: List<String>, id: (T) -> String): List<T> {
    val pins = pinned.mapNotNull { p -> rows.firstOrNull { id(it) == p } }
    return pins + rows.filter { id(it) !in pinned }
}

/** One internet host this phone may contact, and why. */
data class HostRow(val host: String, val why: String)

private fun hostOf(url: String): String =
    url.replace(Regex("^(stuns?|turns?):", RegexOption.IGNORE_CASE), "https://")
        .let { runCatching { java.net.URI(it).authority }.getOrNull() }
        ?: url.replace(Regex("^(stuns?|turns?):", RegexOption.IGNORE_CASE), "").substringBefore('?')

/** Every internet host the settings allow. No analytics, no update check, no other servers. */
fun networkHosts(n: NetworkConfig): List<HostRow> {
    val rows = mutableListOf<HostRow>()
    fun add(host: String, why: String) {
        if (host.isNotEmpty() && rows.none { it.host == host && it.why == why }) rows += HostRow(host, why)
    }
    if (n.transportMode != TransportMode.BLUETOOTH_ONLY) {
        for (s in n.signaling) when (s) {
            is SignalingConfig.Nostr -> s.relays.forEach { add(hostOf(it), "Finds your friend and passes sealed hellos (Nostr relay)") }
            is SignalingConfig.Relay -> add(hostOf(s.url), "Finds your friend and passes sealed messages (your own relay)")
        }
        if (!n.forceRelay) n.stun.forEach { add(hostOf(it), "Tells your phone its public address so a direct connection can start (STUN)") }
        n.turn.forEach { t -> t.urls.forEach { add(hostOf(it), "Carries encrypted traffic when a direct connection is not possible (TURN)") } }
    }
    return rows
}

data class CheckupItem(val id: String, val label: String, val ok: Boolean, val hint: String)

/** Seven privacy basics and which of them hold. All read on this phone; nothing is sent anywhere. */
fun privacyCheckup(
    appLock: Boolean,
    lastBackupAt: Long,
    hideIp: Boolean,
    previewsOff: Boolean,
    groupOnly: Boolean,
    panicWipe: Boolean,
    staleDevices: Int,
    now: Long = System.currentTimeMillis(),
): List<CheckupItem> {
    val recent = lastBackupAt > 0 && now - lastBackupAt < 28L * 24 * 3600 * 1000
    return listOf(
        CheckupItem("lock", "App lock", appLock, if (appLock) "Opening PrivCord asks for your fingerprint, face or screen lock." else "Anyone holding your unlocked phone can read your chats. Turn on the lock."),
        CheckupItem("backup", "Backup", recent, if (recent) "A recent backup exists." else "No recent backup. Without one, a lost phone means lost chats (your 12 words only restore your identity)."),
        CheckupItem("ip", "IP address hidden from friends", hideIp, if (hideIp) "Connections go through a relay." else "Friends can see your IP address on a direct connection. Optional, slower."),
        CheckupItem("previews", "Notifications hide message text", previewsOff, if (previewsOff) "Notifications only say a message arrived." else "Message text can appear on a locked screen. Turn off message previews."),
        CheckupItem("grouponly", "Group-only with people met in groups", groupOnly, if (groupOnly) "Strangers from a group cannot message you directly." else "People you only know from a group can message you directly."),
        CheckupItem("panic", "Panic wipe ready", panicWipe, if (panicWipe) "Triple-tap the Chats title to erase this phone." else "Optional: a one-gesture wipe for emergencies."),
        CheckupItem("devices", "No forgotten devices", staleDevices == 0, if (staleDevices == 0) "Every linked device was seen recently." else "$staleDevices linked device${if (staleDevices == 1) "" else "s"} not seen for 90 days. Remove ${if (staleDevices == 1) "it" else "them"} if lost."),
    )
}

fun minutesLabel(m: Int): String = "%02d:%02d".format(m / 60, m % 60)

/** A message's delivery state in words. */
fun deliveryWords(s: DeliveryState): String = when (s) {
    DeliveryState.QUEUED -> "Waiting for them to come online"
    DeliveryState.SENT -> "Sent"
    DeliveryState.DELIVERED -> "Delivered"
    DeliveryState.READ -> "Read"
    DeliveryState.FAILED -> "Could not be sent"
}

data class ChatStorage(val id: String, val name: String, val messages: Int, val files: Int, val bytes: Long, val fileBytes: Long, val fileIds: List<String>)

/** What each chat takes on this phone, biggest first (deleted messages are free). */
fun storageByChat(messages: Map<String, List<ChatMessage>>, nameOf: (String) -> String): List<ChatStorage> =
    messages.map { (id, list) ->
        val live = list.filter { !it.deleted }
        val fileMsgs = live.filter { it.file != null }
        val fileBytes = fileMsgs.sumOf { it.file!!.size.toLong() }
        ChatStorage(id, nameOf(id), live.size, fileMsgs.size, live.sumOf { it.text.toByteArray().size.toLong() } + fileBytes, fileBytes, fileMsgs.map { it.messageId })
    }.filter { it.messages > 0 }.sortedByDescending { it.bytes }
