package com.duochat.app.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class QuickWinsTest {
    private val rows = listOf("a", "b", "c", "d")

    @Test
    fun `pinned chats come first in pin order`() {
        assertEquals(listOf("c", "a", "b", "d"), withPinsFirst(rows, listOf("c", "a")) { it })
        assertEquals(rows, withPinsFirst(rows, listOf("gone")) { it })
    }

    @Test
    fun `network hosts list relays and STUN, nothing in Bluetooth-only mode`() {
        val hosts = networkHosts(NetworkConfig()).map { it.host }
        assertTrue(hosts.any { it.contains("relay.damus.io") })
        assertTrue(networkHosts(NetworkConfig(transportMode = TransportMode.BLUETOOTH_ONLY)).isEmpty())
        val relayOnly = networkHosts(NetworkConfig(forceRelay = true, turn = listOf(TurnServer(listOf("turn:turn.example.org:3478"), "u", "c"))))
        assertFalse(relayOnly.any { it.why.contains("STUN") })
        assertEquals("turn.example.org:3478", relayOnly.first { it.why.contains("TURN") }.host)
    }

    @Test
    fun `checkup says what to fix`() {
        val now = 1_000_000_000_000L
        val none = privacyCheckup(false, 0, false, false, false, false, 0, now)
        assertEquals(1, none.count { it.ok })
        val all = privacyCheckup(true, now - 1000, true, true, true, true, 0, now)
        assertTrue(all.all { it.ok })
        assertFalse(privacyCheckup(true, now - 60L * 24 * 3600 * 1000, true, true, true, true, 0, now).first { it.id == "backup" }.ok)
        assertTrue(privacyCheckup(true, now, true, true, true, true, 2, now).first { it.id == "devices" }.hint.contains("2 linked devices"))
    }
}

class QuickWins2Test {
    @Test
    fun `time of day is shown as HH_MM`() {
        assertEquals("07:05", minutesLabel(7 * 60 + 5))
    }

    @Test
    fun `delivery states are put in words`() {
        assertEquals("Read", deliveryWords(DeliveryState.READ))
        assertEquals("Waiting for them to come online", deliveryWords(DeliveryState.QUEUED))
    }

    @Test
    fun `storage adds text and files per chat, biggest first, skipping deleted`() {
        fun m(id: String, text: String, size: Int? = null, deleted: Boolean = false) = ChatMessage(
            messageId = id, clientMessageId = id, senderId = "s", text = text, clientCreatedAt = 0L, deleted = deleted,
            file = size?.let { FileMeta("f", "image/png", it, "h") },
        )
        val rows = storageByChat(
            mapOf(
                "a" to listOf(m("1", "hello"), m("2", "", 1000), m("3", "gone", deleted = true)),
                "b" to listOf(m("4", "x".repeat(5000))),
                "c" to listOf(m("5", "bye", deleted = true)),
            ),
        ) { it.uppercase() }
        assertEquals(listOf("B", "A"), rows.map { it.name })
        assertEquals(1005L, rows[1].bytes)
        assertEquals(listOf("2"), rows[1].fileIds)
    }
}
