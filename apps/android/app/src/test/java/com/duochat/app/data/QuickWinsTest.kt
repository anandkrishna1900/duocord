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
    fun `at most three pins, the oldest drops off`() {
        var p = emptyList<String>()
        for (id in listOf("a", "b", "c", "d")) p = togglePin(p, id)
        assertEquals(listOf("b", "c", "d"), p)
        assertEquals(listOf("b", "d"), togglePin(p, "c"))
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
