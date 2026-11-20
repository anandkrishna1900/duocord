package com.duochat.app.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The rules live in the shared engine (and are tested there); here only the reading and writing of its JSON. */
class QuickWinsTest {
    @Test
    fun `pinned chats come first in pin order`() {
        val rows = listOf("a", "b", "c", "d")
        assertEquals(listOf("c", "a", "b", "d"), withPinsFirst(rows, listOf("c", "a")) { it })
        assertEquals(rows, withPinsFirst(rows, listOf("gone")) { it })
    }

    @Test
    fun `the checkup question has the names the engine expects`() {
        val o = org.json.JSONObject(checkupInput(true, 5L, false, true, false, true, 2, 99L))
        assertEquals(true, o.getBoolean("app_lock"))
        assertEquals(5L, o.getLong("last_backup_at"))
        assertEquals(2, o.getInt("stale_devices"))
        assertEquals(99L, o.getLong("now_ms"))
    }

    @Test
    fun `the hosts question flattens the network settings`() {
        val o = org.json.JSONObject(hostsInput(NetworkConfig(forceRelay = true, turn = listOf(TurnServer(listOf("turn:turn.example.org:3478"), "u", "c")))))
        assertEquals("AUTOMATIC", o.getString("mode"))
        assertEquals(true, o.getBoolean("force_relay"))
        assertEquals("turn:turn.example.org:3478", o.getJSONArray("turn").getString(0))
        assertTrue(o.getJSONArray("nostr").length() > 0)
    }

    @Test
    fun `the engine's answers are read back`() {
        val items = parseCheckup("""[{"id":"lock","label":"App lock","ok":false,"hint":"Turn on"}]""")
        assertEquals(CheckupItem("lock", "App lock", false, "Turn on"), items.single())
        assertEquals(HostRow("relay.damus.io", "why"), parseHosts("""[{"host":"relay.damus.io","why":"why"}]""").single())
        val s = parseStorage("""[{"id":"x","messages":2,"files":1,"bytes":1005,"file_bytes":1000,"file_ids":["m1"]}]""") { it.uppercase() }.single()
        assertEquals("X", s.name)
        assertEquals(listOf("m1"), s.fileIds)
        assertTrue(parseCheckup("not json").isEmpty())
    }

    @Test
    fun `time of day is shown as HH_MM`() {
        assertEquals("07:05", minutesLabel(7 * 60 + 5))
    }

    @Test
    fun `delivery states are put in words`() {
        assertEquals("Read", deliveryWords(DeliveryState.READ))
        assertEquals("Waiting for them to come online", deliveryWords(DeliveryState.QUEUED))
    }
}
